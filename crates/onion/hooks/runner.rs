//! 子进程执行命令钩子（两方言共享）。
//!
//! stdin 写 JSON payload、超时、取消。
//! 无 `ctx.shell`，用 `tokio::process::Command` 封装。基础设施故障
//! （spawn 失败 / 信号死亡）成为「无退出码」的非阻塞错误，绝不 crash turn。

use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::process::{Child, Command};
use tokio_util::sync::CancellationToken;

use crate::hooks::codec::parse_hook_output;
use crate::hooks::types::CommandHook;

/// 参考默认单钩子超时（ms，10 分钟）——CC 与 Codex 对未设 timeout 的钩子均用此值。
pub const DEFAULT_HOOK_TIMEOUT_MS: u64 = 600_000;

/// 单次钩子调用的输入（桥构建 payload / env / cwd）。
pub struct RunHookOptions {
    /// 写入钩子 stdin 的 JSON payload（桥构建）。
    pub payload: serde_json::Value,
    /// 钩子进程的额外 env（如 `CLAUDE_PROJECT_DIR`）。
    pub env: Vec<(String, String)>,
    /// 钩子工作目录（缺省为执行器默认）。
    pub cwd: Option<std::path::PathBuf>,
    /// 取消句柄；触发则终止钩子运行。
    pub cancel: CancellationToken,
    /// 是否给 stdin payload 追加 trailing newline（CC 加，Codex 不加）。
    pub trailing_newline: bool,
    /// 钩子自身未设 timeout 时应用的默认超时（桥拥有默认值并显式传入）。
    pub default_timeout_ms: u64,
    /// 触发事件（用于门控 `hookSpecificOutput`；缺省不设门控）。
    pub expected_event_name: Option<String>,
}

/// 解码后的输出 + 运行墙钟时长（供 `hook/result` 审计）。
pub struct RunHookResult {
    pub output: crate::hooks::types::HookOutput,
    pub duration_ms: u64,
}

/// 运行 `hook`：序列化 stdin、解码其输出。单钩子超时（秒）覆盖默认；
/// 信任的 env 条目在 exec 时合并。spawn 失败成为无退出码的结果，本函数不抛。
pub async fn run_hook(hook: &CommandHook, options: RunHookOptions) -> RunHookResult {
    let started = std::time::Instant::now();
    let timeout_ms = hook
        .timeout_sec
        .map(|sec| sec.saturating_mul(1000))
        .unwrap_or(options.default_timeout_ms);

    // Windows 上经 cmd /C 包装；其它平台直接 exec（保持与 shell 语义一致）。
    let mut cmd = make_command(&hook.command);
    cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if let Some(cwd) = &options.cwd {
        cmd.current_dir(cwd);
    }
    for (k, v) in &options.env {
        cmd.env(k, v);
    }

    let mut child: Child = match cmd.spawn() {
        Ok(child) => child,
        Err(error) => {
            // 钩子无法运行 = 非阻塞错误：无退出码，失败上 stderr 留档；轮继续。
            return RunHookResult {
                output: parse_hook_output(None, "", &error.to_string(), options.expected_event_name.as_deref()),
                duration_ms: started.elapsed().as_millis() as u64,
            };
        }
    };

    // 写 stdin（trailing newline 由桥方言决定）。
    let stdin = child.stdin.take();
    let payload = serde_json::to_string(&options.payload).unwrap_or_else(|_| "{}".to_string());
    let stdin_bytes = if options.trailing_newline {
        format!("{payload}\n")
    } else {
        payload
    };
    let stdin_task = tokio::spawn(async move {
        if let Some(mut s) = stdin {
            let _ = s.write_all(stdin_bytes.as_bytes()).await;
            let _ = s.shutdown().await;
        }
    });

    // 收集 stdout/stderr + 等待退出，带超时与取消。
    let cancel = options.cancel;
    let timeout = tokio::time::sleep(Duration::from_millis(timeout_ms));
    tokio::pin!(timeout);
    let output = tokio::select! {
        _ = cancel.cancelled() => {
            let _ = child.kill().await;
            parse_hook_output(None, "", "hook cancelled", options.expected_event_name.as_deref())
        }
        _ = &mut timeout => {
            let _ = child.kill().await;
            let msg = format!("hook timed out after {timeout_ms}ms");
            parse_hook_output(None, "", &msg, options.expected_event_name.as_deref())
        }
        status = child.wait() => {
            let exit_code = status.ok().and_then(|s| s.code());
            let stdout = child_stdout(&mut child).await;
            let stderr = child_stderr(&mut child).await;
            parse_hook_output(exit_code, &stdout, &stderr, options.expected_event_name.as_deref())
        }
    };
    let _ = stdin_task.await;
    let duration_ms = started.elapsed().as_millis() as u64;
    RunHookResult { output, duration_ms }
}

/// 构造命令：Windows 走 `cmd /S /C "<command>"`，其它平台直接 exec。
/// `/S` 让 cmd 剥掉首尾引号、原样保留内部引号——这样命令里的带引号路径
/// （如 `node "${CLAUDE_PLUGIN_ROOT}/hook.mjs"`）不会被 std 的自动引号转义破坏。
fn make_command(command: &str) -> Command {
    #[cfg(windows)]
    {
        let mut cmd = Command::new("cmd");
        cmd.raw_arg("/S").raw_arg("/C").raw_arg(format!("\"{command}\""));
        cmd
    }
    #[cfg(not(windows))]
    {
        Command::new("sh").arg("-c").arg(command)
    }
}

/// 读取子进程 stdout（wait 后取）。
async fn child_stdout(child: &mut Child) -> String {
    match child.stdout.take() {
        Some(mut out) => {
            use tokio::io::AsyncReadExt;
            let mut buf = Vec::new();
            let _ = out.read_to_end(&mut buf).await;
            String::from_utf8_lossy(&buf).into_owned()
        }
        None => String::new(),
    }
}

/// 读取子进程 stderr。
async fn child_stderr(child: &mut Child) -> String {
    match child.stderr.take() {
        Some(mut err) => {
            use tokio::io::AsyncReadExt;
            let mut buf = Vec::new();
            let _ = err.read_to_end(&mut buf).await;
            String::from_utf8_lossy(&buf).into_owned()
        }
        None => String::new(),
    }
}

#[cfg(test)]
#[path = "runner_tests.rs"]
mod tests;
