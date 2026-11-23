//! shell 工具：`bash` + `pwsh`（两个独立工具，同 schema、不同方言）。
//!
//! 薄壳——参数 schema（description / command /
//! timeoutMs / workdir / run_in_background）与执行委派到 `shell_exec` seam。
//! 工具层不直接触碰 `std::process::Command`（沙箱与审批判定收敛在工具层，
//! 进程执行策略收敛在 `shell_exec`）。
//!
//! 执行路径（`run_in_background`）：
//! - 一次性（缺省 `run_in_background` 假）：`OneshotExecutor` 同步返回，输出
//!   截断 + spill；
//! - 后台（`run_in_background` 真）：`BackgroundExecutor` spawn 后台进程并返回
//!   任务 id，输出经 `job_output` 收集、`job_kill` 停止。
//!
//! 风险与模式：`risk()` 声明上界 `Escalating`（沙箱允许集之外的操作须人工审批），
//! 实际档位由 [`classify_shell_command`] 按命令内容降到 `ReadOnly` / `WorkspaceWrite`；
//! 仅在 `work` 模式下可见。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::{ToolPort, jobs::Owner};
use crate::approval::{SandboxLevel, ToolRisk};
use crate::listeners::ApprovalRequest;
use crate::session_cwd::SessionCwd;
use crate::shell_exec::background::BackgroundExecutor;
use crate::shell_exec::{oneshot::OneshotExecutor, ExecOutcome, ShellDialect};
use crate::types::{ContentBlock, ToolCall, ToolDefinition};
use crate::waterfall::BoxFuture;

/// 沙箱拒绝升级通道（Phase E-2，escalation 重试）：命令被 OS 沙箱 `denied`
/// 时，经审批请求通道挂起裁决；放行（`allowed-once` / `allowed-for-session`）则以
/// `danger-full-access` 一次性重放，拒绝则 fail-closed。审批裁决路径与
/// [`crate::listeners::ApprovalGate`] 共用同一 `requests` 发送端，保证单一裁决入口。
#[derive(Clone)]
pub struct SandboxEscalator {
    /// 审批请求发送端（Phase E-2 升级发起；与 ApprovalGate 同源）。
    pub requests: mpsc::UnboundedSender<ApprovalRequest>,
}

impl SandboxEscalator {
    /// 发起一次升级裁决：放行（一次性或本会话）返回 true，其余（拒绝/通道关闭）false。
    /// fail-closed：通道关闭视为拒绝。
    pub async fn escalate(&self, name: &str, input: &Value) -> bool {
        let (reply, receiver) = tokio::sync::oneshot::channel();
        let request = ApprovalRequest {
            name: name.to_string(),
            input: input.clone(),
            // 来源标记：让前端区分「沙箱拒绝后的越界申请」与准入 gate 的首次审批。
            reason: Some("sandbox-retry".to_string()),
            reply,
        };
        if self.requests.send(request).is_err() {
            return false; // 通道关闭 → fail-closed 拒绝
        }
        matches!(receiver.await, Ok(decision) if decision.is_allowed())
    }
}

/// bash：一次性或后台执行 POSIX 命令（fresh `bash -c`）。
pub struct BashTool {
    pub executor: Arc<OneshotExecutor>,
    /// 后台执行器（`run_in_background` 时使用；None = 后台不可用）。
    pub background: Option<Arc<BackgroundExecutor>>,
    /// 会话工作目录句柄（`workdir` 未指定时的默认 cwd；运行期取活动目录）。
    pub cwd: Arc<SessionCwd>,
    /// 后台任务归属会话（owner 隔离，装配后经 cell 接线）。
    pub owner: Owner,
    /// 沙箱拒绝升级通道（Phase E-2；None = 未配置沙箱，无升级联动）。
    pub escalator: Option<SandboxEscalator>,
}

/// pwsh：一次性或后台执行 PowerShell 命令（fresh `pwsh -c`，Windows 原生路径）。
pub struct PwshTool {
    pub executor: Arc<OneshotExecutor>,
    pub background: Option<Arc<BackgroundExecutor>>,
    pub cwd: Arc<SessionCwd>,
    pub owner: Owner,
    /// 沙箱拒绝升级通道（Phase E-2；None = 未配置沙箱，无升级联动）。
    pub escalator: Option<SandboxEscalator>,
}

