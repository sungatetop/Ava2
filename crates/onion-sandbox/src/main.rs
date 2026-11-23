//! onion-sandbox：可插拔的 OS 级沙箱执行器（沙箱模式执行时强制）。
//!
//! 设计：独立二进制，onion shell 工具通过 spawn 本 exe 来执行命令。这样
//! 1) onion 主 crate 保持零 OS 级依赖（仅 spawn 进程，与直接 `Command::new` 同构）；
//! 2) 沙箱实现可插拔——未配置或缺失时 onion 可回退直通，也可替换为其它实现。
//!
//! 协议（stdout 输出 JSON，供 onion 解析）：
//! ```json
//! { "exit_code": 0, "stdout": "...", "stderr": "...", "sandbox_denied": false }
//! ```
//! `sandbox_denied` 为 true 表示命令被沙箱在 OS 层拒绝（如 read-only 下写文件），
//! onion 据此联动升级审批（escalation 重试流程）。
//!
//! 用法（program 与 argv 由 onion 侧 `spawn_plan` 预先解析，本 exe 透明执行）：
//! ```text
//! onion-sandbox --mode <read-only|workspace-write|danger-full-access> \
//!               --program <resolved-exe> --cwd <dir> -- <argv...>
//! ```

use std::path::PathBuf;

use clap::Parser;

mod platform;

/// 沙箱模式（三档）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxMode {
    /// 只读：禁止一切文件写（OS 层强制）。
    ReadOnly,
    /// 工作区可写：仅允许在 `--cwd` 下写，禁止越界写。
    WorkspaceWrite,
    /// 全权限：无文件写限制（危险，仅受信任场景）。
    DangerFullAccess,
}

/// 沙箱执行请求。
#[derive(Debug, Clone)]
pub struct SandboxExecRequest {
    pub mode: SandboxMode,
    /// 已解析的可执行文件（onion 侧 `spawn_plan` 产出，可为完整路径）。
    pub program: String,
    /// 可写根（workspace-write 模式下的允许写目录）。
    pub cwd: PathBuf,
    /// 预构建的完整 argv（逐个透传给 program，不再二次拼接）。
    pub command: Vec<String>,
}

/// 沙箱执行结果。
#[derive(Debug, serde::Serialize)]
pub struct SandboxExecResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    /// 命令是否被沙箱在 OS 层拒绝（写越界/只读写）。
    pub sandbox_denied: bool,
}

impl SandboxExecResult {
    fn ok(exit_code: i32, stdout: String, stderr: String) -> Self {
        Self {
            exit_code,
            stdout,
            stderr,
            sandbox_denied: false,
        }
    }

    fn denied(stdout: String, stderr: String) -> Self {
        Self {
            exit_code: -1,
            stdout,
            stderr,
            sandbox_denied: true,
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "onion-sandbox", about = "Onion 可插拔 OS 级沙箱执行器")]
struct Cli {
    /// 沙箱模式：read-only | workspace-write | danger-full-access。
    #[arg(long, value_enum)]
    mode: ModeArg,

    /// 已解析的可执行文件（完整路径或 PATH 名）。
    #[arg(long)]
    program: String,

    /// 工作目录（workspace-write 模式下的可写根）。
    #[arg(long)]
    cwd: PathBuf,

    /// 待执行的完整 argv（`--` 之后逐个透传给 program）。
    #[arg(last = true)]
    command: Vec<String>,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum ModeArg {
    #[value(name = "read-only")]
    ReadOnly,
    #[value(name = "workspace-write")]
    WorkspaceWrite,
    #[value(name = "danger-full-access")]
    DangerFullAccess,
}

impl From<ModeArg> for SandboxMode {
    fn from(mode: ModeArg) -> Self {
        match mode {
            ModeArg::ReadOnly => SandboxMode::ReadOnly,
            ModeArg::WorkspaceWrite => SandboxMode::WorkspaceWrite,
            ModeArg::DangerFullAccess => SandboxMode::DangerFullAccess,
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let request = SandboxExecRequest {
        mode: cli.mode.into(),
        program: cli.program,
        cwd: cli.cwd,
        command: cli.command,
    };
    let result = match platform::run_sandboxed(&request) {
        Ok(result) => result,
        Err(message) => {
            // 启动失败（如方言不可用）：以非 denied 的失败返回，exit_code 由启动失败语义表示。
            eprintln!("onion-sandbox: {message}");
            SandboxExecResult {
                exit_code: -1,
                stdout: String::new(),
                stderr: message,
                sandbox_denied: false,
            }
        }
    };
    // 结果以 JSON 写到 stdout（onion 侧解析）。stderr 仅用于本工具自身的诊断日志。
    let _ = serde_json::to_writer(std::io::stdout().lock(), &result);
    println!();
}
