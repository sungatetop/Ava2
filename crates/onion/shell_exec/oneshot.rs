//! 一次性进程执行：超时 kill / 输出截断 / exit code 归一 / spill。
//!
//! 工具层（`tools/shell.rs`）只调用 [`OneshotExecutor`]，不直接触碰
//! `std::process::Command`：沙箱与审批判定收敛在工具层，执行策略（超时、截断、
//! spill）收敛在本文件。
//!
//! 策略说明：
//! - 超时：达到 `timeoutMs` 上限时 kill 进程并以 `timed_out` 返回
//!   （`run_in_background` 参数位在工具层预留，后台晋升随 jobs 控制面落地）。
//! - 截断：stdout / stderr 各自保留尾部；完整输出 spill 到
//!   `<root>/logs/jobs/<hash>.log`，并在结果中提示该路径。
//! - exit code：退出码原样透传；被取消 / 启动失败归一为 `-1`。

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use super::{ExecOutcome, ShellDialect, SpawnPlan, sandbox::SandboxRunner};

/// 单流输出保留的最大字节数（超出后丢弃最旧并置 `truncated`）。
const MAX_CAPTURE_BYTES: usize = 16 * 1024;

/// 一次性执行器：对每个请求 spawn 一个 fresh 子进程，捕获输出、处理超时与截断。
///
/// `stdout` 字段承载模型可见结果正文（含 exit code、stderr 摘要、spill 提示），
/// `stderr` 保留原始错误流，`truncated` / `timed_out` 供上层进一步决策。
pub struct OneshotExecutor {
    /// 输出 spill 根目录（`<root>/logs/jobs/`）。None = 不落盘。
    spill_dir: Option<PathBuf>,
    /// 可插拔 OS 级沙箱执行器。None = 直通（无 OS 强制）。
    sandbox: Option<Arc<dyn SandboxRunner>>,
}

impl OneshotExecutor {
    pub fn new(spill_dir: Option<PathBuf>) -> Self {
        Self { spill_dir, sandbox: None }
    }

    /// 接入可插拔 OS 级沙箱执行器（配置 `onion-sandbox` exe 后启用执行时强制）。
    pub fn with_sandbox(mut self, sandbox: Arc<dyn SandboxRunner>) -> Self {
        self.sandbox = Some(sandbox);
        self
    }

    /// 是否接入了 OS 级沙箱。`WorkspaceWrite` 档的免审批前提即此（沙箱越界写兜底）。
    pub fn sandbox_enabled(&self) -> bool {
        self.sandbox.is_some()
    }

    /// 以 fresh 子进程执行一次性命令（program 与 argv 经 [`ShellDialect::spawn_plan`]
    /// 解析，Windows PowerShell 走 `-EncodedCommand`）。沙箱级取共享审批配置的当前
    /// `preset.sandbox`。
    pub async fn exec(
        &self,
        dialect: ShellDialect,
        command: &str,
        cwd: Option<&Path>,
        timeout: Option<std::time::Duration>,
        cancel: CancellationToken,
    ) -> Result<ExecOutcome, String> {
        self.exec_with_mode(dialect, command, cwd, /*mode*/ None, timeout, cancel).await
    }