impl BashTool {
    fn definition(&self) -> ToolDefinition {
        shell_definition("bash", "Execute a bash command and return its stdout/stderr. Each call runs \
            in a fresh bash process; pass `workdir` instead of using `cd`. Set `run_in_background` to \
            start a long-running job and collect output with `job_output`.")
    }
}

impl PwshTool {
    fn definition(&self) -> ToolDefinition {
        shell_definition("pwsh", "Execute a PowerShell command (`pwsh -Command`) and return its \
            stdout/stderr. Each call runs in a fresh pwsh process; pass `workdir` instead of using \
            `cd`. Paths use native Windows form (C:\\...). Set `run_in_background` to start a \
            long-running job and collect output with `job_output`.")
    }
}

/// 共享 schema 构造：description / command / timeoutMs / workdir / run_in_background。
fn shell_definition(name: &str, description: &str) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        description: description.to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "description": { "type": "string", "description": "Clear, concise description of what this command does in active voice, 5-10 words (shown in the UI)." },
                "command": { "type": "string", "description": "The shell command to execute." },
                "timeoutMs": { "type": "number", "description": "Timeout in milliseconds (one-shot only). The executor applies its configured default and cap." },
                "workdir": { "type": "string", "description": "Working directory for this command. Defaults to the session workspace." },
                "run_in_background": { "type": "boolean", "description": "Run in the background and return a job id immediately (collect with job_output, stop with job_kill)." }
            },
            "required": ["description", "command"]
        }),
    }
}

/// 共享执行体：解析参数 → 后台或一次性执行 → 合成模型可见结果。
async fn run_shell(
    executor: &OneshotExecutor,
    background: Option<(&BackgroundExecutor, &Owner)>,
    dialect: ShellDialect,
    cwd: Arc<SessionCwd>,
    escalator: Option<&SandboxEscalator>,
    call: &ToolCall,
    cancel: CancellationToken,
) -> Result<Vec<ContentBlock>, String> {
    let command = arg(&call.input, "command");
    if command.trim().is_empty() {
        return Err("command must be a non-empty string".to_string());
    }
    let run_in_background = call.input.get("run_in_background").and_then(Value::as_bool).unwrap_or(false);
    // 新进程使用会话活动目录；`workdir` 参数同样受可写根约束（权限根 + 系统临时
    // 目录，防 `../` 逃逸）。
    let workdir = match call.input.get("workdir").and_then(Value::as_str) {
        Some(relative) => cwd.resolve(relative)?,
        None => cwd.ensure(),
    };

    // 后台路径：无后台执行器或未归属会话时拒绝（后台晋升须 owner 锚定）。
    if run_in_background {
        let Some((background, owner)) = background else {
            return Err("run_in_background is not available in this deployment".to_string());
        };
        let owner = owner
            .get()
            .cloned()
            .ok_or_else(|| "run_in_background requires a session owner (not wired)".to_string())?;
        let job_id = background.spawn(dialect, &command, Some(workdir), owner)?;
        return Ok(vec![ContentBlock::Text {
            text: format!("Started background job {job_id}. Collect output with job_output, stop with job_kill."),
        }]);
    }

    // 一次性路径：超时 + 截断 + spill。
    let timeout = call
        .input
        .get("timeoutMs")
        .and_then(Value::as_f64)
        .map(|ms| Duration::from_millis(ms.max(0.0) as u64));
    let timeout = timeout.or(Some(dialect.default_timeout()));

    let mut outcome: ExecOutcome =
        executor.exec(dialect, &command, Some(&workdir), timeout, cancel.clone()).await?;
    // 沙箱拒绝→升级审批联动（Phase E-2，`bash-sandbox` 的 `denied` 标记 +
    // escalation 重试流程）：命令被 OS 级沙箱拒绝（写越界/只读写/越权）时，不静默吞掉，
    // 而是经审批升级——挂起人工 /approve（或交 reviewer 自动裁决）；`allowed-once` 则以
    // `danger-full-access` 重放该命令，拒绝则 fail-closed 报错。
    if outcome.sandbox_denied {
        // 未配置升级通道（未配置 OS 沙箱）：fallback 为 fail-closed 显式报错。
        let Some(escalator) = escalator else {
            return Err(format!(
                "sandbox: command denied under current sandbox policy — 该命令超出当前沙箱允许集，\
                 已按 fail-closed 拒绝执行（未产生副作用）。拒绝详情：{}",
                outcome.stdout
            ));
        };
        // 发起升级裁决；放行才重放——以一次性 `danger-full-access` 覆盖执行，
        // 不改写共享 preset（避免并发窗口与取消时的 fail-open）。
        if escalator.escalate(&call.name, &call.input).await {
            outcome = executor
                .exec_with_mode(
                    dialect,
                    &command,
                    Some(&workdir),
                    Some(SandboxLevel::DangerFullAccess),
                    timeout,
                    cancel,
                )
                .await?;
        } else {
            return Err(format!(
                "sandbox: command denied under current sandbox policy and rejected by operator — \
                 已按 fail-closed 拒绝执行（未产生副作用）。拒绝详情：{}",
                outcome.stdout
            ));
        }
    }
    Ok(vec![ContentBlock::Text { text: outcome.stdout }])
}

