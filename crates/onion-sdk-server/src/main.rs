//! onion-sdk-server：onion SDK 层的 stdio JSON-RPC server。
//!
//! 薄协议适配器：引擎逻辑全在 `onion` crate，本二进制只做「JSON-RPC 帧 ↔ 引擎
//! 调用」映射。stdout 只承载协议帧；引擎日志走 stderr（`eprintln!`），保证
//! stdout 纯净（外部 SDK client 只读 stdout）。
//!
//! 方法集：
//! - 请求：`initialize` / `session/prompt` / `session/resume` / `session/wait`
//!   / `session/working-directory/get|set` / `shutdown`
//! - 审批回填：`approval/reply` / `plan/review/reply`
//! - 通知：`session.event` / `session.status` / `subagent.started` / `subagent.finished`
//!
//! `session/prompt` 只返回入队回执 `{message_id}`，结果靠 `session.event` 通知流
//! 观察（贴合 agent 长 turn）。

mod frame;
mod rpc;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::RwLock;

use onion::compaction::config::resolve_compact_spec_for;
use onion::compaction::listener::CompactionListener;
use onion::compaction::summarizer::SummaryConfig;
use onion::config::Config;
use onion::context::Context;
use onion::goal::driver::GoalRoundDriver;
use onion::goal::service::GoalService;
use onion::goal::tools::GoalToolSet;
use onion::kernel::TurnReport;
use onion::llm::OpenAiSseLlm;
use onion::plan::service::PlanMode;
use onion::plan::tools::ExitPlanModeTool;
use onion::runtime::Runtime;
use onion::sandbox::Sandbox;
use onion::system_prompt::{
    context_orders, section_orders, PromptContext, PromptSection, SectionText, SystemPrompt,
};
use onion::subagent::runtime::{ChildSession, SubagentRuntime};
use onion::tools::subagent::SubagentToolSet;
use onion::tools::{
    BashTool, EditTool, GlobTool, GrepTool, PwshTool, ReadImageTool, ReadTool, StrReplaceEditorTool,
    TodoWriteTool, ToolRegistry, WebFetchTool, WebSearchTool, WriteTool,
};
use onion::lsp::{LspService, LspTool};
use onion::ask_user::service::AskUserService;
use onion::ask_user::tools::AskUserQuestionTool;
use onion::present::PresentTool;
use onion::ralph::RalphTool;
use onion::schedule::service::ScheduleService;
use onion::schedule::storage::HostScheduleStore;
use onion::session_query::tools::SessionQueryTool;
use onion::shell_exec::oneshot::OneshotExecutor;
use onion::skill::catalog::SkillCatalogListener;
use onion::skill::filesystem::filesystem_provider;
use onion::skill::gesture::SkillGestureListener;
use onion::skill::registry::SkillRegistry;
use onion::skill::tools::SkillTool;
use onion::working_dir::WorkingDirectoryTool;
use onion::types::AgentMessage;
use rpc::{RpcError, code};
use serde_json::{Value, json};
use tokio::sync::{mpsc, oneshot};

use onion::listeners::{
    ApprovalGate, ApprovalRequest, EchoObserver, EnvInjector, Gatekeeper, RetryNudger,
};

/// 会话句柄：一个 session 对应一个 Runtime + 全部服务/通道（共享状态）。
struct SessionHandle {
    runtime: Arc<Runtime>,
    /// 在飞审批的一次性答复句柄（同一时刻至多一个；drop 即 fail-closed 拒绝）。
    pending_approval: tokio::sync::Mutex<Option<oneshot::Sender<onion::approval::ApprovalDecision>>>,
    /// 在飞 plan-review 的一次性答复句柄。
    pending_plan_review: tokio::sync::Mutex<Option<oneshot::Sender<onion::plan::service::PlanVerdict>>>,
    /// compaction `/compact` 信号发送端。
    compact_tx: mpsc::UnboundedSender<()>,
    /// compaction listener（空闲直驱 `compact_now`）。
    compaction_listener: Arc<CompactionListener>,
    /// 工具注册表（共享 ToolState，/agent 切换）。
    tools_handle: ToolRegistry,
    /// 当前 Agent id（/agent 查询与切换）。
    active_agent: Arc<tokio::sync::Mutex<String>>,
    /// 完整 Agent 目录（/agent 选择器：id 列表 + 解析）。
    agent_catalog: onion::preset::AgentCatalog,
    goal_driver: Arc<GoalRoundDriver>,
    plan_mode: Arc<PlanMode>,
    /// 忙标志（在飞轮次）。
    busy: Arc<AtomicBool>,
    /// turn 完成通知。
    done_tx: mpsc::UnboundedSender<TurnReport>,
    /// 工作目录（working-directory set）。
    working_dir: std::sync::Mutex<PathBuf>,
}

fn build_approval_config(
    preset_name: &str,
    llm: &Arc<OpenAiSseLlm>,
    model: &str,
) -> onion::approval::ApprovalConfig {
    let mut config = onion::approval::ApprovalConfig::for_preset(preset_name);
    if config.preset.name == "auto" {
        config.reviewer = Some(Arc::new(onion::approval::LlmReviewer::new(llm.clone(), model)));
    }
    config
}

