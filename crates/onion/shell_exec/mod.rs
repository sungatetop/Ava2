//! 进程执行 seam：一次性命令与持久 PTY 的统一端口。
//!
//! 工具层（`tools/shell.rs`、`tools/terminal.rs`）只依赖本 trait，不直接触碰
//! `std::process::Command` / `portable-pty`：执行策略（超时、输出截断、spill、
//! 沙箱与审批判定）收敛在实现侧，工具定义保持薄壳。
//!
//! 与后台任务的关系：一次性执行超时或失败即返回 [`ExecOutcome`]；若以
//! `run_in_background` 晋升为后台任务，由实现侧转交 `onion-jobs` 注册表跟踪，
//! 工具层只拿到 `JobId`。

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use onion_jobs::{JobId, SessionId};

pub mod background;
pub mod oneshot;
pub mod sandbox;

/// shell 方言（`bash` / `pwsh` 为两个独立工具，共享本 seam）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellDialect {
    /// POSIX shell（Linux / macOS / Windows Git Bash；WSL 启动器不可用，见
    /// [`Self::resolve_program`]）。
    Bash,
    /// PowerShell（Windows 原生方言，fresh process，无持久 PTY）。
    Pwsh,
}

/// 解析后的 spawn 计划：可执行文件 + 完整 argv（编码修正内建）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpawnPlan {
    /// 可执行文件（PATH 名或完整路径）。
    pub program: String,
    /// spawn argv（bash `-c <command>`；PowerShell `-EncodedCommand <b64>`）。
    pub args: Vec<String>,
}

impl ShellDialect {
    /// 解析实际可执行文件（进程级缓存探测结果）。
    ///
    /// Windows 特例（对齐 ava-toolkit::shell 的 Windows 修复）：
    /// - bash：`Command::new("bash")` 的解析顺序里 System32 优先于 PATH，命中的是
    ///   WSL 启动器（对 `-c` 的 stdout 捕获不可靠、中文乱码根因）——故经 `where.exe`
    ///   显式定位并排除 System32 命中，返回完整路径；无 Git Bash 时明确报错。
    /// - pwsh：PowerShell 7+（需单独安装）优先，回落 Windows 内置 `powershell.exe`。
    pub fn resolve_program(self) -> Result<String, String> {
        static CACHE: [OnceLock<Result<String, String>>; 2] = [OnceLock::new(), OnceLock::new()];
        CACHE[self as usize].get_or_init(|| self.probe_program()).clone()
    }

    /// 构造 spawn 计划：解析 program + 方言 argv（PowerShell 含编码修正）。
    pub fn spawn_plan(self, command: &str) -> Result<SpawnPlan, String> {
        let program = self.resolve_program()?;
        let args = match self {
            ShellDialect::Bash => vec!["-c".to_string(), command.to_string()],
            ShellDialect::Pwsh => pwsh_args(command),
        };
        Ok(SpawnPlan { program, args })
    }

    fn probe_program(self) -> Result<String, String> {
        if std::env::consts::OS != "windows" {
            return Ok(match self {
                ShellDialect::Bash => "bash".to_string(),
                ShellDialect::Pwsh => "pwsh".to_string(),
            });
        }
        match self {
            ShellDialect::Bash => where_first("bash", WSL_LAUNCHER_SUFFIXES)
                .or_else(git_bash_default_path)
                .ok_or_else(|| {
                    "bash not usable on Windows (System32\\bash.exe and the WindowsApps \
                     alias are the WSL launcher; install Git Bash)"
                        .to_string()
                }),
            ShellDialect::Pwsh => {
                where_first("pwsh", &[])
                    .or_else(|| where_first("powershell", &[]))
                    .ok_or_else(|| "neither pwsh nor powershell found on PATH".to_string())
            }
        }
    }

    /// 命令超时默认档（一次性 shell 的默认超时量级）。
    pub fn default_timeout(self) -> Duration {
        match self {
            // bash 无持久进程，超时须保守（长构建类命令可由 run_in_background 承接）。
            ShellDialect::Bash => Duration::from_secs(120),
            ShellDialect::Pwsh => Duration::from_secs(120),
        }
    }
}

/// PowerShell argv（对齐 ava-toolkit::shell 的 Windows 三件套，全平台统一）：
/// - `-EncodedCommand`（UTF-16LE Base64）：命令中的引号/特殊字符不被命令行层二次解析；
/// - 前缀 `$ProgressPreference='SilentlyContinue'`：抑制进度流（否则首次运行会把
///   `#< CLIXML` 进度记录混进 stdout）；
/// - 前缀 `[Console]::OutputEncoding=...UTF8`：管道输出强制 UTF-8（系统 OEM 代码页
///   CP936 是中文乱码根因）。**必须包 try/catch**：OS 沙箱下的受限令牌会令
///   PowerShell 进入 ConstrainedLanguage，属性赋值直接报错；错误则退化为不设编码；
/// - 尾缀 `exit $LASTEXITCODE`：脚本执行完显式退出，`child.wait()` 可靠返回
///   （ConPTY/管道下 PowerShell 不自动退出的修复）。
fn pwsh_args(command: &str) -> Vec<String> {
    let script = format!(
        "$ProgressPreference='SilentlyContinue'; \
         try {{ [Console]::OutputEncoding=[System.Text.Encoding]::UTF8 }} catch {{ }}; \
         {command}\n; exit $LASTEXITCODE"
    );
    vec![
        "-NoProfile".to_string(),
        "-NonInteractive".to_string(),
        "-EncodedCommand".to_string(),
        base64_utf16le(&script),
    ]
}