impl ToolPort for BashTool {
    fn definition(&self) -> ToolDefinition {
        self.definition()
    }

    fn modes(&self) -> Option<Vec<String>> {
        Some(vec!["work".to_string()])
    }

    fn risk(&self) -> ToolRisk {
        ToolRisk::Escalating
    }

    fn classify_risk(&self, call: &ToolCall) -> ToolRisk {
        let command = arg(&call.input, "command");
        let background = call.input.get("run_in_background").and_then(Value::as_bool).unwrap_or(false);
        // 后台执行器绕过 OS 沙箱（直起子进程），不能算沙箱允许集内 → 工作区写退回审批。
        let sandboxed = self.executor.sandbox_enabled() && !background;
        classify_shell_risk(&command, sandboxed)
    }

    fn execute(&self, call: ToolCall, cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let executor = self.executor.clone();
        let background = self.background.clone();
        let cwd = self.cwd.clone();
        let owner = self.owner.clone();
        let escalator = self.escalator.clone();
        Box::pin(async move {
            run_shell(&executor, background.as_deref().map(|b| (b, &owner)), ShellDialect::Bash, cwd, escalator.as_ref(), &call, cancel).await
        })
    }
}

impl ToolPort for PwshTool {
    fn definition(&self) -> ToolDefinition {
        self.definition()
    }

    fn modes(&self) -> Option<Vec<String>> {
        Some(vec!["work".to_string()])
    }

    fn risk(&self) -> ToolRisk {
        ToolRisk::Escalating
    }

    fn classify_risk(&self, call: &ToolCall) -> ToolRisk {
        let command = arg(&call.input, "command");
        let background = call.input.get("run_in_background").and_then(Value::as_bool).unwrap_or(false);
        // 后台执行器绕过 OS 沙箱（直起子进程），不能算沙箱允许集内 → 工作区写退回审批。
        let sandboxed = self.executor.sandbox_enabled() && !background;
        classify_shell_risk(&command, sandboxed)
    }

    fn execute(&self, call: ToolCall, cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let executor = self.executor.clone();
        let background = self.background.clone();
        let cwd = self.cwd.clone();
        let owner = self.owner.clone();
        let escalator = self.escalator.clone();
        Box::pin(async move {
            run_shell(&executor, background.as_deref().map(|b| (b, &owner)), ShellDialect::Pwsh, cwd, escalator.as_ref(), &call, cancel).await
        })
    }
}

