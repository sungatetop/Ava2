//! tests/pty_acceptance.rs：TUI front door 的 PTY 端到端集成验收。
//!
//! 用 portable-pty（Windows 上走 ConPTY）拉起真二进制
//! `env!("CARGO_BIN_EXE_onion-tui")`，验证四件断言：
//! 1. 非 TTY 环境（管道 stdin/stdout）启动即报错退出（TTY gate fail-loud）；
//! 2. 正常退出（Esc）后终端状态恢复（raw mode 解除、alternate screen 退出）；
//! 3. 控制键（Ctrl+C / Ctrl+E）在 raw mode 下按字节到达并被正确消费；
//! 4. panic hook 触发时终端状态可恢复（ONION_TUI_PANIC=1 注入 panic 验证）。
//!
//! 环境门控：ConPTY（CreatePseudoConsole）需要测试进程有 console 上下文。无头宿主
//! （CI / agent shell）下测试进程没有 console，ConPTY 创建会失败（HRESULT
//! 0x80070006, E_HANDLE）。此时优雅跳过并说明，不降级也不误报失败；在真实交互终端
//! （Windows Terminal / VS Code 集成终端）下四个测试全真执行。

#![cfg(windows)]
// PTY 装配是测试夹具的固有失败点（ConPTY / spawn / clone），断言失败即测试失败，
// 故在集成测试层显式放行 expect_used（对齐仓库内其它 tests/*.rs 约定）。
#![allow(clippy::expect_used, clippy::unwrap_used)]