/// 装配 Runtime + 全部 listener + 服务，返回 (共享 handle, 三个主循环 receiver)。
#[allow(clippy::type_complexity)]
fn assemble_session(
    config: Config,
    log_dir: PathBuf,
    schedule_store: Arc<HostScheduleStore>,
    session_id: Option<String>,
) -> std::io::Result<(
    Arc<SessionHandle>,
    mpsc::UnboundedReceiver<ApprovalRequest>,
    mpsc::UnboundedReceiver<onion::plan::service::PlanReviewRequest>,
    mpsc::UnboundedReceiver<TurnReport>,
)> {
    let mut prompts = SystemPrompt::new(&config);
    prompts
        .section(PromptSection {
            name: "onion:mission".to_string(),
            order: section_orders::TOOL_GUIDE,
            text: SectionText::Static(
                "你是 onion 的智能体，运行于洋葱模型内核，经 SDK 通道驱动。当前模型为 {{model}}。\
                 基础工具：read（读取沙箱内文本文件）、read_image（读取沙箱内图片并作为多模态块返回）。\
                 读文件按类型选工具：文本用 read，图片（png/jpg/jpeg/gif/webp）用 read_image；用错会被拒绝并提示改用另一个。\
                 write（整体覆盖）、edit（按字面量替换）、str_replace_editor（view / create / str_replace / insert）用于写改文件；覆盖既有文件前须先用 read 读取（read-before-write 策略），新建文件不受此约束。\
                 工具面由当前 Agent 决定：内置 coding（缺省）与 work，可经 agents.yml 扩展；调用 Agent 工具面外的工具会报错，改用面内等价工具或提示用户切换 Agent。\
                 审批：默认 workspace-write 预设下工作区内写入直通，不逐次审批；shell / 终端等升级操作挂起等待客户端 approve/deny 复决（auto 预设由模型复审），可能被批准或拒绝。\
                 大文本策略：单次输出受 token 上限约束，超长内容（如长文、大文件）必须分段读写——写入时用 write 分多次追加/分段写入，读取时长文件分段读取，切勿在一次工具调用参数中塞入超出输出上限的超大文本。\
                 需要时主动调用工具完成任务。"
                    .to_string(),
            ),
            interpolate: true,
            complete: false,
        })
        .unwrap_or_else(|error| panic!("onion:mission 段注册失败: {error}"));
    // per-tool 文件策略段（read/write/edit）：工具恒注册，段文本恒定。
    prompts
        .section(PromptSection {
            name: "tool:read".to_string(),
            order: section_orders::TOOL_READ,
            text: SectionText::Static(
                "Use the read tool — not shell commands like cat, sed, head, or tail — to inspect \
                 text files. Use offset and limit to read a line range or continue a large file."
                    .to_string(),
            ),
            interpolate: false,
            complete: false,
        })
        .unwrap_or_else(|error| panic!("tool:read 段注册失败: {error}"));
    // write 段：edit 恒注册，取挂载变体文本。
    prompts
        .section(PromptSection {
            name: "tool:write".to_string(),
            order: section_orders::TOOL_WRITE,
            text: SectionText::Static(
                "Read an existing file before overwriting it with write (the default \
                 fs-observation-policy requires it) and prefer edit for targeted changes."
                    .to_string(),
            ),
            interpolate: false,
            complete: false,
        })
        .unwrap_or_else(|error| panic!("tool:write 段注册失败: {error}"));
    prompts
        .section(PromptSection {
            name: "tool:edit".to_string(),
            order: section_orders::TOOL_EDIT,
            text: SectionText::Static(
                "Read a file before editing it (the default fs-observation-policy requires \
                 it), unless you just created or edited it in this session."
                    .to_string(),
            ),
            interpolate: false,
            complete: false,
        })
        .unwrap_or_else(|error| panic!("tool:edit 段注册失败: {error}"));
    // shell 执行策略（bash / pwsh）：工具恒注册，段文本恒定。
    prompts
        .section(PromptSection {
            name: "tool:bash".to_string(),
            order: section_orders::TOOL_BASH,
            text: SectionText::Static(
                "Check the [exit code: N] marker on every bash result; investigate failures \
                 before moving on."
                    .to_string(),
            ),
            interpolate: false,
            complete: false,
        })
        .unwrap_or_else(|error| panic!("tool:bash 段注册失败: {error}"));
    prompts
        .section(PromptSection {
            name: "tool:pwsh".to_string(),
            order: section_orders::TOOL_PWSH,
            text: SectionText::Static(
                "Non-zero exits are reported as `[exit code: N]` markers; investigate failures \
                 before moving on. On Windows a killed process settles as `[exit code: 1]` \
                 without a signal marker; treat a bare exit 1 after an interruption as a \
                 termination, not a command failure."
                    .to_string(),
            ),
            interpolate: false,
            complete: false,
        })
        .unwrap_or_else(|error| panic!("tool:pwsh 段注册失败: {error}"));
    // 后台任务控制面策略段（tool:jobs）。
    prompts
        .section(PromptSection {
            name: "tool:jobs".to_string(),
            order: section_orders::TOOL_JOBS,
            text: SectionText::Static(
                "Track every background job id you start. You are notified in-session when a \
                 job finishes — do not busy-poll or sleep on one; keep working on independent \
                 steps and do not duplicate a running job's work. Before giving a final answer, \
                 collect every still-relevant job with job_output (set wait: true only when you \
                 are genuinely blocked on it), and job_kill jobs that stopped mattering."
                    .to_string(),
            ),
            interpolate: false,
            complete: false,
        })
        .unwrap_or_else(|error| panic!("tool:jobs 段注册失败: {error}"));
    // LSP 策略段（lsp）。
    prompts
        .section(PromptSection {
            name: "tool:lsp".to_string(),
            order: section_orders::TOOL_LSP,
            text: SectionText::Static(onion::lsp::tools::LSP_PROMPT_TEXT.to_string()),
            interpolate: false,
            complete: false,
        })
        .unwrap_or_else(|error| panic!("tool:lsp 段注册失败: {error}"));
    // 终端策略段（terminal_* 6 件）。
    prompts
        .section(PromptSection {
            name: "tool:pty".to_string(),
            order: section_orders::TOOL_PTY,
            text: SectionText::Static(
                "Use a terminal session only when work needs persistent terminal state or \
                 interactive stdin; prefer shell/read/write/edit for bounded one-shot \
                 operations. Track every terminal session id and close sessions that no \
                 longer matter. An inferred_idle or timeout result does not prove the \
                 foreground command exited."
                    .to_string(),
            ),
            interpolate: false,
            complete: false,
        })
        .unwrap_or_else(|error| panic!("tool:pty 段注册失败: {error}"));
    // git worktree 策略段（create_worktree）。
    prompts
        .section(PromptSection {
            name: "onion:worktree-policy".to_string(),
            order: section_orders::WORKTREE_POLICY,
            text: SectionText::Static(
                "\
The create_worktree tool creates a new Git branch and worktree from a local commit, branch, \
or tag, then changes this session's working directory to it. It defaults to HEAD and a generated \
name. Uncommitted files stay in the source checkout. Existing names fail. Use working_directory \
with cd to leave; the checkout and branch remain."
                    .to_string(),
            ),
            interpolate: false,
            complete: false,
        })
        .unwrap_or_else(|error| panic!("onion:worktree-policy 段注册失败: {error}"));
    // workspace 依赖策略段（load_workspace_dependencies）。
    prompts
        .section(PromptSection {
            name: "onion:workspace-deps-policy".to_string(),
            order: section_orders::WORKSPACE_DEPS_POLICY,
            text: SectionText::Static(
                "\
The load_workspace_dependencies tool returns absolute paths to bundled Python and library \
directories, plus bundled Python distribution versions. Node.js and pnpm paths are included \
when the payload provides them. Python includes numpy, pandas, python-docx, python-pptx, \
openpyxl, Pillow, lxml, and XlsxWriter. Use these libraries for Office files unless the user \
or workspace instructions select another environment. When Node.js and pnpm paths are \
returned, run pnpm with that Node executable and pnpm script path. This does not change PATH \
or package-manager settings."
                    .to_string(),
            ),
            interpolate: false,
            complete: false,
        })
        .unwrap_or_else(|error| panic!("onion:workspace-deps-policy 段注册失败: {error}"));
    // 记忆管理策略（onion 自研，蓝本 ava-harness::memory；order 2500）。
    prompts
        .section(PromptSection {
            name: "onion:memory-policy".to_string(),
            order: section_orders::MEMORY_POLICY,
            text: SectionText::Static(
                "\
Memory tools let you persist and recall knowledge across sessions. Memory types are \
closed: user, feedback, project, reference. Save a memory with SaveMemory (writes \
<topic>.md plus one index line in MEMORY.md), update with UpdateMemory, and delete with \
DeleteMemory. When a task clearly matches an existing memory, load it with ReadMemory \
before acting. Use memory_search for keyword lookup when unsure of the exact topic \
name, and RecallMemory at task start to auto-surface relevant memories. Keep the \
MEMORY.md index concise — one line per memory."
                    .to_string(),
            ),
            interpolate: false,
            complete: false,
        })
        .unwrap_or_else(|error| panic!("onion:memory-policy 段注册失败: {error}"));
    // 团队策略（teamPolicy 段文本）。仅 agent-team
    // 启用时注册；interrupt=false 变体对应 onion 无 interrupt_agent 工具，
    // 拼接处取不含 and interrupt_agent 的文本。
    if config.agent_team.is_some() {
        prompts
            .section(PromptSection {
                name: "onion:team-policy".to_string(),
                order: section_orders::TEAM_POLICY,
                text: SectionText::Static(
                    "\
Agent Teams is available in this session

The Team Lead and all teammates share the same working directory and filesystem. Edits are \
immediately visible to every member. Split write work into disjoint scopes, record expected \
write scopes on shared tasks, and use task dependencies when work must be ordered. Write-scope \
overlap is advisory, not a lock.

Prefer read/edit/write for file changes. If a file operation returns FS_STALE_VERSION, read \
the current file, rebase your intended change onto the new content, and retry. Bash, \
formatters, code generators, and scripts are not fully protected by the filesystem version \
guard; coordinate them explicitly and have the Lead review the final diff and run tests.

Use the target returned by spawn_teammate or list_agents for send_message, or as owner when \
assigning or filtering shared tasks. send_message steers a running target at its nearest \
step boundary and starts or resumes an inactive target. inactive means no turn is \
executing; it does not describe task completion, success, failure, or waiting for other \
agents. provisioning means member creation is in progress; failed means member creation \
failed. A peer message starts with its sender name. A successful send does not mean the \
recipient has processed it. Shared-task workflow is list, get, claim with the current \
revision, perform the work, then complete. Task readiness never starts an owner. Before \
wait_agent, use list_agents and make sure another required member is running or \
provisioning; use send_message first when the required member is inactive. wait_agent \
observes only changes after that call starts, never wakes a member, and returns noProgress \
immediately when no other member can produce a change. Re-list after wakeup or timeout. The \
Lead must wait for required teammates before giving the final answer."
                        .to_string(),
                ),
                interpolate: false,
                complete: false,
            })
            .unwrap_or_else(|error| panic!("onion:team-policy 段注册失败: {error}"));
    }
    {
        let model = config.model.clone();
        prompts
            .variable("model", Arc::new(move || Some(model.clone())))
            .unwrap_or_else(|error| panic!("model 变量注册失败: {error}"));
    }
    prompts
        .context(PromptContext {
            name: "runtime_clock".to_string(),
            order: context_orders::RUNTIME_CLOCK,
            text: SectionText::Dynamic(Arc::new(|| {
                format!("session_time_epoch_ms={}", onion::session::epoch_millis())
            })),
            interpolate: false,
            required: true,
        })
        .unwrap_or_else(|error| panic!("runtime_clock 上下文注册失败: {error}"));

    let root = std::env::current_dir()?;
    // 会话目录单一权威：权限根恒定，cwd 可在根内移动。
    let session_cwd = Arc::new(onion::session_cwd::SessionCwd::new(root.clone()));
    let sandbox = Arc::new(Sandbox::with_cwd(session_cwd.clone()));
    // read-before-write 观测表（fs-observation-policy）：会话级、不跨会话持久化。
    let observation = Arc::new(onion::fs_observation::FsObservation::new());
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(ReadTool { sandbox: sandbox.clone(), observation: observation.clone() }));
    tools.register(Arc::new(ReadImageTool { sandbox: sandbox.clone(), observation: observation.clone() }));
    tools.register(Arc::new(WriteTool { sandbox: sandbox.clone(), observation: observation.clone() }));
    tools.register(Arc::new(EditTool { sandbox: sandbox.clone(), observation: observation.clone() }));
    tools.register(Arc::new(StrReplaceEditorTool { sandbox: sandbox.clone(), observation }));
    // shell：bash / pwsh（Escalating；只读命令经 classify_risk 降为 ReadOnly 直通；
    // 一次性 fresh 进程，输出 spill；
    // run_in_background 晋升后台任务，job_* 控制面）。
    let job_registry = Arc::new(onion_jobs::JobRegistry::new());
    let owner = onion::tools::jobs::owner_cell();
    let background =
        Arc::new(onion::shell_exec::background::BackgroundExecutor::new(job_registry.clone()));
    {
        let spill_dir = onion::paths::logs_dir(&onion::paths::data_home()).join("jobs");
        let executor = Arc::new(OneshotExecutor::new(Some(spill_dir)));
        tools.register(Arc::new(BashTool {
            executor: executor.clone(),
            background: Some(background.clone()),
            cwd: session_cwd.clone(),
            owner: owner.clone(),
            escalator: None,
        }));
        tools.register(Arc::new(PwshTool {
            executor,
            background: Some(background.clone()),
            cwd: session_cwd.clone(),
            owner: owner.clone(),
            escalator: None,
        }));
    }
    // jobs 控制面：job_list / job_output / job_kill（job_kill 为 Escalating）。
    {
        let (job_list, job_output, job_kill) =
            onion::tools::jobs::jobs_tools(job_registry.clone(), owner.clone());
        tools.register(Arc::new(job_list));
        tools.register(Arc::new(job_output));
        tools.register(Arc::new(job_kill));
    }
    // 文件发现工具：glob / grep（只读；纯 Rust 递归 + regex，无 rg 依赖）。
    tools.register(Arc::new(GlobTool::new(sandbox.clone())));
    tools.register(Arc::new(GrepTool::new(sandbox.clone())));
    // todo_write：整表替换任务清单（session-owned durable `todo/write` 事件）。
    let todo_tool = Arc::new(TodoWriteTool::new(config.allow_parallel_in_progress));
    tools.register(todo_tool.clone());
    // lsp：精确代码导航（只读；seam 可注册 provider，缺省无 provider，返回 LSP_UNAVAILABLE）。
    let lsp_service = Arc::new(LspService::new());
    tools.register(Arc::new(LspTool::with_cwd(lsp_service, session_cwd.clone())));
    // terminal_* 6 件：持久 PTY 会话（terminal_open / terminal_send / terminal_read /
    // terminal_signal / terminal_close / terminal_list）。写/交互为 Escalating（仅
    // coding 预设工具面；work 预设不含 terminal），读为 ReadOnly。
    // owner 隔离 + dispose 清理。
    let terminal_service = Arc::new(onion::terminal::TerminalService::new());
    terminal_service
        .register_backend(Arc::new(onion::terminal::PtyBackend::new()))
        .unwrap_or_else(|e| panic!("terminal backend 注册失败: {}", e.message));
    let terminal_tools = onion::terminal::TerminalToolSet::new(terminal_service)
        .with_background_enabled(config.terminal_enable_background);
    terminal_tools.register_all(&mut tools);
    // create_worktree：从本地 commit/branch/tag 建 git worktree 并切会话 cwd
    // （WorkspaceWrite）。
    let worktree_service = Arc::new(onion::worktree::WorktreeService::with_cwd(session_cwd.clone()));
    tools.register(Arc::new(onion::worktree::CreateWorktreeTool::new(
        worktree_service,
    )));
    // load_workspace_dependencies：查询捆绑运行时的绝对路径与版本（只读）。
    // source 未配置时工具仍注册，执行期结构化失败。
    let workspace_deps_service = Arc::new(onion::workspace_deps::WorkspaceDepsService::new(
        config.workspace_deps_source.clone().unwrap_or_default(),
    ));
    tools.register(Arc::new(onion::workspace_deps::LoadWorkspaceDependenciesTool::new(
        workspace_deps_service,
    )));
    // memory 工具 7 件（onion 自研记忆管理，蓝本 ava-harness::memory）：主会话
    // 5 件（SaveMemory / UpdateMemory / RecallMemory / ReadMemory / memory_search）
    // + agent 侧 2 件（ListMemories / DeleteMemory），去重并集见
    // `session_memory_tools`。存储根为
    // `data_home()/projects/<cwd 指纹>/memory/`，除 DeleteMemory 为
    // WorkspaceWrite 外全部 ReadOnly。
    {
        let store = onion::memory::MemoryStore::new(onion::memory::MemoryPaths::resolve(&root));
        for tool in onion::memory::session_memory_tools(store) {
            tools.register(tool);
        }
    }
    // web 工具：web_search（可配置 provider，缺省执行期结构化失败）/ web_fetch
    // （reqwest 直连）。均只读。
    tools.register(Arc::new(WebSearchTool::new(
        config.web_search_base_url.clone(),
        config.web_search_model.clone(),
        config.web_search_max_results,
        config.web_search_max_queries,
    )));
    tools.register(Arc::new(WebFetchTool::new(config.web_fetch_max_output_chars)));
    // session_* 5 件：会话历史查询（只读；共享接线槽，Runtime 装配后 wire）。
    let session_query = Arc::new(SessionQueryTool::new(config.session_query_max_results));
    for tool in session_query.define_all() {
        tools.register(tool);
    }
    // present：声明已有文件为最终交付物（只读；session-owned durable 事件）。
    let present_tool = Arc::new(PresentTool::new(sandbox, config.present_max_files));
    tools.register(present_tool.clone());
    // skill：加载技能指令（只读；注册表与 catalog / gesture listener 共享同一实例）。
    // 禁用策略（skills-manager）：`<projectRoot>/.onion/skills-manager.json` disabled 名单。
    let skill_registry = Arc::new(std::sync::Mutex::new(
        SkillRegistry::new().with_disable_policy(Arc::new(onion::skill::FileDisablePolicy)),
    ));
    {
        let mut reg = skill_registry
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        reg.register_provider(filesystem_provider())
            .unwrap_or_else(|e| panic!("skill filesystem provider 注册失败: {e}"));
    }
    let skill_tool =
        Arc::new(SkillTool::new(skill_registry.clone()).with_session_cwd(session_cwd.clone()));
    tools.register(skill_tool.clone());
    // working_directory：读 / 改会话工作目录（只读；session-owned durable 事件）。
    let working_dir_tool = Arc::new(WorkingDirectoryTool::with_cwd(session_cwd.clone()));
    tools.register(working_dir_tool);
    // schedule_* 4 件：durable 提醒（Host 独立存储，任务比 Session 活得久）。
    let schedule_service = Arc::new(ScheduleService::new(schedule_store));
    let schedule_tools =
        onion::schedule::tools::ScheduleToolSet::new(schedule_service);
    schedule_tools.register_all(&mut tools);
    // MCP 资源 3 件 + 工具桥接：server 自动发现（data_home/mcp.json →
    // <workspace>/.onion/mcp.json → ONION_MCP_SERVERS），桥接工具在装配期调
    // `tools/list` 动态注册为 `mcp__<server>__<tool>`。
    let mcp_resource_service = Arc::new(onion::mcp_resources::McpResourceService::new());
    let (mcp_servers, mut mcp_discovery_notes) = onion::mcp_resources::discover_servers(
        root.as_path(),
        &onion::paths::data_home(),
        config.mcp_servers_config.as_deref(),
    );
    for server in &mcp_servers {
        let name = server.server_name().to_string();
        let provider = Arc::new(onion::mcp_resources::McpClientProvider::new(server.clone()));
        mcp_resource_service
            .register(&name, provider)
            .unwrap_or_else(|e| panic!("MCP server {name} 注册失败: {e}"));
    }
    let mcp_bridge_tools = onion::mcp_resources::McpBridgeToolSet::new(mcp_servers);
    let (mcp_bridge_names, bridge_notes) = mcp_bridge_tools.register_all(&mut tools);
    mcp_discovery_notes.extend(bridge_notes);
    onion::mcp_resources::McpResourceToolSet::new(mcp_resource_service)
        .register_all(&mut tools);
    for note in &mcp_discovery_notes {
        eprintln!("[sdk-server] MCP: {note}");
    }

    let llm = Arc::new(OpenAiSseLlm::new(config.base_url.clone(), config.api_key.clone()));

    // subagent Runtime（进程单例）。
    // Agent 目录：子代理 `agent_preset` 白名单与工具面 / persona 的来源。
    let preset_catalog = onion::preset::AgentCatalog::merged(
        config.agents_dir.as_deref().map(|d| d.join("agents.yml")).as_deref(),
    )
    .unwrap_or_else(|e| panic!("Agent 目录加载失败: {e}"));
    let preset_choices = preset_catalog.ids();
    let inherited_preset = config.agent_preset.clone();
    let subagent_runtime = Arc::new(SubagentRuntime::new(
        1,
        config.clone(),
        llm.clone(),
        {
            let subagent_cwd = session_cwd.clone();
            let child_config = config.clone();
            let child_catalog = preset_catalog;
            let child_inherited_preset = inherited_preset;
            Arc::new(move |delegated_cwd: Option<&str>, preset: Option<&str>| {
                // 委派 cwd：相对路径基于父会话当前目录解析，须落在权限根内。
                let child_session_cwd = match delegated_cwd {
                    Some(raw) => {
                        let resolved = subagent_cwd.resolve(raw)?;
                        if !resolved.is_dir() {
                            return Err(format!("subagent cwd does not exist: {raw}"));
                        }
                        Arc::new(onion::session_cwd::SessionCwd::new_at(
                            subagent_cwd.root().to_path_buf(),
                            resolved,
                        ))
                    }
                    None => subagent_cwd.clone(),
                };
                let sandbox = Arc::new(Sandbox::with_cwd(child_session_cwd));
                let mut registry = ToolRegistry::new();
                registry.register(Arc::new(ReadTool {
                    sandbox,
                    observation: Arc::new(onion::fs_observation::FsObservation::new()),
                }));
                // 显式 `agent_preset` 优先，否则继承父会话预设。
                let preset_id = preset.unwrap_or(&child_inherited_preset);
                let definition = child_catalog.get(preset_id).ok_or_else(|| {
                    format!(
                        "unknown agent preset `{preset_id}` (available: {})",
                        child_catalog.ids().join(", ")
                    )
                })?;
                registry.state().set_agent(definition.tools.clone());
                let directive = (!definition.system_prompt.is_empty())
                    .then_some(definition.system_prompt.as_str());
                Ok(ChildSession::with_directive(&child_config, registry, directive))
            })
        },
        Some(log_dir.clone()),
    ));
    // 模型目录（list_subagent_models 数据源；providers.json / 内嵌表）。
    let provider_catalog = onion::provider_catalog::ProviderCatalog::load(
        &onion::paths::data_home(),
        &config.model,
    );
    // 父会话当前路由（无白名单时子代理显式选择只允许它）。
    let parent_route = provider_catalog
        .route_for(&config.model, &config.base_url)
        .unwrap_or_else(|| {
            onion::subagent::model_selection::AllowedModelRoute::new("local", &config.model)
        });
    let subagent_tools = SubagentToolSet::new(
        subagent_runtime.clone(),
        provider_catalog,
        parent_route,
        preset_choices,
    );
    // agent-team 启用时（ONION_AGENT_TEAM=1）共享名三件让位给 Team 语义，
    // 故 subagent 控制三件不注册。
    let mut team_service: Option<Arc<onion::agent_team::service::TeamService>> = None;
    let mut team_tools: Option<onion::agent_team::TeamToolSet> = None;
    if let Some(team_config) = config.agent_team {
        subagent_tools.register_all_team_compat(&mut tools);
        let service = Arc::new(
            onion::agent_team::service::TeamService::new(team_config, subagent_runtime.clone())
                .unwrap_or_else(|e| panic!("agent-team 配置校验失败: {e}")),
        );
        let tt = onion::agent_team::TeamToolSet::new(service.clone());
        tt.register_all(&mut tools);
        team_service = Some(service);
        team_tools = Some(tt);
    } else {
        subagent_tools.register_all(&mut tools);
    }
    // ralph：固定前景 fresh-agent 循环（复用 subagent runtime 逐轮驱动全新 child）。
    tools.register(Arc::new(RalphTool::new(subagent_runtime.clone(), 256)));

    let goal_service = Arc::new(GoalService::new());
    let goal_tools = GoalToolSet::new(goal_service.clone());
    goal_tools.register_all(&mut tools);

    // ask_user：阻塞问询 seam。SDK 协议当前无人工
    // 问询 RPC，通道 drain 以「无人工通道」答复 fail-closed 回填，避免工具永久挂起。
    let ask_user_service = Arc::new(AskUserService::new());
    let (ask_user_tx, mut ask_user_rx) = mpsc::unbounded_channel();
    ask_user_service.set_request_channel(ask_user_tx);
    tools.register(Arc::new(AskUserQuestionTool::new(ask_user_service.clone())));
    tokio::spawn(async move {
        while let Some(request) = ask_user_rx.recv().await {
            let answer = onion::ask_user::types::AskUserAnswer {
                answers: request
                    .questions
                    .iter()
                    .map(|question| onion::ask_user::types::AskUserAnswerItem {
                        id: question.id.clone(),
                        selected: Vec::new(),
                        custom: Some(
                            "本部署无人工问询通道，请基于现有信息继续或停止并说明阻塞点。"
                                .to_string(),
                        ),
                    })
                    .collect(),
            };
            let _ = request.reply.send(answer);
        }
    });

    // Agent 目录 + 工具面白名单（与 assemble.rs 同源逻辑）：按 config.agent_preset
    // 选定当前 Agent，设置工具面（工具面由 Agent 工具名列表决定）。
    let agent_catalog = onion::preset::AgentCatalog::merged(
        config.agents_dir.as_deref().map(|d| d.join("agents.yml")).as_deref(),
    )
    .unwrap_or_else(|e| panic!("Agent 目录加载失败: {e}"));
    let agent = agent_catalog
        .get(&config.agent_preset)
        .cloned()
        .unwrap_or_else(|| {
            panic!(
                "未知 Agent id `{}`（可选：{}）",
                config.agent_preset,
                agent_catalog.ids().join(", ")
            )
        });
    // MCP 桥接工具名是装配期 `tools/list` 动态发现的，无法写进静态 Agent 预设
    // 清单；不并入工具面白名单则 `in_agent()` 过滤会让它们永远不可见。
    let mut agent_tools = agent.tools;
    agent_tools.extend(mcp_bridge_names);
    tools.state().set_agent(agent_tools);
    let active_agent_id = agent.id;

    let plan_mode = Arc::new(PlanMode::new());
    let (plan_review_tx, plan_review_rx) = mpsc::unbounded_channel();
    plan_mode.set_review_channel(plan_review_tx);
    tools.register(Arc::new(ExitPlanModeTool::new(plan_mode.clone())));
    {
        let plan_mode = plan_mode.clone();
        prompts
            .section(PromptSection {
                name: "plan:policy".to_string(),
                order: section_orders::PLAN_POLICY,
                text: SectionText::Dynamic(Arc::new(move || {
                    if !plan_mode.active() {
                        return String::new();
                    }
                    "\
You are in plan mode. Produce a complete execution plan as markdown, starting with a # heading. \
Do NOT call tools or take actions yet — plan only. \
When the plan is complete submit it for approval with the exit_plan_mode tool."
                        .to_string()
                })),
                interpolate: false,
                complete: false,
            })
            .unwrap_or_else(|error| panic!("plan:policy 段注册失败: {error}"));
    }

    let context = Context { system: None, messages: Vec::new(), tools: tools.definitions() };

    let spec = resolve_compact_spec_for(&config, &config.model)
        .unwrap_or_else(|error| panic!("compact spec 派生失败: {error}"));
    let (compaction_listener, compact_tx) = CompactionListener::new(
        llm.clone(),
        spec,
        SummaryConfig { model: config.model.clone(), max_tokens: config.max_tokens, temperature: config.temperature },
        tools.definitions(),
    );
    let compaction_listener = Arc::new(compaction_listener);

    let approval_config = Arc::new(RwLock::new(build_approval_config(
        &config.approval_preset,
        &llm,
        &config.model,
    )));
    // 风险分类器：按调用内容判定（shell 读/写命令区分）。
    let classify_registry = tools.clone();
    // SDK server 未配置 OS 沙箱（无 Phase E-2 升级联动），审批通道仅由 ApprovalGate 使用。
    let (approval_tx, approval_rx) = tokio::sync::mpsc::unbounded_channel();
    let approval_gate = ApprovalGate::new(
        approval_config,
        Arc::new(move |call: &onion::types::ToolCall| classify_registry.classify(call)),
        approval_tx,
    );
    let approval_gate = Arc::new(approval_gate);

    let tools_handle = tools.clone();
    let listeners = || {
        vec![
            Arc::new(EchoObserver) as Arc<dyn onion::listener::Listener>,
            Arc::new(EnvInjector::with_session_cwd(session_cwd.clone()))
                as Arc<dyn onion::listener::Listener>,
            Arc::new(RetryNudger) as Arc<dyn onion::listener::Listener>,
            Arc::new(Gatekeeper::new()) as Arc<dyn onion::listener::Listener>,
            // skill 手势 listener 先于 catalog：gesture 最外层，瀑布把
            // catalog 承载的列表交给它扩展。
            Arc::new(
                SkillGestureListener::new(skill_registry.clone())
                    .with_session_cwd(session_cwd.clone()),
            ) as Arc<dyn onion::listener::Listener>,
            Arc::new(
                SkillCatalogListener::new(skill_registry.clone()).with_session_cwd(session_cwd.clone()),
            ) as Arc<dyn onion::listener::Listener>,
            approval_gate.clone() as Arc<dyn onion::listener::Listener>,
            compaction_listener.clone() as Arc<dyn onion::listener::Listener>,
        ]
    };
    // 子代理模型选择白名单（config 随后被 move 进 Runtime，先取一份）。
    let subagent_model_selection = config.subagent_model_selection.clone();
    let runtime = match session_id {
        Some(sid) => {
            let mut rt = Runtime::resume(config, prompts, context, &log_dir, &sid, llm, tools)?;
            for listener in listeners() {
                rt = rt.with_listener(listener);
            }
            Arc::new(rt)
        }
        None => {
            let mut rt = Runtime::new(config, prompts, context, &log_dir, llm, tools)?;
            for listener in listeners() {
                rt = rt.with_listener(listener);
            }
            Arc::new(rt)
        }
    };
    eprintln!("[sdk-server] 会话日志目录：{}", log_dir.display());

    // 后台任务接线：owner 锚定会话 id + 完成通知注入父会话 inbox。
    onion::tools::jobs::wire_owner(&owner, runtime.session().session_id());
    // terminal owner 接线：会话 id 与 jobs 共用（owner 隔离 + dispose 清理）。
    terminal_tools.wire_owner(runtime.session().session_id());
    terminal_tools.wire_background(job_registry);
    background.wire_inbox(runtime.inbox());

    // 子代理模型选择白名单：首个模型请求之前落 durable 事件；必须先于
    // wire_parent（后者从日志投影白名单）。
    if let Err(error) = onion::subagent::model_selection::seed_policy(
        &runtime.session(),
        subagent_model_selection.as_deref(),
    ) {
        panic!("子代理模型选择白名单写入失败: {error}");
    }
    subagent_tools.wire_parent(runtime.inbox(), runtime.session());
    // agent-team 延迟接线：注入 Lead session id + durable append 函数
    //（写 Lead SessionLog 的 TeamMemberChange/TeamTaskChange 事件）。
    if let (Some(service), Some(tt)) = (team_service, team_tools) {
        let session = runtime.session();
        let lead_id = session.session_id().to_string();
        let closure_lead = lead_id.clone();
        let append: onion::agent_team::journal::AppendFn = {
            Arc::new(move |event: onion::agent_team::projection::TeamEvent| {
                let ev = match event {
                    onion::agent_team::projection::TeamEvent::Member(m) => {
                        onion::session::SessionEvent::TeamMemberChange { team_id: closure_lead.clone(), member: m }
                    }
                    onion::agent_team::projection::TeamEvent::Task(t) => {
                        onion::session::SessionEvent::TeamTaskChange { team_id: closure_lead.clone(), task: t }
                    }
                };
                session.append(ev);
                Ok(())
            })
        };
        service
            .wire_sync(lead_id.clone(), append)
            .unwrap_or_else(|e| panic!("agent-team 延迟接线失败: {e}"));
        tt.wire_caller(lead_id);
    }
    goal_service.wire_session(runtime.session());
    let goal_driver = Arc::new(GoalRoundDriver::new(
        goal_service,
        runtime.inbox(),
        runtime.session(),
    ));
    {
        goal_driver.wire_subagents(Arc::new(move || subagent_runtime.inflight() > 0));
    }
    plan_mode.wire_session(runtime.session());
    // 会话目录单一权威接线（worktree 服务与 working_directory 共享同一句柄）。
    session_cwd.wire_session(runtime.session());
    // todo / ask_user / session_query / present / skill / schedule 延迟接线：锚定会话（durable 事件写入 + 调用方校验）。
    todo_tool.wire_session(runtime.session());
    ask_user_service.wire_session(runtime.session());
    session_query.wire_session(runtime.session());
    present_tool.wire_session(runtime.session());
    skill_tool.wire_session(runtime.session());
    schedule_tools.wire_session(runtime.session());
    // 会话目录单一权威接线：重放 `WorkingDir` / `WorktreeChange`（working_directory
    // 与 create_worktree 共享同一句柄，故只需接线一次）。
    session_cwd.wire_session(runtime.session());

    let busy = Arc::new(AtomicBool::new(false));
    let (done_tx, done_rx) = mpsc::unbounded_channel::<TurnReport>();

    let handle = Arc::new(SessionHandle {
        runtime,
        pending_approval: tokio::sync::Mutex::new(None),
        pending_plan_review: tokio::sync::Mutex::new(None),
        compact_tx,
        compaction_listener,
        tools_handle,
        goal_driver,
        plan_mode,
        busy,
        done_tx,
        active_agent: Arc::new(tokio::sync::Mutex::new(active_agent_id)),
        agent_catalog,
        working_dir: std::sync::Mutex::new(root),
    });

    Ok((handle, approval_rx, plan_review_rx, done_rx))
}