/// 读参（缺省空串，由下游校验给出模型可见错误）。
fn arg(input: &Value, key: &str) -> String {
    input.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

/// 按命令内容分类 shell 命令的风险（「按操作边界判定」而非整体工具名）：
/// - 全段只读 → `ReadOnly`（任何预设下直通，无需人工审批）；
/// - 全段属于「只读 ∪ 工作区内写」且无越界重定向 → `WorkspaceWrite`（工作区写预设
///   下直通，越界写由 OS 沙箱兜底拒绝）；
/// - 命令替换 / 反引号 / 脚本块、网络访问、破坏性参数、未知命令 → `Escalating`。
///
/// 对照说明：命令本身不做内容级审批——命令默认在沙箱策略下
/// 直通，审批只在模型显式请求升级（`sandbox_permissions` + justification）时出现。
/// onion 的 `ReadOnly` / `WorkspaceWrite` 档以 OS 级沙箱（`onion-sandbox`）为前置
/// 条件：沙箱未就位时由 [`confine_to_sandbox`] 一并收敛为 `Escalating`（无强制
/// 遏制时免审批直通不成立，显式审批兜底，fail-closed）。
///
/// 判定流程：移除引号内容（防 `grep "a|b"` 被管道切分误判）→ 拒绝命令替换与
/// 脚本块（内容不可静态判定）→ 按 `&&` / `||` / `;` / `|` 切段 → **每段**独立
/// 校验（fd 复制与 null 重定向放行；无危险参数；首词命中白名单）。
pub fn classify_shell_command(command: &str) -> ToolRisk {
    let cmd = command.trim().to_lowercase();
    if cmd.is_empty() {
        return ToolRisk::Escalating;
    }
    let unquoted = strip_quoted(&cmd);
    // 命令替换 / 反引号 / 脚本块：内容不可静态判定，一律升级（fail-closed）。
    if unquoted.contains("$(") || unquoted.contains('`') || unquoted.contains('{') {
        return ToolRisk::Escalating;
    }
    let mut workspace_write = false;
    for segment in split_segments(&unquoted) {
        let Some(risk) = classify_segment(&segment) else {
            return ToolRisk::Escalating;
        };
        workspace_write |= risk == ToolRisk::WorkspaceWrite;
    }
    if workspace_write {
        ToolRisk::WorkspaceWrite
    } else {
        ToolRisk::ReadOnly
    }
}

/// 沙箱未就位时的档位收敛（fail-closed，confinement 缺失**拒绝执行**而非
/// 透传 argv）：`ReadOnly` 与 `WorkspaceWrite` 的免审批直通都以 OS 级沙箱兜底为
/// 前提——没有沙箱就等于无强制地自动执行命令，全部收敛为 `Escalating` 退回显式
/// 审批兜底。`danger-full-access` 预设不受影响（其沙箱允许集覆盖 `Escalating`，
/// 仍直通）。
/// `sandboxed = false` 的场景：未部署 `onion-sandbox`（含 Linux/macOS 未实现）、
/// 或命令走后台执行器（`run_in_background` 不经过沙箱）。
pub fn confine_to_sandbox(risk: ToolRisk, sandboxed: bool) -> ToolRisk {
    if sandboxed {
        return risk;
    }
    match risk {
        ToolRisk::ReadOnly | ToolRisk::WorkspaceWrite => ToolRisk::Escalating,
        other => other,
    }
}

/// 沙箱盲区判定：任一段命中网络命令（沙箱不做网络遏制，放行即裸奔外网/变更远端）。
/// 段切分与引号剥离复用静态分类的同款逻辑（引号内的 URL 等不参与判定）。
fn is_network_command(command: &str) -> bool {
    let unquoted = strip_quoted(&command.trim().to_lowercase());
    split_segments(&unquoted)
        .iter()
        .any(|seg| NETWORK_COMMANDS.iter().any(|entry| matches_entry(seg.trim(), entry)))
}

/// dsh web ui 对齐的 shell 风险判定（执行时强制优先，静态分类只补沙箱盲区）：
///
/// - **沙箱就位**（OS 级执行时遏制）：命令一律进沙箱直通（dsh 的编排——命令不做
///   字符串级前置审批）；未识别命令、命令替换 / 脚本块、文件重定向、危险参数
///   （`-delete` / `-exec`）等静态不可判定的情形按 [`ToolRisk::WorkspaceWrite`]
///   放行，越界写在运行时被 OS 沙箱拒绝后走既有的 `sandbox_denied` → 升级审批
///   流程。唯一保留的前置升级是**网络命令**——onion 沙箱不做网络遏制，放行即
///   无遏制的裸网络访问（对 dsh 的显式收窄，见模块文档）。
/// - **沙箱未就位**（未部署 `onion-sandbox` / Linux·macOS 未实现 / 后台执行器）：
///   维持 fail-closed——无强制遏制时免审批直通不成立，全部收敛为升级审批。
pub fn classify_shell_risk(command: &str, sandboxed: bool) -> ToolRisk {
    let static_risk = classify_shell_command(command);
    if !sandboxed {
        return confine_to_sandbox(static_risk, false);
    }
    if is_network_command(command) {
        return ToolRisk::Escalating;
    }
    match static_risk {
        // 沙箱可遏制：静态未识别 / 不可判定档交运行时强制，不再前置审批
        ToolRisk::Escalating => ToolRisk::WorkspaceWrite,
        other => other,
    }
}

/// 只读命令白名单（小写；单词条目匹配首词，含空格条目按前缀 + 词边界匹配）：
/// 覆盖 bash / pwsh 常用只读命令与纯转发型管道 cmdlet。迷你语言（awk / sed /
/// python）与脚本块类 cmdlet（foreach-object / where-object）**不在列**——其
/// 内容不可静态判定，保守升级。
const READONLY_COMMANDS: &[&str] = &[
    // 文件 / 目录读取
    "ls", "dir", "pwd", "get-location", "get-childitem", "gci", "cat", "type", "get-content",
    "gc", "head", "tail", "less", "more", "stat", "file", "wc", "du", "df", "tree", "find",
    // 文本检索 / 处理（纯读）
    "grep", "findstr", "rg", "select-string", "sls", "diff", "cut", "uniq", "sort-object",
    "select-object", "format-table", "format-list", "format-wide", "measure-object",
    // 输出 / 管道黑洞（pwsh 纯输出 cmdlet 与 echo 同类，无文件写入能力）
    "echo", "print", "write-output", "write-host", "out-null", "out-string", "out-host",
    // 纯计算 / 等待 / 路径解析（无副作用；只读且无文件写入能力）
    "sleep", "start-sleep", "true", "false", "test", "printf", "expr", "seq", "nproc", "uptime",
    "free", "basename", "dirname", "realpath", "readlink", "get-itemproperty", "split-path",
    "join-path", "compare-object",
    // git 只读子命令（多词前缀；`git` 单词不在列，写类子命令自然升级）
    "git status", "git diff", "git log", "git branch", "git remote", "git show",
    // 系统 / 进程信息
    "whoami", "hostname", "id", "groups", "date", "get-date", "uname", "printenv", "env",
    "ps", "get-process", "tasklist", "history", "man", "get-help", "get-command",
    "get-variable", "get-item", "test-path", "resolve-path", "which", "where", "where.exe",
    // 网络诊断
    "ping", "netstat", "ipconfig", "nslookup",
];

/// 工作区内写命令白名单（`WorkspaceWrite` 档的判据；匹配规则同只读白名单）：
/// 覆盖文件 / 目录创建与内容写入、常见构建与测试入口。这些命令的**写入目标由
/// OS 沙箱限制在工作区内**（越界写被拒并转升级审批），因此可免逐次审批。
/// 破坏性命令（`rm` / `mv` 覆盖 / `reg` …）与网络命令**不在列**。
const WORKSPACE_WRITE_COMMANDS: &[&str] = &[
    // 文件 / 目录创建与复制（跨目录搬移不在此列：`mv` 目标是工作区外时不可静态判定）
    "mkdir", "md", "touch", "cp", "copy", "ln", "new-item", "ni", "copy-item",
    // 内容写入（重定向已在上游拒绝；显式写入命令按路径由沙箱遏制）
    "set-content", "add-content", "out-file", "tee", "tee-object",
    // git 写类子命令（工作区内的索引 / 提交）
    "git add", "git commit", "git stash",
    // 构建 / 测试 / 代码生成（产物写入工作区内 target / dist 等目录）
    "cargo build", "cargo check", "cargo test", "cargo clippy", "cargo fmt", "cargo bench",
    "npm run", "npm test", "pnpm run", "pnpm test", "yarn run", "yarn test",
    "npx tsc", "tsc", "make",
];

/// 网络访问命令（命中即升级）：请求外网 / 变更远端状态，沙箱不做网络遏制，
/// 必须由人工确认后放行。
const NETWORK_COMMANDS: &[&str] = &[
    "curl", "wget", "ssh", "scp", "sftp", "ftp", "telnet", "nc", "ncat", "aria2c",
    "invoke-webrequest", "iwr", "invoke-restmethod", "irm", "start-bitstransfer",
    "npm install", "npm i", "npm ci", "npm add", "npm publish", "npm update",
    "pnpm install", "pnpm i", "pnpm add", "yarn install", "yarn add",
    "pip install", "pip3 install", "uv pip install", "uv add", "poetry add",
    "cargo install", "cargo add", "cargo publish", "cargo update", "cargo fetch",
    "git push", "git pull", "git fetch", "git clone", "git remote update",
    "go get", "go mod download", "gem install", "dotnet restore", "nuget",
    "docker pull", "docker push", "gh",
];

/// 危险参数模式（命中任一即升级，即使首词在白名单内——如 `find . -delete`）。
const DANGEROUS_PATTERNS: &[&str] = &["-delete", "-exec ", "-execdir "];

/// 移除引号包裹的内容（含引号本身替换为空格）：引号内的 `|` / `>` / 命令词不参与
/// 判定（如 `echo "x > y"` 是纯打印、`grep "a|b" f` 不被切分误判）。未配对引号按
/// 原文到尾全吞（内容不可判定，但结构上退化为前缀命令，仍走白名单校验）。
fn strip_quoted(cmd: &str) -> String {
    let mut out = String::with_capacity(cmd.len());
    let mut quote: Option<char> = None;
    for c in cmd.chars() {
        match quote {
            Some(q) if c == q => {
                quote = None;
                out.push(' ');
            }
            Some(_) => out.push(' '),
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                out.push(' ');
            }
            None => out.push(c),
        }
    }
    out
}

