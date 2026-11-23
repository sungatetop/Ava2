//! 可插拔 OS 级沙箱执行端口（执行时强制）。
//!
//! onion 主 crate 保持零 OS 级依赖（不引入 windows-sys / landlock / seatbelt）：
//! 沙箱是**独立可插拔 exe**（`onion-sandbox`），本模块只是一个 trait + 配置 seam。
//! 未配置沙箱时 shell 执行回退直通（`std::process::Command`），保持教学示例可独立运行。
//!
//! 沙箱协议（见 `onion-sandbox` crate）：program 与 argv 由
//! [`ShellDialect::spawn_plan`] 预先解析（Windows 编码修正内建），sandbox exe
//! 只做透明执行：
//! ```text
//! onion-sandbox --mode <read-only|workspace-write|danger-full-access> \
//!               --program <resolved-exe> --cwd <dir> -- <argv...>
//! ```
//! stdout 输出 JSON：`{ "exit_code", "stdout", "stderr", "sandbox_denied" }`。
//! `sandbox_denied` 为 true 表示命令被沙箱在 OS 层拒绝（如 read-only 下写文件），
//! 上层据此联动升级审批（escalation 重试流程）。

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use tokio_util::sync::CancellationToken;

use super::{ExecOutcome, ShellDialect};
use crate::approval::{ApprovalConfig, SandboxLevel};
use crate::waterfall::BoxFuture;

/// 沙箱执行端口。实现方负责 spawn `onion-sandbox` exe 并解析 JSON 结果。
pub trait SandboxRunner: Send + Sync {
    /// 在 OS 沙箱内执行一次性命令。`cwd` 为工作目录（workspace-write 模式的可写根）；
    /// `mode` 为一次性沙箱级覆盖（`None` = 取配置的当前预设级）。
    fn exec(
        &self,
        dialect: ShellDialect,
        command: &str,
        cwd: Option<&std::path::Path>,
        mode: Option<SandboxLevel>,
        cancel: CancellationToken,
    ) -> BoxFuture<Result<ExecOutcome, String>>;
}

/// 沙箱执行结果（镜像 `onion-sandbox` 的 JSON 输出）。
#[derive(Debug, serde::Deserialize)]
pub struct SandboxResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    /// 命令是否被沙箱在 OS 层拒绝（写越界/只读写）。
    pub sandbox_denied: bool,
}

/// 沙箱配置：可插拔 exe 路径 + 共享审批配置句柄。
#[derive(Clone)]
pub struct SandboxConfig {
    /// `onion-sandbox` exe 的绝对路径。
    pub executable: std::path::PathBuf,
    /// 共享审批配置（与 `ApprovalGate` 同源）：OS 沙箱模式由 `preset.sandbox`
    /// 动态派生（`/permission` 切换即时生效），而非装配期硬编码。
    pub approval: Arc<RwLock<ApprovalConfig>>,
}

/// 经 `onion-sandbox` exe 执行的沙箱 runner。
///
/// 未配置（`Option::None`）时由调用方回退直通，见 [`OneshotExecutor`]。
pub struct ProcessSandboxRunner {
    pub config: Arc<SandboxConfig>,
}

impl ProcessSandboxRunner {
    pub fn new(config: SandboxConfig) -> Self {
        Self { config: Arc::new(config) }
    }
}

impl SandboxConfig {
    /// 当前审批预设的沙箱模式（`SandboxMode` 值名）。
    /// 读取共享句柄的 `preset.sandbox`，运行时可切换。
    pub fn mode_label(&self) -> &'static str {
        self.mode().label()
    }

    /// 当前审批预设的沙箱级别（与 `ApprovalGate` 同源）。
    pub fn mode(&self) -> SandboxLevel {
        self.approval
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .preset
            .sandbox
    }
}

impl SandboxLevel {
    /// 沙箱模式的模型可见标签（`SandboxMode` 值名）。
    pub fn label(self) -> &'static str {
        match self {
            SandboxLevel::ReadOnly => "read-only",
            SandboxLevel::WorkspaceWrite => "workspace-write",
            SandboxLevel::DangerFullAccess => "danger-full-access",
        }
    }
}

impl SandboxRunner for ProcessSandboxRunner {
    fn exec(
        &self,
        dialect: ShellDialect,
        command: &str,
        cwd: Option<&std::path::Path>,
        mode: Option<SandboxLevel>,
        _cancel: CancellationToken,
    ) -> BoxFuture<Result<ExecOutcome, String>> {
        let config = self.config.clone();
        let command = command.to_string();
        let cwd = cwd.map(PathBuf::from);
        Box::pin(async move {
            // 一次性覆盖优先（升级重放固定 danger-full-access），否则取预设当前级。
            let mode = mode.unwrap_or_else(|| config.mode()).label();
            // 解析 spawn 计划（program 解析失败即返回错误，不进沙箱）。
            let plan = dialect.spawn_plan(&command)?;
            let mut cmd = tokio::process::Command::new(&config.executable);
            cmd.arg("--mode")
                .arg(mode)
                .arg("--program")
                .arg(&plan.program);
            if let Some(cwd) = cwd {
                cmd.arg("--cwd").arg(cwd);
            }
            cmd.arg("--").args(&plan.args);
            cmd.stdin(std::process::Stdio::null());
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::piped());

            let output = cmd.output().await.map_err(|error| {
                format!("failed to start sandbox {}: {error}", config.executable.display())
            })?;
            let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            // stdout 是 JSON 结果；若解析失败则按普通输出处理（不阻断）。
            let result: Option<SandboxResult> = serde_json::from_str(&stdout).ok();
            let (exit_code, body, sandbox_denied) = match result {
                Some(result) => (
                    result.exit_code,
                    if result.stdout.is_empty() {
                        result.stderr
                    } else {
                        result.stdout
                    },
                    result.sandbox_denied,
                ),
                None => (output.status.code().unwrap_or(-1), stdout, false),
            };
            // 沙箱拒绝（写越界/只读写）：以模型可见标记返回，上层联动升级审批。
            if sandbox_denied {
                return Ok(ExecOutcome {
                    exit_code,
                    stdout: format!("[sandbox: command denied under {mode} mode]{body}"),
                    stderr: String::new(),
                    truncated: false,
                    timed_out: false,
                    job_id: None,
                    sandbox_denied: true,
                });
            }
            Ok(ExecOutcome {
                exit_code,
                stdout: body,
                stderr,
                truncated: false,
                timed_out: false,
                job_id: None,
                sandbox_denied: false,
            })
        })
    }
}