use portable_pty::native_pty_system;
use portable_pty::{CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::time::{Duration, Instant};

/// ConPTY（CreatePseudoConsole）在调用进程没有 console 上下文时会失败
/// （HRESULT 0x80070006, E_HANDLE）——无头宿主（CI / agent shell）下 cargo test
/// 派生的测试进程没有 console。这里先给测试进程挂一个 console 再建 PTY；
/// 已有 console 时 AllocConsole 失败，忽略即可。
fn ensure_console() -> bool {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    static mut HAS_CONSOLE: bool = false;
    unsafe {
        ONCE.call_once(|| {
            #[link(name = "kernel32")]
            extern "system" {
                fn FreeConsole() -> i32;
                fn AllocConsole() -> i32;
                fn GetLastError() -> u32;
            }
            FreeConsole();
            let alloc = AllocConsole();
            let err = GetLastError();
            eprintln!("ensure_console: AllocConsole={alloc} GetLastError={err:#x}");
            HAS_CONSOLE = alloc != 0;
        });
        HAS_CONSOLE
    }
}

/// PTY 里跑 onion-tui 的辅助：拉起 + 给你 reader/writer + 子进程句柄。
fn spawn_in_pty(
    extra_envs: &[(&str, &str)],
) -> (
    Box<dyn Write + Send>,
    Box<dyn Read + Send>,
    Box<dyn portable_pty::Child + Send + Sync>,
) {
    ensure_console();
    let pty_system = native_pty_system();
    let pty = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("openpty");

    let mut cmd = CommandBuilder::new(env!("CARGO_BIN_EXE_onion-tui"));
    cmd.cwd(env!("CARGO_MANIFEST_DIR"));
    for (k, v) in extra_envs {
        cmd.env(k, v);
    }

    let child = pty.slave.spawn_command(cmd).expect("spawn in pty");
    let reader = pty.master.try_clone_reader().expect("clone reader");
    let writer = pty.master.take_writer().expect("take writer");
    (writer, reader, child)
}

/// 汇聚输出直到匹配到 `needle` 或超时；返回已读全量字节。
fn wait_for_output(reader: &mut Box<dyn Read + Send>, needle: &[u8], timeout: Duration) -> Vec<u8> {
    let mut buf = [0u8; 4096];
    let mut all = Vec::new();
    let deadline = Instant::now() + timeout;
    loop {
        if all.windows(needle.len()).any(|w| w == needle) {
            return all;
        }
        if Instant::now() > deadline {
            return all;
        }
        match reader.read(&mut buf) {
            Ok(0) => {
                if Instant::now() > deadline {
                    return all;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(n) => all.extend_from_slice(&buf[..n]),
            Err(_) => return all,
        }
    }
}

/// 断言一：非 TTY 环境（管道 stdin/stdout）启动即报错退出。
#[test]
fn non_tty_startup_fails_loud() {
    let bin = env!("CARGO_BIN_EXE_onion-tui");
    let out = std::process::Command::new(bin)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("spawn non-tty");
    assert!(!out.status.success(), "non-tty 启动必须报错退出（fail-loud）");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("not a TTY") || stderr.contains("TTY"),
        "错误信息应说明 TTY gate，实际: {stderr}"
    );
}

/// 断言二：正常退出（Esc）后终端状态恢复（raw mode 解除、alternate screen 退出）。
#[test]
fn normal_exit_restores_terminal() {
    if !ensure_console() {
        eprintln!("skip: 无 console 宿主下 ConPTY 无法创建 PTY（环境限制，非被测代码缺陷）");
        return;
    }
    let (mut writer, mut reader, mut child) = spawn_in_pty(&[]);
    // 先读一阵子输出，确认 TUI 已渲染（含 alternate screen 序列）。
    let out = wait_for_output(&mut reader, b"onion", Duration::from_secs(15));
    assert!(
        out.windows(b"\x1b[?1049h".len()).any(|w| w == b"\x1b[?1049h"),
        "应输出 EnterAlternateScreen 序列，实际: {:?}",
        String::from_utf8_lossy(&out)
    );

    // Esc → is_quit_key → break 'main → term.restore()。
    writer.write_all(&[0x1b]).expect("write Esc");
    writer.flush().ok();

    let status = child.wait().expect("wait child");
    assert!(status.success(), "正常退出应 success，实际: {status:?}");

    let out = wait_for_output(&mut reader, b"\x1b[?1049l", Duration::from_secs(10));
    assert!(
        out.windows(b"\x1b[?1049l".len()).any(|w| w == b"\x1b[?1049l"),
        "退出后应输出 LeaveAlternateScreen 序列，实际: {:?}",
        String::from_utf8_lossy(&out)
    );
}

/// 断言三：控制键（Ctrl+C / Ctrl+E）在 raw mode 下按字节到达并被正确消费。
#[test]
fn control_keys_consumed_in_raw_mode() {
    if !ensure_console() {
        eprintln!("skip: 无 console 宿主下 ConPTY 无法创建 PTY（环境限制，非被测代码缺陷）");
        return;
    }
    let (mut writer, mut reader, mut child) = spawn_in_pty(&[]);
    // 等待 TUI 起来。
    let _ = wait_for_output(&mut reader, b"onion", Duration::from_secs(15));

    // raw mode 下 Ctrl+C = 0x03，Ctrl+E = 0x05；按字节到达即被消费（不产生 SIGINT，
    // 进程继续活着）；最后 Esc 退出。
    writer.write_all(&[0x03]).expect("write Ctrl+C");
    writer.flush().ok();
    writer.write_all(&[0x05]).expect("write Ctrl+E");
    writer.flush().ok();
    std::thread::sleep(Duration::from_millis(200));

    writer.write_all(&[0x1b]).expect("write Esc");
    writer.flush().ok();

    let status = child.wait().expect("wait child");
    assert!(status.success(), "raw mode 下控制键应被消费，进程应能正常退出，实际: {status:?}");
}

/// 断言四：panic hook 触发时终端状态可恢复（ONION_TUI_PANIC=1 注入 panic 验证）。
#[test]
fn panic_hook_restores_terminal() {
    if !ensure_console() {
        eprintln!("skip: 无 console 宿主下 ConPTY 无法创建 PTY（环境限制，非被测代码缺陷）");
        return;
    }
    let (mut writer, mut reader, mut child) = spawn_in_pty(&[("ONION_TUI_PANIC", "1")]);
    // 注入 panic 在 enter() 之后、主循环之前——进程应崩溃，但先 restore 终端。
    let status = child.wait().expect("wait child");
    assert!(!status.success(), "注入 panic 应使进程异常退出");

    let out = wait_for_output(&mut reader, b"\x1b[?1049l", Duration::from_secs(10));
    assert!(
        out.windows(b"\x1b[?1049l".len()).any(|w| w == b"\x1b[?1049l"),
        "panic 路径应先 restore 终端（LeaveAlternateScreen），实际: {:?}",
        String::from_utf8_lossy(&out)
    );
    let _ = writer.write_all(&[0x1b]); // 收尾清缓冲（reader 已不需要）
}