/// 按链式与管道分隔符切段：`&&` / `||` / `;` / `|`。顺序关键：`||` 先于 `|`，
/// 避免逻辑或被当管道切开。空段由下游跳过（分隔符连用的语法错误命令本就不可执行）。
fn split_segments(cmd: &str) -> Vec<String> {
    let mut segments = vec![cmd.to_string()];
    for sep in ["&&", "||", ";", "|"] {
        let mut next = Vec::new();
        for seg in &segments {
            next.extend(seg.split(sep).map(ToString::to_string));
        }
        segments = next;
    }
    segments
}

/// 单段风险判定：返回 `None` 表示该段不可静态放行（重定向写出、危险参数、网络
/// 访问、未知命令），整条命令升级审批。空段（分隔符连用的语法错误产物）按只读处理。
fn classify_segment(seg: &str) -> Option<ToolRisk> {
    let normalized = normalize_redirects(seg)?;
    let seg = normalized.trim();
    if seg.is_empty() {
        return Some(ToolRisk::ReadOnly);
    }
    if DANGEROUS_PATTERNS.iter().any(|pat| seg.contains(pat)) {
        return None;
    }
    // 网络命令先于白名单判定：`git push` 等虽以白名单首词开头也不可免审批。
    if NETWORK_COMMANDS.iter().any(|entry| matches_entry(seg, entry)) {
        return None;
    }
    if READONLY_COMMANDS.iter().any(|entry| matches_entry(seg, entry)) {
        return Some(ToolRisk::ReadOnly);
    }
    if WORKSPACE_WRITE_COMMANDS.iter().any(|entry| matches_entry(seg, entry)) {
        return Some(ToolRisk::WorkspaceWrite);
    }
    None
}