/// 常驻 kick 驱动（对齐 CLI `spawn_turn`，`kick()` → `while(turn())` 循环）：
/// 以空轮启动，由 kernel 内部 `claim_open()`/`drain_next_step()` 消费 inbox 排队
/// 消息。所有外部输入（session/prompt 的 followup/steer、goal 续轮）统一经 inbox
/// 投递，由本驱动（或主循环 select 的 inbox.notified() 分支）开轮，无带 input 直驱。
fn spawn_turn(handle: &Arc<SessionHandle>) {
    handle.busy.store(true, Ordering::SeqCst);
    let handle = handle.clone();
    let runtime = handle.runtime.clone();
    let done = handle.done_tx.clone();
    let busy = handle.busy.clone();
    let goal_driver = handle.goal_driver.clone();
    let plan_mode = handle.plan_mode.clone();
    tokio::spawn(async move {
        loop {
            // Ok(None) = 无轮可开（空输入且无排队消息，正常空转）；Err = 状态机
            // 拒绝开轮，必须给 stderr 反馈，否则 spawn 静默死亡、busy 悬挂为 true。
            let report = match runtime.run(Vec::new()).await {
                Ok(Some(report)) => report,
                Ok(None) => break,
                Err(error) => {
                    eprintln!("[sdk-server] turn 启动失败：{error}");
                    break;
                }
            };
            if matches!(report.reason, onion::kernel::TurnEndReason::MaxTokens)
                && goal_driver.disarm()
            {
                eprintln!("[sdk-server] goal 因 max-tokens 结束已 disarm");
            }
            plan_mode.flush_pending();
            let turn = report.turn;
            if done.send(report).is_err() {
                break;
            }
            if runtime.inbox().has_next_turn() {
                eprintln!("[sdk-server] next-turn 有排队消息，开新一轮 t{}", turn + 1);
                continue;
            }
            if runtime.inbox().has_next_step() && runtime.inbox().take_wakeup() {
                eprintln!("[sdk-server] next-step 有引导消息，开新一轮 t{}", turn + 1);
                continue;
            }
            // goal 自动续轮：续轮消息 followup 进
            // inbox（带 GoalRound source 锚点），由下一轮 kernel claim 消费；CAS
            // 预留（record_round）在 followup 前建立，失败放弃本轮。
            if goal_driver.should_continue() {
                if let Some(message) = goal_driver.continuation_input() {
                    eprintln!("[sdk-server] goal 自动续轮，开新一轮 t{}", turn + 1);
                    goal_driver.service_enter();
                    if goal_driver.record_round().is_err() {
                        goal_driver.service_exit();
                        break;
                    }
                    if runtime.inbox().followup(message).is_err() {
                        goal_driver.service_exit();
                        break;
                    }
                    continue;
                }
            }
            break;
        }
        goal_driver.service_exit();
        // 收尾兜底：
        // 收敛瞬间若 inbox 仍有排队消息，再 kick，防「队列有 pending 但无人开轮」。
        if runtime.inbox().has_pending() {
            busy.store(false, Ordering::SeqCst);
            spawn_turn(&handle);
            return;
        }
        busy.store(false, Ordering::SeqCst);
    });
}