    /// 同上，但可一次性覆盖 OS 沙箱级（`mode = None` 时取共享配置的当前值）。
    /// 供沙箱拒绝后的升级重放使用——不改写共享 preset，避免并发窗口与取消时
    /// 的 fail-open。
    pub async fn exec_with_mode(
        &self,
        dialect: ShellDialect,
        command: &str,
        cwd: Option<&Path>,
        mode: Option<crate::approval::SandboxLevel>,
        timeout: Option<std::time::Duration>,
        cancel: CancellationToken,
    ) -> Result<ExecOutcome, String> {
        // 配置了 OS 级沙箱：经 `onion-sandbox` exe 执行（执行时 OS 强制）。
        // 未配置则直通（保持教学示例可独立运行，无 OS 强制）。
        if let Some(sandbox) = &self.sandbox {
            return sandbox.exec(dialect, command, cwd, mode, cancel).await;
        }

        let plan = dialect.spawn_plan(command)?;
        let SpawnPlan { program, args } = plan;
        let mut cmd = Command::new(&program);
        cmd.args(&args);
        if let Some(cwd) = cwd {
            cmd.current_dir(cwd);
        }
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.kill_on_drop(true);

        let mut child = cmd.spawn().map_err(|error| {
            format!("failed to start {program}: {error} (is it installed and on PATH?)")
        })?;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let out_fut = capture(stdout, MAX_CAPTURE_BYTES);
        let err_fut = capture(stderr, MAX_CAPTURE_BYTES);
        tokio::pin!(out_fut);
        tokio::pin!(err_fut);

        // 等待退出 / 取消 / 超时 三路竞速。
        enum Wait {
            Exit,
            Cancelled,
            TimedOut,
        }
        let wait = match timeout {
            Some(timeout) => {
                tokio::select! {
                    _ = cancel.cancelled() => Wait::Cancelled,
                    _ = tokio::time::sleep(timeout) => Wait::TimedOut,
                    status = child.wait() => {
                        let (out, err) = tokio::join!(out_fut, err_fut);
                        return Ok(finish(command, status, out, err, &self.spill_dir));
                    }
                }
            }
            None => {
                tokio::select! {
                    _ = cancel.cancelled() => Wait::Cancelled,
                    status = child.wait() => {
                        let (out, err) = tokio::join!(out_fut, err_fut);
                        return Ok(finish(command, status, out, err, &self.spill_dir));
                    }
                }
            }
        };

        // 取消或超时：kill 子进程，归一退出码 -1。
        let _ = child.kill().await;
        let status = child.wait().await;
        let (out, err) = tokio::join!(out_fut, err_fut);
        let mut outcome = finish(command, status, out, err, &self.spill_dir);
        outcome.exit_code = -1;
        outcome.timed_out = matches!(wait, Wait::TimedOut);
        Ok(outcome)
    }
}

/// 进程结束后拼装结果：exit code + 输出 + spill 提示。
fn finish(
    command: &str,
    status: Result<std::process::ExitStatus, std::io::Error>,
    out: (String, bool),
    err: (String, bool),
    spill_dir: &Option<PathBuf>,
) -> ExecOutcome {
    let exit_code = status.map(|s| s.code().unwrap_or(-1)).unwrap_or(-1);
    let mut truncated = out.1 || err.1;
    let stdout = out.0;
    let stderr = err.0;

    // 有完整输出且配了 spill 目录：落盘并提示路径（标记截断，完整内容在文件）。
    let mut spill_hint = String::new();
    if let Some(dir) = spill_dir {
        if !stdout.is_empty() || !stderr.is_empty() {
            if let Some(path) = spill(dir, command, &stdout, &stderr) {
                // spill notice（notice + retrievalHint）：读类工具不受
                // 权限根遏制，模型可直接 read 该路径取回完整输出。
                spill_hint = format!("\n[full output saved to {}; use read to view it]", path.display());
                truncated = true;
            }
        }
    }

    let stderr_text = if stderr.is_empty() { String::new() } else { format!("\n{stderr}") };
    let body = format!("[exit code: {exit_code}]{stderr_text}{spill_hint}");

    ExecOutcome {
        exit_code,
        stdout: body,
        stderr,
        truncated,
        timed_out: false,
        job_id: None,
        sandbox_denied: false,
    }
}

/// 有界捕获：读满流后截断到尾部 `max` 字节。返回 (内容, 是否截断)。
async fn capture<R: AsyncRead + Unpin>(reader: Option<R>, max: usize) -> (String, bool) {
    let Some(mut reader) = reader else {
        return (String::new(), false);
    };
    let mut buf = Vec::new();
    let _ = reader.read_to_end(&mut buf).await;
    if buf.len() <= max {
        (String::from_utf8_lossy(&buf).to_string(), false)
    } else {
        let start = buf.len() - max;
        (String::from_utf8_lossy(&buf[start..]).to_string(), true)
    }
}

/// 把完整输出 spill 到 `<spill_dir>/<hash>.log`，返回文件路径。
fn spill(dir: &Path, command: &str, stdout: &str, stderr: &str) -> Option<PathBuf> {
    std::fs::create_dir_all(dir).ok()?;
    let name = crate::paths::encode_session_id(command);
    let path = dir.join(format!("{name}.log"));
    let body = format!("$ {command}\n\n[stdout]\n{stdout}\n\n[stderr]\n{stderr}\n");
    std::fs::write(&path, body).ok()?;
    Some(path)
}