/// 白名单条目匹配：单词条目比首词；多词条目（如 `git status` / `cargo test`）按
/// 词边界前缀匹配，防 `git statusx` / `cargo testing` 之类误命中。
fn matches_entry(seg: &str, entry: &str) -> bool {
    if entry.contains(' ') {
        seg.strip_prefix(entry)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
    } else {
        seg.split_whitespace().next() == Some(entry)
    }
}

/// 处理段内重定向：fd 复制（`2>&1` / `1>&2`，无文件效果）与 null 黑洞目标
/// （`/dev/null` / `nul` / `$null`）放行；其余文件重定向（`>` / `>>` / `2>` / `&>`）
/// 返回 `None`（升级）。返回归一化后的段（重定向子句已移除）。
fn normalize_redirects(seg: &str) -> Option<String> {
    let mut cur = seg.replace("2>&1", " ").replace("1>&2", " ");
    while let Some(pos) = cur.find('>') {
        // `>>` 为双字符：吞掉连续的重定向符号后需再次 trim 首部空白。
        let rest = cur[pos + 1..].trim_start().trim_start_matches('>').trim_start();
        let target_end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let target = &rest[..target_end];
        if target != "/dev/null" && target != "nul" && target != "$null" {
            return None;
        }
        // 移除「重定向符号 + 目标」子句，继续检查剩余部分。
        let head = &cur[..pos];
        let tail = &rest[target_end..];
        cur = format!("{head} {tail}");
    }
    Some(cur)
}

#[cfg(test)]
#[path = "shell_tests.rs"]
mod tests;