/// 事件订阅转发任务：订阅 Runtime 事件 → stdout 发 `session.event` 通知。
fn spawn_event_forwarder(handle: Arc<SessionHandle>) {
    let runtime = handle.runtime.clone();
    let session_id = runtime.session().session_id().to_string();
    tokio::spawn(async move {
        let mut rx = runtime.subscribe();
        loop {
            let event = rx.recv().await;
            let Ok(event) = event else { break };
            let Ok(serialized) = serde_json::to_value(&event) else { continue };
            let line = rpc::session_event(&session_id, &serialized);
            let stdout = std::io::stdout();
            let mut lock = stdout.lock();
            if frame::write_frame(&mut lock, &line).is_err() {
                break;
            }
        }
    });
}

/// 处理一个 JSON-RPC 请求，返回写入 stdout 的响应行。
async fn handle_request(
    handle: &Arc<SessionHandle>,
    method: &str,
    params: Option<&Value>,
    id: u64,
) -> String {
    let result = dispatch(handle, method, params).await;
    match result {
        Ok(value) => rpc::response(id, value),
        Err(error) => rpc::error_response(id, error),
    }
}

/// 方法分发：返回结果或错误。
async fn dispatch(handle: &Arc<SessionHandle>, method: &str, params: Option<&Value>) -> Result<Value, RpcError> {
    match method {
        "initialize" => {
            // 可选：params.session_id 指定 resume；否则新建。
            let resume_id = params
                .and_then(|p| p.get("session_id"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            // 若 initialize 带 session_id 且当前会话非该 id，需重建——本实现
            // 在启动时已按 ONION_SDK_SESSION_ID 装配；此处仅返回当前会话信息。
            let _ = resume_id;
            Ok(json!({
                "protocol_version": "1.0",
                "server_info": "onion-sdk-server",
                "session_id": handle.runtime.session().session_id(),
            }))
        }
        "session/prompt" => {
            let message = params
                .and_then(|p| p.get("message"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| RpcError::new(code::INVALID_PARAMS, "缺少 message"))?;
            let partition = params
                .and_then(|p| p.get("partition"))
                .and_then(|v| v.as_str())
                .unwrap_or("followup");
            let agent_message = AgentMessage::user(message.to_string());
            let message_id = format!("m{}", onion::session::epoch_millis());
            match partition {
                "followup" | "next-turn" => {
                    handle.runtime.inbox().followup(agent_message).map_err(|e| {
                        RpcError::new(code::BUSINESS_ERROR, format!("followup 失败: {e}"))
                    })?;
                }
                "steer" | "next-step" => {
                    handle.runtime.inbox().steer(agent_message).map_err(|e| {
                        RpcError::new(code::BUSINESS_ERROR, format!("steer 失败: {e}"))
                    })?;
                }
                "inject" => {
                    handle.runtime.inbox().inject(agent_message).map_err(|e| {
                        RpcError::new(code::BUSINESS_ERROR, format!("inject 失败: {e}"))
                    })?;
                }
                other => {
                    return Err(RpcError::new(
                        code::INVALID_PARAMS,
                        format!("未知 partition {other}"),
                    ));
                }
            }
            // 空闲唤醒开轮：由主循环 select 的 inbox.notified() kick 分支消费；
            // busy 期间由活跃 driver 内循环接管。
            Ok(json!({ "message_id": message_id }))
        }
        "session/wait" => {
            // 阻塞至 idle（轮询 busy 标志）。timeout_ms 可选。
            let timeout_ms = params
                .and_then(|p| p.get("timeout_ms"))
                .and_then(serde_json::Value::as_u64);
            let deadline = timeout_ms.map(|ms| {
                tokio::time::Instant::now() + std::time::Duration::from_millis(ms)
            });
            loop {
                if !handle.busy.load(Ordering::SeqCst) {
                    break;
                }
                if let Some(deadline) = deadline {
                    if tokio::time::Instant::now() >= deadline {
                        return Ok(json!({ "idle": false }));
                    }
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            Ok(json!({ "idle": true }))
        }
        "session/working-directory/get" => {
            let dir = handle.working_dir.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
            Ok(json!({ "path": dir.display().to_string() }))
        }
        "session/working-directory/set" => {
            let path = params
                .and_then(|p| p.get("path"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| RpcError::new(code::INVALID_PARAMS, "缺少 path"))?;
            *handle.working_dir.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
                PathBuf::from(path);
            Ok(json!({ "path": path }))
        }
        "approval/reply" => {
            let approve = params
                .and_then(|p| p.get("approve"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            let mut pending = handle.pending_approval.lock().await;
            match pending.take() {
                Some(reply) => {
                    let decision = if approve {
                        onion::approval::ApprovalDecision::AllowedOnce
                    } else {
                        onion::approval::ApprovalDecision::Rejected
                    };
                    let _ = reply.send(decision);
                    Ok(json!({ "ok": true }))
                }
                None => Err(RpcError::new(code::BUSINESS_ERROR, "当前无待审批请求")),
            }
        }
        "plan/review/reply" => {
            let approve = params
                .and_then(|p| p.get("approve"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            let mut pending = handle.pending_plan_review.lock().await;
            match pending.take() {
                Some(reply) => {
                    let verdict = if approve {
                        onion::plan::service::PlanVerdict::Approve
                    } else {
                        onion::plan::service::PlanVerdict::KeepPlanning
                    };
                    let _ = reply.send(verdict);
                    Ok(json!({ "ok": true }))
                }
                None => Err(RpcError::new(code::BUSINESS_ERROR, "当前无待 plan-review 请求")),
            }
        }
        "session/agent" => {
            match params.and_then(|p| p.get("agent")).and_then(|v| v.as_str()) {
                Some(id) => {
                    let Some(agent) = handle.agent_catalog.get(id).cloned() else {
                        return Err(RpcError::new(
                            code::INVALID_PARAMS,
                            format!("未知 Agent {id}，可选：{}", handle.agent_catalog.ids().join(", ")),
                        ));
                    };
                    handle.tools_handle.state().set_agent(agent.tools);
                    *handle.active_agent.lock().await = id.to_string();
                    Ok(json!({ "agent": id }))
                }
                None => {
                    let ids = handle.agent_catalog.ids();
                    let current = handle.active_agent.lock().await.clone();
                    Ok(json!({ "agent": current, "available": ids }))
                }
            }
        }
        "session/compact" => {
            if handle.busy.load(Ordering::SeqCst) {
                let _ = handle.compact_tx.send(());
                Ok(json!({ "scheduled": true }))
            } else {
                handle
                    .compaction_listener
                    .compact_now(&handle.runtime.session())
                    .await;
                Ok(json!({ "scheduled": false }))
            }
        }
        "shutdown" => {
            handle
                .runtime
                .end("sdk shutdown")
                .await
                .map_err(|e| RpcError::new(code::INTERNAL_ERROR, e.to_string()))?;
            Ok(json!({ "ok": true }))
        }
        _ => Err(RpcError::new(code::METHOD_NOT_FOUND, format!("方法不存在: {method}"))),
    }
}

/// stdin 读取线程 → mpsc：避免阻塞 async 主循环。
/// EOF 时发送空帧标记关闭。
fn spawn_stdin_pump(line_tx: mpsc::UnboundedSender<String>) {
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        let mut reader = std::io::BufReader::new(stdin);
        loop {
            match frame::read_frame(&mut reader) {
                Ok(Some(line)) => {
                    if line_tx.send(line).is_err() {
                        break;
                    }
                }
                Ok(None) => break, // stdin EOF
                Err(_) => break,
            }
        }
        let _ = line_tx.send(String::new()); // EOF 哨兵
    });
}

/// stdout 事件通知（事件转发器与审批/plan 挂起共用）。
fn emit_notification(line: &str) {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    let _ = frame::write_frame(&mut lock, line);
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let config = Config::from_env();
    eprintln!(
        "onion-sdk-server — model={} base={}",
        config.model, config.base_url
    );

    let log_dir = std::env::current_dir()?.join(".onion-demo");
    // 可选：ONION_SDK_SESSION_ID 指定恢复既有会话。
    let session_id = std::env::var("ONION_SDK_SESSION_ID").ok().filter(|s| !s.is_empty());

    let (handle, mut approval_rx, mut plan_review_rx, mut done_rx) = assemble_session(
        config,
        log_dir,
        Arc::new(
            HostScheduleStore::open(&onion::paths::data_home())
                .unwrap_or_else(|e| panic!("schedule 存储打开失败: {e}")),
        ),
        session_id,
    )?;

    // 事件订阅转发（stdout 通知流）。
    spawn_event_forwarder(handle.clone());

    // stdin 专用线程（阻塞 read_line，不占用 async 运行时）。
    let (line_tx, mut line_rx) = mpsc::unbounded_channel::<String>();
    spawn_stdin_pump(line_tx);

    // 常驻 kick 的唤醒通知源：绑定一次避免 select 内临时值生命周期问题。
    let inbox = handle.runtime.inbox();

    // 主循环：stdin 帧 / 审批 / plan / done / kick 五路 select。
    loop {
        tokio::select! {
            line = line_rx.recv() => {
                let Some(line) = line else { break };
                if line.is_empty() {
                    // EOF 哨兵：等待在飞轮次消化完再退出。
                    while handle.busy.load(Ordering::SeqCst) {
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                    let _ = handle.runtime.end("stdin eof").await;
                    break;
                }
                // 解析为请求；空行/注释忽略。
                let parsed: Result<rpc::RpcMessage, _> = serde_json::from_str(&line);
                let msg = match parsed {
                    Ok(m) => m,
                    Err(_) => {
                        emit_notification(&rpc::error_response(0, RpcError::new(code::PARSE_ERROR, "解析错误")));
                        continue;
                    }
                };
                let Some(id) = msg.id else {
                    continue;
                };
                let Some(method) = msg.method else {
                    emit_notification(&rpc::error_response(id, RpcError::new(code::INVALID_REQUEST, "无效请求")));
                    continue;
                };
                let out = handle_request(&handle, &method, msg.params.as_ref(), id).await;
                emit_notification(&out);
                // shutdown 响应发出后退出主循环（server 进程终止，
                // client `child.wait()` 随之返回）。
                if method == "shutdown" {
                    break;
                }
            }
            request = approval_rx.recv() => {
                match request {
                    Some(request) => {
                        let ApprovalRequest { name, input, reply, .. } = request;
                        let summary = input.to_string();
                        let summary: String = if summary.chars().count() > 80 {
                            format!("{}…", summary.chars().take(80).collect::<String>())
                        } else {
                            summary
                        };
                        eprintln!("[sdk-server] ⏸ 待审批 {name}({summary}) — approval/reply 放行/拒绝");
                        *handle.pending_approval.lock().await = Some(reply);
                        // 审批请求通知（stdout 事件流）。
                        emit_notification(&rpc::notification(
                            "session.event",
                            json!({
                                "session_id": handle.runtime.session().session_id(),
                                "event": {
                                    "type": "approval.requested",
                                    "tool": name,
                                    "summary": summary,
                                }
                            }),
                        ));
                    }
                    None => break,
                }
            }
            review = plan_review_rx.recv() => {
                match review {
                    Some(review) => {
                        let onion::plan::service::PlanReviewRequest { plan, reply } = review;
                        let summary: String = if plan.chars().count() > 80 {
                            format!("{}…", plan.chars().take(80).collect::<String>())
                        } else {
                            plan
                        };
                        eprintln!("[sdk-server] ⏸ 待审批 exit_plan_mode — plan/review/reply");
                        eprintln!("    {summary}");
                        *handle.pending_plan_review.lock().await = Some(reply);
                        emit_notification(&rpc::notification(
                            "session.event",
                            json!({
                                "session_id": handle.runtime.session().session_id(),
                                "event": {
                                    "type": "plan.review.requested",
                                    "summary": summary,
                                }
                            }),
                        ));
                    }
                    None => break,
                }
            }
            report = done_rx.recv() => {
                if let Some(report) = report {
                    eprintln!("[sdk-server] [turn] t{} · {} steps · {}", report.turn, report.steps, report.reason_label());
                }
            }
            // 常驻 kick：inbox 被 followup/steer 唤醒，空闲
            //（无在飞轮次）且有排队消息时开轮。busy 期间由 spawn_turn 内循环接管。
            _ = inbox.notified() => {
                if !handle.busy.load(Ordering::SeqCst) && inbox.has_pending() {
                    spawn_turn(&handle);
                }
            }
        }
    }
    Ok(())
}