/// 将脚本编码为 PowerShell `-EncodedCommand` 所需的 UTF-16LE Base64。
fn base64_utf16le(script: &str) -> String {
    use base64::Engine as _;
    let utf16: Vec<u16> = script.encode_utf16().collect();
    let bytes: Vec<u8> = utf16.iter().flat_map(|unit| unit.to_le_bytes()).collect();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// WSL 启动器的两类路径后缀：`C:\Windows\System32\bash.exe`（系统组件）与
/// `%LOCALAPPDATA%\Microsoft\WindowsApps\bash.exe`（Store 版别名）。二者都不是
/// Git Bash，命中即挂起或输出乱码，必须排除。
#[cfg(windows)]
const WSL_LAUNCHER_SUFFIXES: &[&str] = &["system32\\bash.exe", "windowsapps\\bash.exe"];

/// Git Bash 默认安装路径（不在 PATH 时的兜底），返回真实存在的第一个。
#[cfg(windows)]
fn git_bash_default_path() -> Option<String> {
    ["ProgramFiles", "ProgramFiles(x86)"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(|root| PathBuf::from(root).join("Git").join("bin").join("bash.exe"))
        // 用户级安装（Git for Windows 安装器的「仅为我安装」）。
        .chain(std::env::var_os("LOCALAPPDATA").map(|local| {
            PathBuf::from(local)
                .join("Programs")
                .join("Git")
                .join("bin")
                .join("bash.exe")
        }))
        .find(|path| path.is_file())
        .map(|path| path.to_string_lossy().into_owned())
}

/// `where.exe <name>` 的首个 PATH 命中（排除 `exclude_suffixes` 后缀命中，大小写不敏感）。
#[cfg(windows)]
fn where_first(name: &str, exclude_suffixes: &[&str]) -> Option<String> {
    let output = std::process::Command::new("where.exe").arg(name).output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .find(|line| {
            let lower = line.to_lowercase();
            !exclude_suffixes.iter().any(|suffix| lower.ends_with(suffix))
        })
        .map(str::to_string)
}

/// 一次性执行请求。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecRequest {
    pub dialect: ShellDialect,
    /// 命令行（单一字符串，由方言解释；不做 shell 词法解析）。
    pub command: String,
    /// 工作目录（None = 会话工作目录；越界由沙箱层拒绝）。
    pub cwd: Option<PathBuf>,
    pub timeout: Option<Duration>,
    /// 晋升后台任务时的归属会话（None = 纯一次性）。
    pub owner: Option<SessionId>,
}

/// 一次性执行结果（已晋升后台任务时 `job_id` 为 Some，正文输出走 `job_output`）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecOutcome {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    /// 输出是否被截断（完整内容见 spill 路径提示）。
    pub truncated: bool,
    /// 是否因超时被终止。
    pub timed_out: bool,
    /// 后台任务晋升后的任务标识。
    pub job_id: Option<JobId>,
    /// 命令是否被 OS 级沙箱拒绝（写越界/只读写；仅配置沙箱时可为 true）。
    pub sandbox_denied: bool,
}

/// 进程执行端口。实现方负责超时 kill、输出截断与 spill、退出码归一。
///
/// 异步方法用 RPITIT 显式声明 `Send` 约束（不用 `#[async_trait]`）。
pub trait ShellExecutor: Send + Sync {
    /// 一次性执行；`cancel` 触发时终止进程并以 `exit_code = -1` 返回。
    fn exec(
        &self,
        request: ExecRequest,
        cancel: tokio_util::sync::CancellationToken,
    ) -> impl std::future::Future<Output = Result<ExecOutcome, String>> + Send;
}

/// [`OneshotExecutor`] 即 [`ShellExecutor`] 的一次性实现：对每个请求 spawn 一个
/// fresh 子进程（program 与 argv 经 [`ShellDialect::spawn_plan`] 解析），处理超时、
/// 截断与 spill。
///
/// `run_in_background`（`owner` 非 None）在本实现中仅登记意图；后台进程晋升由
/// 更高层（jobs 控制面）承接，一次性路径始终返回同步结果。
impl ShellExecutor for oneshot::OneshotExecutor {
    fn exec(
        &self,
        request: ExecRequest,
        cancel: tokio_util::sync::CancellationToken,
    ) -> impl std::future::Future<Output = Result<ExecOutcome, String>> + Send {
        let ExecRequest { dialect, command, cwd, timeout, owner: _ } = request;
        async move { self.exec(dialect, &command, cwd.as_deref(), timeout, cancel).await }
    }
}

#[cfg(test)]
#[path = "oneshot_tests.rs"]
mod oneshot_tests;

#[cfg(test)]
#[path = "background_tests.rs"]
mod background_tests;
