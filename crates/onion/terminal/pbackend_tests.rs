//! `pbackend` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//!
//! 真实 PTY 验收（`real_pty_roundtrip`）需要 console 上下文：Windows 走 ConPTY，
//! 无头宿主（CI / agent shell）下测试进程无 console，ConPTY 创建失败。这里沿用
//! `onion-tui/tests/pty_acceptance.rs` 的 `ensure_console` 门控——无 console 时
//! 优雅跳过（返回 Ok 空断言），不降级也不误报失败；交互终端下全真执行。

use crate::terminal::pbackend::{is_ready_marker, PtyBackend, CONTROLLED_PROMPT};
use crate::terminal::seam::{TerminalBackend, TerminalStatus};

#[test]
fn controlled_prompt_is_non_empty_marker() {
    // 就绪判定：受控提示符非空即视为 stdin 可写。
    assert!(!CONTROLLED_PROMPT.is_empty());
    assert!(is_ready_marker(CONTROLLED_PROMPT.as_bytes()));
    assert!(!is_ready_marker(&[]));
}

#[test]
fn type_is_shell() {
    let backend = PtyBackend::new();
    assert_eq!(backend.type_(), "shell");
}

#[test]
fn status_is_exited_when_no_session() {
    let backend = PtyBackend::new();
    // 未 spawn 即读状态：无会话视为已退出（幂等默认）。
    assert!(matches!(
        backend.status(),
        TerminalStatus::Exited { .. }
    ));
}

/// 真实 PTY 端到端：spawn → 写命令 → 读输出 → kill。
/// 无 console 上下文时优雅跳过（Windows ConPTY 需要）。
#[test]
fn real_pty_roundtrip() {
    if !ensure_console() {
        // 无 console 上下文（CI / agent shell）：无法创建 ConPTY，优雅跳过。
        return;
    }
    let backend = PtyBackend::new();
    let spawned = backend
        .spawn(None)
        .expect("spawn 真实 shell 应成功（已确保 console 上下文）");
    assert!(spawned.motd.contains("spawned"));

    // 写一个命令并提交（跨平台：Windows 用 pwsh，unix 用 bash）。
    let cmd = if cfg!(windows) { "Write-Output onion-pty-ok" } else { "echo onion-pty-ok" };
    backend.write(cmd, true).expect("写命令应成功");

    // 读输出：轮询直到出现标记或超时。
    let mut output = String::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        if let Ok(bytes) = backend.read() {
            output.push_str(&String::from_utf8_lossy(&bytes));
            if output.contains("onion-pty-ok") {
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(
        output.contains("onion-pty-ok"),
        "PTY 输出应包含命令回显/结果，实际: {output:?}"
    );

    // 清理：kill 进程树（幂等）。
    backend.kill().expect("kill 应成功");
}

fn ensure_console() -> bool {
    #[cfg(windows)]
    {
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
                // 已有 console 时 AllocConsole 失败（ERROR_ACCESS_DENIED=5），
                // 但此时已有 console 可用；故 alloc==0 且 err==5 也算有 console。
                HAS_CONSOLE = alloc != 0 || err == 5;
            });
            HAS_CONSOLE
        }
    }
    #[cfg(not(windows))]
    {
        true
    }
}
