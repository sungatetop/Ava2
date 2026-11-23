//! 会话装配与驱动（onion-server 内部装配单元，蓝本 onion-sdk-server main.rs）。
//!
//! 与 stdio 蓝本的关键差异：多 session —— 每个 A2A context 一个独立
//! Runtime（隔离 prompts / 沙箱 / 工具面 / 审批 / inbox）。装配逻辑
//! `assemble_session` 与 kick 驱动 `spawn_turn` 与蓝本同源，事件经
//! `EventBus` 转发为 A2A Artifact，由 jsonrpc / router 层消费。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::RwLock;

use serde::Serialize;
use tokio::sync::broadcast;

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
use onion::types::Usage;
use onion::sandbox::Sandbox;
use onion::session::LiveEvent;
use onion::system_prompt::{
    context_orders, section_orders, PromptContext, PromptSection, SectionText, SystemPrompt,
};
use onion::subagent::runtime::{ChildSession, SubagentRuntime};
use onion::tools::subagent::SubagentToolSet;
use onion::tools::{
    BashTool, EditTool, GlobTool, GrepTool, PwshTool, ReadImageTool, ReadTool, SandboxEscalator,
    StrReplaceEditorTool, TodoWriteTool, ToolRegistry, WebFetchTool, WebSearchTool, WriteTool,
};
use onion::ask_user::service::AskUserService;
use onion::ask_user::tools::AskUserQuestionTool;
use onion::present::PresentTool;
use onion::ralph::RalphTool;
use onion::schedule::service::ScheduleService;
use onion::schedule::storage::HostScheduleStore;
use onion::session_query::tools::SessionQueryTool;
use onion::skill::catalog::SkillCatalogListener;
use onion::skill::filesystem::filesystem_provider;
use onion::skill::gesture::SkillGestureListener;
use onion::skill::registry::SkillRegistry;
use onion::skill::tools::SkillTool;
use onion::working_dir::WorkingDirectoryTool;
use onion::lsp::{LspService, LspTool};
use onion::shell_exec::oneshot::OneshotExecutor;
use onion::shell_exec::sandbox::{ProcessSandboxRunner, SandboxConfig};
use onion::listeners::{
    ApprovalGate, ApprovalRequest, EchoObserver, EnvInjector, Gatekeeper, RetryNudger,
};

use crate::a2a::{ContextId, TaskId};

/// 会话句柄：一个 session 对应一个 Runtime + 全部服务/通道（共享状态）。
pub(crate) struct SessionHandle {
    pub runtime: Arc<Runtime>,
    /// 事件扇出 + 待回填输入表（REST/A2A 共用）。
    pub channels: Arc<SessionChannels>,
    /// compaction `/compact` 信号发送端。
    pub compact_tx: mpsc::UnboundedSender<()>,
    /// compaction listener（空闲直驱 `compact_now`）。
    pub compaction_listener: Arc<CompactionListener>,
    /// 工具注册表（共享 ToolState，/agent 切换）。
    pub tools_handle: ToolRegistry,
    /// 当前 Agent id（/agent 查询与切换）。
    pub active_agent: Arc<tokio::sync::Mutex<String>>,
    /// 完整 Agent 目录（/agent 选择器：id �每列表 + 解析）。
    pub agent_catalog: onion::preset::AgentCatalog,
    /// goal 服务（REST goal 读面与动作）。
    pub goal_service: Arc<GoalService>,
    pub(crate) goal_driver: Arc<GoalRoundDriver>,
    pub(crate) plan_mode: Arc<PlanMode>,
    /// 轮次闸门（driver 唯一性 + 在飞计数 + 事件驱动等待）。
    pub gate: Arc<crate::turn_gate::TurnGate>,
    /// turn 完成通知。
    pub done_tx: mpsc::UnboundedSender<TurnReport>,
    /// 会话目录单一权威（REST 读面与全部工具共享）。
    pub session_cwd: Arc<onion::session_cwd::SessionCwd>,
    /// 运行期模型目标（kernel 与 llm 共享；REST /model 切换经 swap_current）。
    pub model_target: Arc<onion::model_target::ModelTargetRef>,
    /// 审批配置（ApprovalGate 共享句柄；REST /approval 切换写入口）。
    pub approval_config: Arc<RwLock<onion::approval::ApprovalConfig>>,
    /// 主 LLM 客户端（审批 `auto` 预设重建 LlmReviewer 复用）。
    pub llm: Arc<OpenAiSseLlm>,
    /// 技能注册表（REST 技能枚举）。
    pub skill_registry: Arc<std::sync::Mutex<SkillRegistry>>,
    /// 日程服务（REST 日程读面）。
    pub schedule_service: Arc<ScheduleService>,
    /// 后台作业注册表（REST 作业读面）。
    pub job_registry: Arc<onion_jobs::JobRegistry>,
    /// 本会话的作业归属。
    pub job_owner: onion_jobs::SessionId,
    /// MCP 桥接工具集共享句柄（resync/probe/reconcile 管理面 + 配置/注册名/状态查询）。
    pub mcp_bridge: Arc<onion::mcp_resources::McpBridgeToolSet>,
    /// MCP 资源 provider 服务（reconcile 时按期望配置增删 provider）。
    pub mcp_resources: Arc<onion::mcp_resources::McpResourceService>,
}

use tokio::sync::{mpsc, oneshot};

/// SSE 扇出容量（滞后订阅者收到 `Lagged` 后由客户端重连对齐）。
const CHANNEL_CAPACITY: usize = 512;
/// 已解答输入保留条数（重复回填幂等窗口）。
const RESOLVED_KEEP: usize = 64;

/// 进程内请求序号（`requestId` = `req-<epoch_ms>-<seq>`，进程内唯一）。
static REQUEST_SEQ: AtomicU64 = AtomicU64::new(1);

fn next_request_id() -> String {
    format!(
        "req-{}-{}",
        onion::session::epoch_millis(),
        REQUEST_SEQ.fetch_add(1, Ordering::Relaxed)
    )
}

fn lock<T>(mutex: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// 会话事件帧（REST SSE `/v1/sessions/{id}/events` 的 `type` 标签）。
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum SessionNotice {
    /// 连接建立快照（durable 事件全量，便于重连对齐）。
    Snapshot {
        events: Vec<onion::session::SessionEvent>,
    },
    /// durable 事件（带落盘时间戳）。
    Durable {
        ts: u64,
        event: onion::session::SessionEvent,
    },
    /// live 瞬时事件（StateChanged / Inbox* / AssistantStream）。
    Live {
        event: LiveEvent,
    },
    /// 待审批。
    ApprovalRequested {
        #[serde(rename = "requestId")]
        request_id: String,
        tool: String,
        input: serde_json::Value,
        /// 审批来源标记（`sandbox-retry` = 沙箱拒绝后的越界执行申请；缺省 = 首次审批）。
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    /// 待 plan 裁决。
    PlanReviewRequested {
        #[serde(rename = "requestId")]
        request_id: String,
        plan: String,
    },
    /// 待人工问询。
    QuestionRequested {
        #[serde(rename = "requestId")]
        request_id: String,
        #[serde(rename = "callId")]
        call_id: String,
        questions: Vec<onion::ask_user::types::AskUserQuestion>,
    },
    /// 轮次结束。
    TurnReport {
        turn: u32,
        steps: u32,
        reason: String,
        /// 本轮各步 usage 累计（input/output/cache_*，snake_case 与 durable 事件一致）
        usage: Usage,
    },
    /// 会话关闭。
    SessionClosed {
        reason: String,
    },
}

impl SessionNotice {
    /// SSE `event:` 名（与 `type` 标签一致）。
    pub(crate) fn event_name(&self) -> &'static str {
        match self {
            Self::Snapshot { .. } => "snapshot",
            Self::Durable { .. } => "durable",
            Self::Live { .. } => "live",
            Self::ApprovalRequested { .. } => "approval_requested",
            Self::PlanReviewRequested { .. } => "plan_review_requested",
            Self::QuestionRequested { .. } => "question_requested",
            Self::TurnReport { .. } => "turn_report",
            Self::SessionClosed { .. } => "session_closed",
        }
    }
}

/// 待回填输入的类别（含展示载荷）。
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum PendingKind {
    Approval {
        tool: String,
        input: serde_json::Value,
        /// 审批来源标记（`sandbox-retry` = 沙箱拒绝后的越界执行申请；缺省 = 首次审批）。
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    PlanReview {
        plan: String,
    },
    Question {
        #[serde(rename = "callId")]
        call_id: String,
        questions: Vec<onion::ask_user::types::AskUserQuestion>,
    },
}

impl PendingKind {
    /// 类别标签（日志 / 幂等记录）。
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::Approval { .. } => "approval",
            Self::PlanReview { .. } => "plan_review",
            Self::Question { .. } => "question",
        }
    }

    /// 面向裁决方的人读摘要（A2A Task `status.message` 与跨实例确认请求正文）。
    pub(crate) fn summary(&self) -> String {
        match self {
            Self::Approval { tool, input, .. } => {
                format!("工具 `{tool}` 请求放行，输入：{input}")
            }
            Self::PlanReview { plan } => plan.clone(),
            Self::Question { call_id, questions } => format!(
                "callId={call_id}\n{}",
                serde_json::to_string(questions).unwrap_or_default()
            ),
        }
    }
}

/// 待回填输入的一次性答复通道。
pub(crate) enum ReplySender {
    Approval(oneshot::Sender<onion::approval::ApprovalDecision>),
    PlanReview(oneshot::Sender<onion::plan::service::PlanVerdict>),
    Question(oneshot::Sender<onion::ask_user::types::AskUserAnswer>),
}

/// 待回填输入记录。
pub(crate) struct PendingInput {
    pub id: String,
    pub kind: PendingKind,
    pub created_at_ms: u64,
    pub reply: ReplySender,
}

/// 已解答输入（幂等窗口：记录原始决定）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResolvedInput {
    pub id: String,
    pub decision: String,
}

/// 回填错误（404 / 409）。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ResolveError {
    NotFound,
    AlreadyResolved,
}

/// 轮次摘要（`GET /v1/sessions/{id}` 的 lastTurn）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TurnSummary {
    pub turn: u32,
    pub steps: u32,
    pub reason: String,
    /// 本轮各步 usage 累计
    pub usage: Usage,
}

/// 会话级事件扇出 + 待回填输入表 + 最近轮次摘要。
pub(crate) struct SessionChannels {
    pub interactive: bool,
    events: broadcast::Sender<SessionNotice>,
    pending: std::sync::Mutex<HashMap<String, PendingInput>>,
    resolved: std::sync::Mutex<Vec<ResolvedInput>>,
    last_report: std::sync::Mutex<Option<TurnSummary>>,
}

impl SessionChannels {
    fn new(interactive: bool) -> Self {
        let (events, _rx) = broadcast::channel(CHANNEL_CAPACITY);
        Self {
            interactive,
            events,
            pending: std::sync::Mutex::new(HashMap::new()),
            resolved: std::sync::Mutex::new(Vec::new()),
            last_report: std::sync::Mutex::new(None),
        }
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<SessionNotice> {
        self.events.subscribe()
    }

    pub(crate) fn notice(&self, notice: SessionNotice) {
        let _ = self.events.send(notice);
    }

    /// 登记待回填输入，返回服务端 `requestId`。
    pub(crate) fn push(&self, kind: PendingKind, reply: ReplySender) -> String {
        let id = next_request_id();
        let input = PendingInput {
            id: id.clone(),
            kind,
            created_at_ms: onion::session::epoch_millis(),
            reply,
        };
        lock(&self.pending).insert(id.clone(), input);
        id
    }

    /// 取走待回填输入；重复回填返回 `AlreadyResolved`，未知 id 返回 `NotFound`。
    pub(crate) fn resolve(&self, id: &str, decision: &str) -> Result<PendingInput, ResolveError> {
        let mut pending = lock(&self.pending);
        if let Some(input) = pending.remove(id) {
            drop(pending);
            let mut resolved = lock(&self.resolved);
            resolved.push(ResolvedInput {
                id: id.to_string(),
                decision: decision.to_string(),
            });
            if resolved.len() > RESOLVED_KEEP {
                let excess = resolved.len() - RESOLVED_KEEP;
                resolved.drain(..excess);
            }
            return Ok(input);
        }
        drop(pending);
        if lock(&self.resolved).iter().any(|item| item.id == id) {
            return Err(ResolveError::AlreadyResolved);
        }
        Err(ResolveError::NotFound)
    }

    /// 已解答输入的原始决定（重复回填的 409 消息）。
    pub(crate) fn resolved_decision(&self, id: &str) -> Option<String> {
        lock(&self.resolved)
            .iter()
            .find(|item| item.id == id)
            .map(|item| item.decision.clone())
    }

    /// 中止全部待回填输入（会话中断）：drop 一次性答复端，等待端 recv Err →
    /// 内核按 `cancelled` 结局落审计。
    /// 返回被中止的请求数；这些 id 一并进入已解答窗口（后续回填返回 409）。
    pub(crate) fn cancel_all(&self, decision: &str) -> usize {
        let mut pending = lock(&self.pending);
        if pending.is_empty() {
            return 0;
        }
        let cancelled: Vec<PendingInput> = pending.drain().map(|(_, input)| input).collect();
        drop(pending);
        let count = cancelled.len();
        let mut resolved = lock(&self.resolved);
        for input in cancelled {
            resolved.push(ResolvedInput { id: input.id, decision: decision.to_string() });
            drop(input.reply);
        }
        if resolved.len() > RESOLVED_KEEP {
            let excess = resolved.len() - RESOLVED_KEEP;
            resolved.drain(..excess);
        }
        count
    }

    /// 改写已解答记录的决定字面值（回填未送达时纠正为 `cancelled`）。
    pub(crate) fn override_decision(&self, id: &str, decision: &str) {
        if let Some(item) = lock(&self.resolved).iter_mut().find(|item| item.id == id) {
            item.decision = decision.to_string();
        }
    }

    /// 待回填列表（`requestId` + 类别 + 登记时间）。
    pub(crate) fn pending_views(&self) -> Vec<(String, PendingKind, u64)> {
        let pending = lock(&self.pending);
        let mut views: Vec<(String, PendingKind, u64)> = pending
            .values()
            .map(|input| (input.id.clone(), input.kind.clone(), input.created_at_ms))
            .collect();
        views.sort_by(|a, b| a.0.cmp(&b.0));
        views
    }

    pub(crate) fn set_last_report(&self, summary: TurnSummary) {
        *lock(&self.last_report) = Some(summary);
    }

    pub(crate) fn last_report(&self) -> Option<TurnSummary> {
        lock(&self.last_report).clone()
    }
}

/// 服务端会话：id + handle + 事件总线 + A2A task 状态。
pub(crate) struct ServerSession {
    /// onion 会话 id（= A2A context_id）。
    pub id: String,
    pub handle: Arc<SessionHandle>,
    pub channels: Arc<SessionChannels>,
    /// 是否有人工输入通道（A2A 会话固定 false）。
    pub interactive: bool,
    /// 会话权限根：装配期固定，`cd` 不可越根。
    pub root: PathBuf,
    /// 该根的会话落盘容器（`<data_home>/sessions/<normalized-cwd>/`）。
    pub sessions_dir: PathBuf,
    pub created_at_ms: u64,
    /// 上下文预算（`Config::context_window_tokens`，装配期固定）：占比展示的分母。
    pub context_window_tokens: u64,
    /// task_id → Task（服务端持有状态机：submitted → working → completed）。
    pub tasks: tokio::sync::Mutex<HashMap<TaskId, TaskRecord>>,
    /// push notification 配置（url/token/authentication）。
    pub push_config: tokio::sync::Mutex<Option<crate::a2a::PushNotificationConfig>>,
    /// 发起方回传地址（`message.metadata.reportTo`，A2A 委派时透传落库；跨实例
    /// 审批桥按它路由）。与 `spawn_session_loops` 共享同一槽位。
    pub report_to: Arc<tokio::sync::Mutex<Option<serde_json::Value>>>,
    /// 本会话最近一次投递的 task id（跨实例确认请求携带，`tasks/provideInput` 反查）。
    pub current_task: Arc<tokio::sync::Mutex<Option<TaskId>>>,
}

/// 服务端 Task 记录（状态机：submitted → working → completed/failed）。
#[derive(Clone, Debug)]
pub(crate) struct TaskRecord {
    pub id: TaskId,
    pub context_id: ContextId,
    pub state: crate::a2a::TaskState,
    /// 任务历史（Message 序列）。
    pub history: Vec<crate::a2a::Message>,
    /// 产出物（Artifact 序列）。
    pub artifacts: Vec<crate::a2a::Artifact>,
    /// `input-required` 时的状态消息（携带 `confirmRequest` 元数据）。
    pub status_message: Option<crate::a2a::Message>,
}

/// 会话注册表：session_id → ServerSession（共享装配参数）。
#[derive(Clone)]
pub(crate) struct SessionRegistry {
    inner: Arc<RegistryInner>,
}

struct RegistryInner {
    /// session_id → 槽位。装配在锁外进行（MCP 发现会 spawn 子进程），锁内只占位判重。
    sessions: tokio::sync::Mutex<HashMap<String, Slot>>,
    config: Config,
    /// 兜底工作目录（`ONION_SERVER_WORKSPACE`）：
    /// 会话未显式给 `cwd` 时用它。
    workspace_root: PathBuf,
    /// 兜底工作目录对应的会话落盘容器。
    default_sessions_dir: PathBuf,
    /// 跨实例审批桥（peers 表 + 自身 URL + mesh 令牌）。
    bridge: Arc<crate::bridge::Bridge>,
    /// 日程存储（进程级单例：同一 `tasks.jsonl` 只允许一个内存投影）。
    schedule_store: Arc<HostScheduleStore>,
}

/// 会话槽：装配中为 `Pending`（装配完成后 `send` 唤醒等待者），完成后为 `Ready`。
///
/// `Pending` 用 `watch` 而非 `Notify`：`watch::Receiver::changed` 记录版本号，
/// 「装配完成」与「开始等待」之间不存在丢唤醒窗口；`Notify::notify_waiters`
/// 只唤醒已注册的等待者，会出现等待者永久阻塞。
enum Slot {
    Pending(Arc<tokio::sync::watch::Sender<()>>),
    Ready(Arc<ServerSession>),
}

impl Slot {
    fn ready_session(&self) -> Option<Arc<ServerSession>> {
        match self {
            Self::Ready(session) => Some(session.clone()),
            Self::Pending(_) => None,
        }
    }
}

/// 装配产出、尚未接入会话循环的四个输入通道。
struct SessionLoops {
    approval_rx: mpsc::UnboundedReceiver<ApprovalRequest>,
    plan_review_rx: mpsc::UnboundedReceiver<onion::plan::service::PlanReviewRequest>,
    question_rx: mpsc::UnboundedReceiver<onion::ask_user::service::AskUserRequest>,
    done_rx: mpsc::UnboundedReceiver<TurnReport>,
}

impl SessionRegistry {
    pub(crate) fn new(
        config: Config,
        workspace_root: PathBuf,
        default_sessions_dir: PathBuf,
        bridge: Arc<crate::bridge::Bridge>,
    ) -> Self {
        Self {
            inner: Arc::new(RegistryInner {
                sessions: tokio::sync::Mutex::new(HashMap::new()),
                config,
                workspace_root,
                default_sessions_dir,
                bridge,
                schedule_store: Arc::new(
                    HostScheduleStore::open(&onion::paths::data_home())
                        .unwrap_or_else(|e| panic!("schedule 存储打开失败: {e}")),
                ),
            }),
        }
    }

    /// 兜底工作目录（`ONION_SERVER_WORKSPACE`）：会话未显式给 `cwd` 时用它。
    pub(crate) fn workspace_root(&self) -> &PathBuf {
        &self.inner.workspace_root
    }

    /// 日程存储（进程级单例）：全局任务面（调度循环 + REST）与会话内
    /// `schedule_create` 共用同一 `tasks.jsonl` 内存投影。
    pub(crate) fn schedule_store(&self) -> Arc<HostScheduleStore> {
        self.inner.schedule_store.clone()
    }

    /// 兜底工作目录对应的会话落盘容器。
    pub(crate) fn default_sessions_dir(&self) -> &PathBuf {
        &self.inner.default_sessions_dir
    }

    /// 会话容器根（`<data_home>/sessions`）：由兜底容器反推，避免运行期再读
    /// `data_home()`（进程环境可能与装配期不一致）。
    pub(crate) fn sessions_root(&self) -> PathBuf {
        self.inner
            .default_sessions_dir
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.inner.default_sessions_dir.clone())
    }

    /// 工作目录 → 该目录专属的会话落盘容器。
    pub(crate) fn sessions_dir_for(&self, root: &Path) -> PathBuf {
        self.sessions_root().join(onion::paths::normalize_cwd(root))
    }

    /// 会话 `cwd` → 权限根（`session.cwd ?? workspaceRoot`）。
    ///
    /// `None` / 空白 = 兜底工作目录；显式值必须是**已存在的绝对路径**（不做
    /// 隐式建目录），否则 `InvalidInput`。保留调用方拼写、不做 `canonicalize`
    ///（Windows 的 `\\?\` 前缀会污染落盘容器名）。
    fn resolve_root(&self, cwd: Option<&str>) -> std::io::Result<PathBuf> {
        let Some(raw) = cwd.map(str::trim).filter(|value| !value.is_empty()) else {
            return Ok(self.inner.workspace_root.clone());
        };
        let path = Path::new(raw);
        if !path.is_absolute() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("cwd must be an absolute path: {raw}"),
            ));
        }
        if !path.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("cwd is not an existing directory: {raw}"),
            ));
        }
        Ok(path.to_path_buf())
    }

    /// 创建会话（同 id 已存在则返回既有会话）：显式 id 或 `onion-<epoch_ms>`。
    pub(crate) async fn create(
        &self,
        session_id: Option<String>,
        agent_id: Option<String>,
        interactive: bool,
        cwd: Option<String>,
    ) -> std::io::Result<Arc<ServerSession>> {
        let id = session_id.unwrap_or_else(onion::session::fresh_session_id);
        let root = self.resolve_root(cwd.as_deref())?;
        let sessions_dir = self.sessions_dir_for(&root);
        write_workspace_marker(&sessions_dir, &root);
        let assembled = self
            .get_or_assemble(
                id.clone(),
                agent_id,
                interactive,
                onion::session::epoch_millis(),
                root,
                sessions_dir,
            )
            .await?;
        assembled.ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, format!("会话 {id} 未能装配"))
        })
    }

    /// 打开会话：live 内存会话优先，未在内存但已落盘的历史会话按需恢复
    /// （进程重启后回放日志重建内存状态）。
    ///
    /// live 优先：内存中已就绪的会话不依赖落盘日志仍然存在（日志可能被
    /// 外部清理，磁盘 `locate` 失败不应把 live 会话误判为不存在）。
    ///
    /// 先查兜底工作目录的容器，再扫 `<data_home>/sessions/*` 全项目枚举定位
    ///（会话 id 在实例内全局唯一）。
    ///
    /// `Ok(None)` = 内存与磁盘皆无该会话；`Err` = 恢复装配失败。
    pub(crate) async fn open(
        &self,
        session_id: &str,
        interactive: bool,
    ) -> std::io::Result<Option<Arc<ServerSession>>> {
        // live 命中即返回；`Pending` 不在此处理，交给下方 `get_or_assemble`
        // 的占位等待逻辑（装配完成后由同一入口返回）。
        {
            let sessions = self.inner.sessions.lock().await;
            if let Some(Slot::Ready(session)) = sessions.get(session_id) {
                return Ok(Some(session.clone()));
            }
        }
        let Some((sessions_dir, created_at_ms)) = self.locate(session_id) else {
            return Ok(None);
        };
        let root = self.root_for_sessions_dir(&sessions_dir);
        // `locate` 已确认磁盘存在，故按「可装配」处理（`create_if_missing=false`
        // 会让冷会话永远返回 None）。
        self.get_or_assemble(
            session_id.to_string(),
            None,
            interactive,
            created_at_ms,
            root,
            sessions_dir,
        )
        .await
    }

    /// 定位会话落盘容器：兜底容器优先，其次扫 `<data_home>/sessions/*`。
    fn locate(&self, session_id: &str) -> Option<(PathBuf, u64)> {
        if let Some(ms) = stored_session_ms(&self.inner.default_sessions_dir, session_id) {
            return Some((self.inner.default_sessions_dir.clone(), ms));
        }
        let sessions_root = self.sessions_root();
        let entries = std::fs::read_dir(&sessions_root).ok()?;
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            if let Some(ms) = stored_session_ms(&dir, session_id) {
                return Some((dir, ms));
            }
        }
        None
    }

    /// 落盘容器 → 权限根：兜底容器用兜底根；其余容器读创建期写入的
    /// `<container>/cwd` 标记（见 [`write_workspace_marker`]）。
    fn root_for_sessions_dir(&self, sessions_dir: &Path) -> PathBuf {
        container_root(
            sessions_dir,
            &self.inner.default_sessions_dir,
            &self.inner.workspace_root,
        )
    }

    /// 两段式取会话：锁内占位判重，锁外装配（MCP 发现会 spawn 子进程），回锁替换。
    ///
    /// 调用方负责先确认「该装配」（`create` 直接装配；`open` 先经 [`Self::locate`]
    /// 确认磁盘存在），因此这里不再区分 create / open。
    #[allow(clippy::too_many_arguments)]
    async fn get_or_assemble(
        &self,
        id: String,
        agent_id: Option<String>,
        interactive: bool,
        created_at_ms: u64,
        root: PathBuf,
        sessions_dir: PathBuf,
    ) -> std::io::Result<Option<Arc<ServerSession>>> {
        // 抢到占位的一方负责装配；其余等待者订阅同一 `watch`，被唤醒后回读槽位
        // （`watch` 记录版本号，「完成」与「开始等待」之间无丢唤醒窗口）。
        let notify = loop {
            let pending = {
                let mut sessions = self.inner.sessions.lock().await;
                match sessions.get(&id) {
                    Some(Slot::Ready(session)) => return Ok(Some(session.clone())),
                    Some(Slot::Pending(notify)) => Some(notify.subscribe()),
                    None => {
                        let (tx, _rx) = tokio::sync::watch::channel(());
                        let notify = Arc::new(tx);
                        sessions.insert(id.clone(), Slot::Pending(notify.clone()));
                        break notify;
                    }
                }
            };
            if let Some(mut rx) = pending {
                rx.changed().await.ok();
            }
        };
        // 锁外装配：慢 MCP server 不再阻塞其他会话的创建。
        let assembled = self.assemble(id.clone(), agent_id, interactive, created_at_ms, root, sessions_dir);
        let (session, loops) = match assembled {
            Ok(assembled) => assembled,
            Err(error) => {
                let mut sessions = self.inner.sessions.lock().await;
                sessions.remove(&id);
                drop(sessions);
                // 占位已撤销：广播唤醒等待者，让它们重新参与占位。
                // `send_replace` 不返回 Result：此处无接收者也是合法的（等待者可能已先离开）。
                notify.send_replace(());
                return Err(error);
            }
        };
        // 回锁替换占位并起会话循环（循环启动必须只做一次，故在替换后执行）。
        let mut sessions = self.inner.sessions.lock().await;
        sessions.insert(id.clone(), Slot::Ready(session.clone()));
        drop(sessions);
        notify.send_replace(());
        self.start_loops(&session, loops);
        Ok(Some(session))
    }

    /// 装配会话（不触碰 `sessions` 锁）：返回会话与尚未启动的四个 receiver。
    #[allow(clippy::type_complexity)]
    fn assemble(
        &self,
        id: String,
        agent_id: Option<String>,
        interactive: bool,
        created_at_ms: u64,
        root: PathBuf,
        sessions_dir: PathBuf,
    ) -> std::io::Result<(Arc<ServerSession>, SessionLoops)> {
        let (handle, approval_rx, plan_review_rx, question_rx, done_rx) = assemble_session(
            self.inner.config.clone(),
            sessions_dir.clone(),
            root.clone(),
            self.inner.schedule_store.clone(),
            id.clone(),
            agent_id,
            interactive,
        )?;
        let channels = handle.channels.clone();
        let session = Arc::new(ServerSession {
            id,
            handle,
            channels,
            interactive,
            root,
            sessions_dir,
            created_at_ms,
            context_window_tokens: self.inner.config.context_window_tokens,
            tasks: tokio::sync::Mutex::new(HashMap::new()),
            push_config: tokio::sync::Mutex::new(None),
            report_to: Arc::new(tokio::sync::Mutex::new(None)),
            current_task: Arc::new(tokio::sync::Mutex::new(None)),
        });
        let loops = SessionLoops {
            approval_rx,
            plan_review_rx,
            question_rx,
            done_rx,
        };
        Ok((session, loops))
    }

    /// 起会话的事件/审批/轮次循环（每个会话只调用一次）。
    fn start_loops(&self, session: &Arc<ServerSession>, loops: SessionLoops) {
        spawn_session_loops(
            session.channels.clone(),
            self.inner.bridge.clone(),
            session.report_to.clone(),
            session.current_task.clone(),
            session.handle.runtime.subscribe(),
            loops.approval_rx,
            loops.plan_review_rx,
            loops.question_rx,
            loops.done_rx,
            session.handle.clone(),
        );
    }

    /// A2A 入口：context_id 即 session_id，固定 `interactive=false`，落兜底工作目录。
    pub(crate) async fn get_or_create(
        &self,
        context_id: &ContextId,
    ) -> std::io::Result<Arc<ServerSession>> {
        self.create(Some(context_id.as_str().to_string()), None, false, None)
            .await
    }

    pub(crate) async fn remove(&self, session_id: &str) -> Option<Arc<ServerSession>> {
        self.inner
            .sessions
            .lock()
            .await
            .remove(session_id)
            .and_then(|slot| slot.ready_session())
    }

    /// 全量 live session 快照（装配中的会话不返回）。
    pub(crate) async fn all(&self) -> Vec<Arc<ServerSession>> {
        self.inner
            .sessions
            .lock()
            .await
            .values()
            .filter_map(Slot::ready_session)
            .collect()
    }
}

/// 历史会话日志文件的修改时间（毫秒）：`None` = 磁盘上没有该会话（同
/// [`onion::session::list_sessions`] 的「目录 + 当前版本日志」判定）。
fn stored_session_ms(sessions_dir: &std::path::Path, session_id: &str) -> Option<u64> {
    let path = onion::paths::session_log_file(&onion::paths::session_dir(sessions_dir, session_id));
    std::fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|elapsed| elapsed.as_millis() as u64)
}

/// 构造审批配置（部署侧装配点）：`auto` 预设升级为 LLM 复审裁决者。
pub(crate) fn build_approval_config(
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

/// 装配一个 Runtime + 全部 listener + 服务，返回 (共享 handle, 四个输入通道 receiver)。
/// 与 onion-sdk-server `assemble_session` 同源（多 session 版：root 按会话隔离，
/// session_id 显式指定，日志经 `open_with_replay` 落 `sessions_dir/<id>/`）。
#[allow(clippy::type_complexity)]
pub(crate) fn assemble_session(
    config: Config,
    sessions_dir: PathBuf,
    root: PathBuf,
    schedule_store: Arc<HostScheduleStore>,
    session_id: String,
    agent_id: Option<String>,
    interactive: bool,
) -> std::io::Result<(
    Arc<SessionHandle>,
    mpsc::UnboundedReceiver<ApprovalRequest>,
    mpsc::UnboundedReceiver<onion::plan::service::PlanReviewRequest>,
    mpsc::UnboundedReceiver<onion::ask_user::service::AskUserRequest>,
    mpsc::UnboundedReceiver<TurnReport>,
)> {
    let mut prompts = SystemPrompt::new(&config);
    prompts
        .section(PromptSection {
            name: "onion:mission".to_string(),
            order: section_orders::TOOL_GUIDE,
            text: SectionText::Static(
                "你是 onion 的智能体，运行于洋葱模型内核，经 A2A server 通道驱动。当前模型为 {{model}}。\
                 基础工具：read（读取沙箱内文本文件）、read_image（读取沙箱内图片并作为多模态块返回）。\
                 读文件按类型选工具：文本用 read，图片（png/jpg/jpeg/gif/webp）用 read_image；用错会被拒绝并提示改用另一个。\
                 write（整体覆盖）、edit（按字面量替换）、str_replace_editor（view / create / str_replace / insert）用于写改文件；覆盖既有文件前须先用 read 读取（read-before-write 策略），新建文件不受此约束。\
                 工具面由当前 Agent 决定：内置 coding（缺省）与 work，可经 agents.yml 扩展；调用 Agent 工具面外的工具会报错，改用面内等价工具或提示用户切换 Agent。\
                 审批：默认 workspace-write 预设下，只读命令与工作区内写入（建目录、复制、构建、测试等）直通；越界写、破坏性命令、网络访问与不可静态判定的命令（命令替换、脚本块）会挂起等待审批，本进程当前无人工审批通道，此类调用一律被拒绝。\
                 shell 方言按平台固定：Windows 用 pwsh / cmd（无 bash），macOS / Linux 用 bash。\
                 大文本策略：单次输出受 token 上限约束，超长内容（如长文、大文件）必须分段读写——写入时用 write 分多次追加/分段写入，读取时长文件分段读取，切勿在一次工具调用参数中塞入超出输出上限的超大文本。\
                 需要时主动调用工具完成任务。"
                    .to_string(),
            ),
            interpolate: true,
            complete: false,
        })
        .unwrap_or_else(|error| panic!("onion:mission 段注册失败: {error}"));
    // per-tool 文件策略段（read/write/edit）。
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
    // shell 执行策略：只注册当前平台实际挂载的 shell 段（Windows = pwsh，POSIX = bash）。
    prompts
        .section(PromptSection {
            name: if cfg!(windows) { "tool:pwsh" } else { "tool:bash" }.to_string(),
            order: if cfg!(windows) { section_orders::TOOL_PWSH } else { section_orders::TOOL_BASH },
            text: SectionText::Static(
                if cfg!(windows) {
                    "Non-zero exits are reported as `[exit code: N]` markers; investigate failures \
                     before moving on. On Windows a killed process settles as `[exit code: 1]` \
                     without a signal marker; treat a bare exit 1 after an interruption as a \
                     termination, not a command failure. Read-only commands and workspace writes \
                     (mkdir / copy / build / test) normally run without approval. Background \
                     execution (`run_in_background`) is not covered by the OS sandbox, so those \
                     calls still need approval — run workspace writes and builds in the foreground."
                } else {
                    "Check the [exit code: N] marker on every bash result; investigate failures \
                     before moving on. Read-only commands and workspace writes (mkdir / cp / \
                     build / test) normally run without approval. Background execution \
                     (`run_in_background`) is not covered by the OS sandbox, so those calls still \
                     need approval — run workspace writes and builds in the foreground."
                }
                .to_string(),
            ),
            interpolate: false,
            complete: false,
        })
        .unwrap_or_else(|error| panic!("shell 策略段注册失败: {error}"));
    // 后台任务控制面策略。
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
    // LSP 策略。
    prompts
        .section(PromptSection {
            name: "tool:lsp".to_string(),
            order: section_orders::TOOL_LSP,
            text: SectionText::Static(onion::lsp::tools::LSP_PROMPT_TEXT.to_string()),
            interpolate: false,
            complete: false,
        })
        .unwrap_or_else(|error| panic!("tool:lsp 段注册失败: {error}"));
    // 终端策略（terminal_* 6 件）。
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
    // git worktree 策略。
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
    // workspace 依赖策略。
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
    // 记忆管理策略。
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
    // 启用时注册。
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

    // 会话目录单一权威：权限根恒定，cwd 可在根内移动。
    // 所有工具共享同一实例——worktree 隔离成立的结构性前提。
    let session_cwd = Arc::new(onion::session_cwd::SessionCwd::new(root.clone()));
    let sandbox = Arc::new(Sandbox::with_cwd(session_cwd.clone()));
    // read-before-write 观测表：会话级、不跨会话持久化。
    let observation = Arc::new(onion::fs_observation::FsObservation::new());
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(ReadTool { sandbox: sandbox.clone(), observation: observation.clone() }));
    tools.register(Arc::new(ReadImageTool { sandbox: sandbox.clone(), observation: observation.clone() }));
    tools.register(Arc::new(WriteTool { sandbox: sandbox.clone(), observation: observation.clone() }));
    tools.register(Arc::new(EditTool { sandbox: sandbox.clone(), observation: observation.clone() }));
    tools.register(Arc::new(StrReplaceEditorTool { sandbox: sandbox.clone(), observation: observation.clone() }));
    // apply_patch：一次提交多文件补丁（语法 → 预检 → 顺序提交三阶段）。
    tools.register(Arc::new(onion::tools::ApplyPatchTool {
        sandbox: sandbox.clone(),
        observation,
    }));
    // shell：按平台二选一（Windows = pwsh，POSIX = bash）。注册延后到 approval_config /
    // 审批通道就绪之后——沙箱模式与之同源、越界升级复用同一通道（见下方「shell 工具」）。
    let job_registry = Arc::new(onion_jobs::JobRegistry::new());
    let owner = onion::tools::jobs::owner_cell();
    let background =
        Arc::new(onion::shell_exec::background::BackgroundExecutor::new(job_registry.clone()));
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
    // lsp：精确代码导航（只读；seam 可注册 provider，缺省无 provider）。
    let lsp_service = Arc::new(LspService::new());
    tools.register(Arc::new(LspTool::with_cwd(lsp_service, session_cwd.clone())));
    // terminal_* 6 件：持久 PTY 会话。
    let terminal_service = Arc::new(onion::terminal::TerminalService::new());
    terminal_service
        .register_backend(Arc::new(onion::terminal::PtyBackend::new()))
        .unwrap_or_else(|e| panic!("terminal backend 注册失败: {}", e.message));
    let terminal_tools = onion::terminal::TerminalToolSet::new(terminal_service)
        .with_background_enabled(config.terminal_enable_background);
    terminal_tools.register_all(&mut tools);
    // create_worktree：从本地 commit/branch/tag 建 git worktree 并切会话 cwd。
    let worktree_service = Arc::new(onion::worktree::WorktreeService::with_cwd(session_cwd.clone()));
    tools.register(Arc::new(onion::worktree::CreateWorktreeTool::new(
        worktree_service,
    )));
    // load_workspace_dependencies：查询捆绑运行时的绝对路径与版本。
    let workspace_deps_service = Arc::new(onion::workspace_deps::WorkspaceDepsService::new(
        config.workspace_deps_source.clone().unwrap_or_default(),
    ));
    tools.register(Arc::new(onion::workspace_deps::LoadWorkspaceDependenciesTool::new(
        workspace_deps_service,
    )));
    // memory 工具 7 件（主会话 5 + agent 侧 ListMemories / DeleteMemory，去重
    // 并集见 `session_memory_tools`）。
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
    // a2a_call / a2a_send：A2A 出站工具（对齐 onion-server-a2a-guide §4.4 P0）。
    // a2a_call 只读 GET 探测（ReadOnly）；a2a_send 委派 POST（WorkspaceWrite，实例内自动放行），
    // 配置了 mesh 令牌（`ONION_A2A_TOKEN`）时随请求带 `X-Onion-Token`。
    tools.register(Arc::new(onion::tools::A2aCallTool::new()));
    let a2a_send_tool = Arc::new(onion::tools::A2aSendTool::new(
        config.a2a_token.clone(),
        sessions_dir.clone(),
        onion::paths::instance_id(),
        onion::paths::instance_name(),
    ));
    tools.register(a2a_send_tool.clone());
    // a2a_approve：把跨实例挂起的人机交互项回填给对端（`tasks/provideInput`）。
    tools.register(Arc::new(onion::tools::A2aApproveTool::new(
        config.a2a_token.clone(),
    )));
    // session_* 5 件：会话历史查询（只读；共享接线槽，Runtime 装配后 wire）。
    let session_query = Arc::new(SessionQueryTool::new(config.session_query_max_results));
    for tool in session_query.define_all() {
        tools.register(tool);
    }
    // present：声明已有文件为最终交付物（只读；session-owned durable 事件）。
    let present_tool = Arc::new(PresentTool::new(sandbox, config.present_max_files));
    tools.register(present_tool.clone());
    // skill：加载技能指令（只读；注册表与 catalog / gesture listener 共享同一实例）。
    // 除全局 `<data_home>/skills` 外，已启用 App 的 `contributes.skills` 也注册为
    // 独立 provider（provider 名带 App id，避免与全局技能根互相覆盖）。
    let app_contribs = crate::app_contrib::load();
    for note in &app_contribs.notes {
        eprintln!("[onion-server] App 贡献: {note}");
    }
    let skill_registry = Arc::new(std::sync::Mutex::new(
        SkillRegistry::new().with_disable_policy(std::sync::Arc::new(onion::skill::FileDisablePolicy)),
    ));
    {
        let mut reg = skill_registry
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        reg.register_provider(filesystem_provider())
            .unwrap_or_else(|e| panic!("skill filesystem provider 注册失败: {e}"));
        for provider in &app_contribs.skills {
            reg.register_provider(provider.clone())
                .unwrap_or_else(|e| panic!("App 技能 provider 注册失败: {e}"));
        }
    }
    let skill_tool =
        Arc::new(SkillTool::new(skill_registry.clone()).with_session_cwd(session_cwd.clone()));
    tools.register(skill_tool.clone());
    // working_directory：读 / 改会话工作目录（只读；session-owned durable 事件）。
    let working_dir_tool = Arc::new(WorkingDirectoryTool::with_cwd(session_cwd.clone()));
    tools.register(working_dir_tool);
    // schedule_* 4 件：durable 提醒（Host 独立存储，任务比 Session 活得久）。
    // 存储由注册表持有（进程级单例），多会话共享同一内存投影与同一 `tasks.jsonl`。
    let schedule_service = Arc::new(ScheduleService::new(schedule_store));
    let schedule_tools = onion::schedule::tools::ScheduleToolSet::new(schedule_service.clone());
    schedule_tools.register_all(&mut tools);
    // MCP 资源 3 件 + 工具桥接：server 自动发现（data_home/mcp.json →
    // <workspace>/.onion/mcp.json → ONION_MCP_SERVERS），桥接工具在装配期调
    // `tools/list` 动态注册为 `mcp__<server>__<tool>`。
    let mcp_resource_service = Arc::new(onion::mcp_resources::McpResourceService::new());
    let (mut mcp_servers, mut mcp_discovery_notes) = onion::mcp_resources::discover_servers(
        root.as_path(),
        &onion::paths::data_home(),
        config.mcp_servers_config.as_deref(),
    );
    // App 贡献的 MCP server：与文件发现结果同权装配；重名（含内建优先级）时跳过，
    // 避免同一 server 名出现两份 provider。
    for server in &app_contribs.mcp {
        let name = server.server_name();
        if mcp_servers.iter().any(|existing| existing.server_name() == name) {
            mcp_discovery_notes.push(format!("App MCP server `{name}` 与既有配置重名，已跳过"));
            continue;
        }
        mcp_servers.push(server.clone());
    }
    for server in &mcp_servers {
        let name = server.server_name().to_string();
        let provider = Arc::new(onion::mcp_resources::McpClientProvider::new(server.clone()));
        mcp_resource_service
            .register(&name, provider)
            .unwrap_or_else(|e| panic!("MCP server {name} 注册失败: {e}"));
    }
    let mcp_bridge = Arc::new(onion::mcp_resources::McpBridgeToolSet::new(mcp_servers.clone()));
    let (mcp_bridge_names, bridge_notes) = mcp_bridge.register_all(&tools);
    mcp_discovery_notes.extend(bridge_notes);
    onion::mcp_resources::McpResourceToolSet::new(mcp_resource_service.clone())
        .register_all(&mut tools);
    for note in &mcp_discovery_notes {
        eprintln!("[onion-server] MCP: {note}");
    }

    // 模型目录（list_subagent_models 数据源 + 显式默认模型解析）。
    let provider_catalog = onion::provider_catalog::ProviderCatalog::load(
        &onion::paths::data_home(),
        &config.model,
    );
    // 运行期模型目标：kernel 与 llm 共享同一 ModelTargetRef（对齐 onion SDK
    // assemble 侧做法），REST /model 切换经 swap_current 双侧生效——llm 读
    // assembled 目标路由 base_url/api_key，kernel 在 prompt-assembly 边界快照。
    // 初始目标：providers.json 显式 defaultModel（模型管理 App 设默认）优先，
    // 否则 env 装配的 config.model。
    let initial_target = if provider_catalog.has_custom_default {
        provider_catalog.target_for(&provider_catalog.default_model)
    } else {
        None
    };
    let model_target = Arc::new(onion::model_target::ModelTargetRef::new(
        initial_target.unwrap_or(onion::model_target::ModelTarget {
            model: config.model.clone(),
            base_url: config.base_url.clone(),
            api_key: config.api_key.clone(),
            effort: None,
        }),
    ));
    let llm = Arc::new(
        OpenAiSseLlm::new(config.base_url.clone(), config.api_key.clone())
            .with_target(model_target.clone()),
    );
    // 父会话当前路由（无白名单时子代理显式选择只允许它）。
    // 目录里查不到时按 env 三元组兜底。
    let parent_route = provider_catalog
        .route_for(&config.model, &config.base_url)
        .unwrap_or_else(|| {
            onion::subagent::model_selection::AllowedModelRoute::new("local", &config.model)
        });
    // 子代理模型选择白名单（config 随后被 move 进 Runtime，先取一份）。
    let subagent_model_selection = config.subagent_model_selection.clone();

    // 审批配置（沙箱模式随 `/permission` 切换即时生效，runner / gate 读同一 RwLock）。
    let approval_config = Arc::new(RwLock::new(build_approval_config(
        &config.approval_preset,
        &llm,
        &config.model,
    )));
    // 审批请求通道：沙箱拒绝升级（`SandboxEscalator`）与 `ApprovalGate` 共用同一发送端，
    // 保证裁决路径单一。（提前构造：subagent 子会话 shell 工具复用同一执行器 / 升级通道，
    // 与 assemble 侧同构——子代理的越界操作同样走审批，不因派生而绕过。）
    let (approval_tx, approval_rx) = mpsc::unbounded_channel();
    let shell_executor = {
        let spill_dir = onion::paths::logs_dir(&onion::paths::data_home()).join("jobs");
        let mut executor = OneshotExecutor::new(Some(spill_dir));
        // 可插拔 OS 级沙箱：解析到沙箱 exe（`ONION_SANDBOX_EXE` 或同目录自动识别）时经
        // 独立 exe 隔离执行，否则直通——直通时 ReadOnly / WorkspaceWrite 档在工具层一并
        // 收敛为 Escalating 走显式审批兜底（fail-closed，见 `confine_to_sandbox`）。
        if let Some(executable) = config.sandbox_executable.clone() {
            executor = executor.with_sandbox(Arc::new(ProcessSandboxRunner::new(SandboxConfig {
                executable,
                approval: approval_config.clone(),
            })));
        }
        Arc::new(executor)
    };
    // 沙箱拒绝升级通道（仅配置了 OS 沙箱时启用）：放行后以一次性
    // `danger-full-access` 重放，不改写共享 preset。
    let shell_escalator = config
        .sandbox_executable
        .is_some()
        .then(|| SandboxEscalator { requests: approval_tx.clone() });

    // Agent 目录（内置 + 自定义 YAML 合并）：父会话与子代理共用同一目录——
    // 子代理的 `agent_preset` 白名单与工具面 / persona 都从这里取。
    let agent_catalog = onion::preset::AgentCatalog::merged(
        config.agents_dir.as_deref().map(|d| d.join("agents.yml")).as_deref(),
    )
    .unwrap_or_else(|e| panic!("Agent 目录加载失败: {e}"));
    // 省略 `agent_preset` 的子代理继承父会话预设（省略即沿用部署默认预设）。
    let inherited_preset = agent_id.clone().unwrap_or_else(|| config.agent_preset.clone());
    let preset_choices = agent_catalog.ids();

    // subagent Runtime（会话单例）。子会话工具面 = 宿主文件/搜索/shell 组合
    //（子代理继承宿主组合），与父会话共用 sandbox / 执行器 / 审批通道。
    let subagent_runtime = Arc::new(SubagentRuntime::new(
        1,
        config.clone(),
        llm.clone(),
        {
            let subagent_cwd = session_cwd.clone();
            let child_executor = shell_executor.clone();
            let child_escalator = shell_escalator.clone();
            let child_owner = owner.clone();
            let child_config = config.clone();
            let child_catalog = agent_catalog.clone();
            Arc::new(move |delegated_cwd: Option<&str>, preset: Option<&str>| {
                // 委派 cwd：相对路径基于父会话当前目录解析，须落在权限根内；
                // 子会话用独立 SessionCwd（初始目录派生时固定，不写父会话事件）。
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
                let sandbox = Arc::new(Sandbox::with_cwd(child_session_cwd.clone()));
                let observation = Arc::new(onion::fs_observation::FsObservation::new());
                let registry = ToolRegistry::new();
                registry.register(Arc::new(ReadTool {
                    sandbox: sandbox.clone(),
                    observation: observation.clone(),
                }));
                registry.register(Arc::new(ReadImageTool {
                    sandbox: sandbox.clone(),
                    observation: observation.clone(),
                }));
                registry.register(Arc::new(WriteTool {
                    sandbox: sandbox.clone(),
                    observation: observation.clone(),
                }));
                registry.register(Arc::new(EditTool {
                    sandbox: sandbox.clone(),
                    observation: observation.clone(),
                }));
                registry.register(Arc::new(StrReplaceEditorTool {
                    sandbox: sandbox.clone(),
                    observation: observation.clone(),
                }));
                registry.register(Arc::new(onion::tools::ApplyPatchTool {
                    sandbox: sandbox.clone(),
                    observation,
                }));
                registry.register(Arc::new(GlobTool::new(sandbox.clone())));
                registry.register(Arc::new(GrepTool::new(sandbox)));
                if cfg!(windows) {
                    registry.register(Arc::new(PwshTool {
                        executor: child_executor.clone(),
                        background: None,
                        cwd: child_session_cwd,
                        owner: child_owner.clone(),
                        escalator: child_escalator.clone(),
                    }));
                } else {
                    registry.register(Arc::new(BashTool {
                        executor: child_executor.clone(),
                        background: None,
                        cwd: child_session_cwd,
                        owner: child_owner.clone(),
                        escalator: child_escalator.clone(),
                    }));
                }
                // Agent 预设：显式 `agent_preset` 优先，否则继承父会话预设；
                // 工具面按预设工具名过滤（preset 决定工具组合），
                // persona 作为 `agent:directive` 段进入子会话提示词。
                let preset_id = preset.unwrap_or(&inherited_preset);
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
        Some(onion::paths::subagent_logs_dir(&onion::paths::data_home())),
    ));
    // subagent 工具面（模型目录复用上方 provider_catalog；预设白名单同源目录）。
    let subagent_tools = SubagentToolSet::new(
        subagent_runtime.clone(),
        provider_catalog,
        parent_route,
        preset_choices,
    );
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

    // workflow：声明式 JSON 程序编排（与 onion::assemble 同源接线；此前边车
    // 装配漏注册，模型工具面缺 `workflow`，只能以 subagent 串行降级）。
    // 复用 subagent runtime 派发子代理；`run_in_background` 注册为 jobs 后台
    // 任务（job_kill 经注册表取消钩子取消 CancellationToken）。
    tools.register(Arc::new(onion::tools::WorkflowTool::new(
        subagent_runtime.clone(),
        job_registry.clone(),
        owner.clone(),
    )));
    // tool:workflow 段：说明适用场景与「不是 JS 脚本」的事实。
    prompts
        .section(PromptSection {
            name: "tool:workflow".to_string(),
            order: onion::system_prompt::section_orders::TOOL_WORKFLOW,
            text: SectionText::Static(
                "The workflow tool takes a declarative JSON program (meta + steps), not a script: \
                 steps run in order, each dispatching one fresh subagent, and a step with \
                 `for_each` fans out one subagent per array item concurrently. Use it for \
                 independent parallel research and map-reduce over explicit lists; use `subagent` \
                 for a single delegation. Long workflows may run in the background and are then \
                 inspected with job_output and stopped with job_kill."
                    .to_string(),
            ),
            interpolate: false,
            complete: false,
        })
        .unwrap_or_else(|error| panic!("tool:workflow 段注册失败: {error}"));

    let goal_service = Arc::new(GoalService::new());
    let goal_tools = GoalToolSet::new(goal_service.clone());
    goal_tools.register_all(&mut tools);

    // ask_user：阻塞问询 seam。请求经 `question_rx` 交会话循环：交互式会话
    // 挂起等待 REST 回填，A2A 会话自动答复（fail-closed）。
    let ask_user_service = Arc::new(AskUserService::new());
    let (ask_user_tx, ask_user_rx) = mpsc::unbounded_channel();
    ask_user_service.set_request_channel(ask_user_tx);
    tools.register(Arc::new(AskUserQuestionTool::new(ask_user_service.clone())));

    // Agent 预设：按请求 Agent id（缺省 config.agent_preset）选定父会话工具面。
    let requested_agent = agent_id.unwrap_or_else(|| config.agent_preset.clone());
    let agent = agent_catalog.get(&requested_agent).cloned().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "未知 Agent id `{requested_agent}`（可选：{}）",
                agent_catalog.ids().join(", ")
            ),
        )
    })?;
    // Agent 行为指令（preset system_prompt）：非空时注册为
    // `agent:directive` 段，紧跟 persona、先于 plan/工具策略，让 Agent 的
    // 行为契约（如 creator 的 App 开发工作流）随 Agent 选择生效。
    if !agent.system_prompt.is_empty() {
        prompts
            .section(PromptSection {
                name: "agent:directive".to_string(),
                order: section_orders::AGENT_DIRECTIVE,
                text: SectionText::Static(agent.system_prompt.clone()),
                interpolate: true,
                complete: false,
            })
            .unwrap_or_else(|error| panic!("agent:directive 段注册失败: {error}"));
    }
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

    // 审批配置已在上方构造（shell_executor / ApprovalGate 读同一 RwLock），此处直接复用。
    // shell 工具：注册在此处（而非文件工具同批），因为执行器需要与 `ApprovalGate` 共享的
    // `approval_config`（沙箱模式随 `/permission` 切换即时生效）与审批通道（越界升级）。
    // 风险上界 Escalating；只读 / 工作区写命令经 `classify_risk` 降到 ReadOnly /
    // WorkspaceWrite 直通。一次性 fresh 进程，输出 spill；run_in_background 晋升后台任务。
    // 执行器 / 升级通道已在上方构造（subagent 子会话复用同一套）。
    {
        // bash 是 POSIX 系统命令，Windows 侧走 pwsh / cmd；且 msys bash 在受限 token
        // 沙箱下无法创建信号管道（Win32 error 5）而直接启动失败，故 Windows 不注册。
        let background = Some(background.clone());
        if cfg!(windows) {
            tools.register(Arc::new(PwshTool {
                executor: shell_executor,
                background,
                cwd: session_cwd.clone(),
                owner: owner.clone(),
                escalator: shell_escalator,
            }));
        } else {
            tools.register(Arc::new(BashTool {
                executor: shell_executor,
                background,
                cwd: session_cwd.clone(),
                owner: owner.clone(),
                escalator: shell_escalator,
            }));
        }
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

    // 风险分类器：按调用内容判定。
    let classify_registry = tools.clone();
    let approval_gate = ApprovalGate::new(
        approval_config.clone(),
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
            // skill 手势 listener 先于 catalog。
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
    // 会话 id 显式指定：`open_with_replay` 对不存在的日志等价新建，保证
    // `SessionRegistry` 的 id 与 Runtime 内的 session_id 始终一致。
    let mut rt = Runtime::resume(
        config,
        prompts,
        context,
        &sessions_dir,
        &session_id,
        llm.clone(),
        tools,
    )?;
    // resume 内部自建静态模型目标；置换为与 llm 共享的实例（装配期 kernel 锁空闲）。
    rt.set_model_target_sync(model_target.clone())?;
    // spill 工件库：超大工具结果持久化（#14；未接线时结果原文直通）。
    rt.set_spill_store_sync(Arc::new(onion::spill::SpillStore::new(
        onion::paths::spill_dir(&onion::paths::data_home()),
        onion::spill::store::DEFAULT_MAX_INLINE_TOKENS,
    )))?;
    for listener in listeners() {
        rt = rt.with_listener(listener);
    }
    // hooks 配置：用户级（`<ONION_HOME>/hooks.json` + settings.json 的 hooks 键）
    // 与工作区级（`<workspace>/.onion/hooks.json`）分层发现，再叠加 App 贡献
    // （`${CLAUDE_PLUGIN_ROOT}` 已按各自 App 根目录替换为绝对路径）。
    let (discovered_hooks, hook_notes) =
        onion::hooks::discover::discover_hooks(&root, &onion::paths::data_home());
    for note in &hook_notes {
        eprintln!("[onion-server] hooks: {note}");
    }
    let hooks_raw = match (discovered_hooks, app_contribs.hooks_config(&root)) {
        (None, None) => None,
        (Some(base), None) => Some(base),
        (None, Some(app)) => Some(app),
        (Some(base), Some(app)) => Some(onion::hooks::discover::merge_hooks(Some(base), app)),
    };
    if let Some(hooks_raw) = hooks_raw {
        let vars = onion::hooks::config::SubstitutionVars {
            plugin_root: None,
            project_dir: None,
        };
        match onion::hooks_claude::ClaudeCodeHooksListener::from_json(&hooks_raw, &vars, rt.session())
        {
            Ok(listener) => rt = rt.with_listener(Arc::new(listener)),
            Err(error) => eprintln!("[onion-server] hooks 配置无效，已跳过: {error}"),
        }
    }
    let runtime = Arc::new(rt);
    eprintln!("[onion-server] 会话日志目录：{}", sessions_dir.display());

    // 后台任务接线：owner 锚定会话 id + 完成通知注入父会话 inbox。
    onion::tools::jobs::wire_owner(&owner, runtime.session().session_id());
    // a2a_send owner 接线：委派链接（`tasks/links.jsonl`）与 `reportTo` 需要会话 id。
    a2a_send_tool.wire_owner(runtime.session().session_id());
    // terminal owner 接线。
    terminal_tools.wire_owner(runtime.session().session_id());
    terminal_tools.wire_background(job_registry.clone());
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
    // agent-team 延迟接线。
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
        goal_service.clone(),
        runtime.inbox(),
        runtime.session(),
    ));
    {
        let inflight_runtime = subagent_runtime.clone();
        goal_driver.wire_subagents(Arc::new(move || inflight_runtime.inflight() > 0));
    }
    plan_mode.wire_session(runtime.session());
    // 会话目录单一权威接线：重放 `WorkingDir` / `WorktreeChange`（worktree 服务与
    // working_directory 工具共享同一句柄，故只需接线一次）。
    session_cwd.wire_session(runtime.session());
    // todo / ask_user / session_query / present / skill / schedule 延迟接线：
    // 锚定会话（durable 事件写入 + 调用方校验）。
    todo_tool.wire_session(runtime.session());
    ask_user_service.wire_session(runtime.session());
    session_query.wire_session(runtime.session());
    present_tool.wire_session(runtime.session());
    skill_tool.wire_session(runtime.session());
    schedule_tools.wire_session(runtime.session());

    let gate = Arc::new(crate::turn_gate::TurnGate::new());
    let (done_tx, done_rx) = mpsc::unbounded_channel::<TurnReport>();
    let channels = Arc::new(SessionChannels::new(interactive));
    let job_owner = onion_jobs::SessionId(runtime.session().session_id().to_string());

    let handle = Arc::new(SessionHandle {
        runtime,
        channels,
        compact_tx,
        compaction_listener,
        tools_handle,
        goal_service,
        goal_driver,
        plan_mode,
        gate,
        done_tx,
        active_agent: Arc::new(tokio::sync::Mutex::new(active_agent_id)),
        agent_catalog,
        session_cwd,
        model_target,
        approval_config,
        llm,
        skill_registry,
        schedule_service,
        job_registry,
        job_owner,
        mcp_bridge,
        mcp_resources: mcp_resource_service,
    });
    // 父会话唤醒回调：子代理 settle 回投 relay/notice 后立即驱动父会话消费，
    // 避免父会话 idle 时回投滞留到下一次用户输入。
    //
    // 用 Weak 断开 `SessionHandle → SubagentRuntime → wake → SessionHandle` 的循环引用，
    // 会话销毁后回调自动失效。重复唤起由 `TurnGate::try_acquire_driver` 丢弃。
    {
        let weak = Arc::downgrade(&handle);
        subagent_runtime.set_parent_wake(Arc::new(move || {
            if let Some(handle) = weak.upgrade() {
                spawn_turn(&handle);
            }
        }));
    }
    // 投递分区选择：
    // settle 时刻据此判定父会话是否还有在飞/排队的轮次——空闲则落 next-turn 排队，
    // 运行中则落 next-step 插队。同样用 Weak 断开循环引用。
    {
        let weak = Arc::downgrade(&handle);
        subagent_runtime.set_parent_idle(Arc::new(move || {
            weak.upgrade().is_some_and(|handle| !handle.gate.is_busy())
        }));
    }
    Ok((
        handle,
        approval_rx,
        plan_review_rx,
        ask_user_rx,
        done_rx,
    ))
}

/// 常驻 kick 驱动（对齐 CLI `spawn_turn`）：
/// 以空轮启动，由 kernel 内部 `claim_open()`/`drain_next_step()` 消费 inbox 排队
/// 消息。所有外部输入统一经 inbox 投递，由本驱动（或主循环 select 的
/// inbox.notified() 分支）开轮，无带 input 直驱。
///
/// driver 唯一性由 `TurnGate` 的 CAS 保证：抢不到说明已有 driver 在消费 inbox，
/// 直接返回（只在 idle 时起 driver）。
pub(crate) fn spawn_turn(handle: &Arc<SessionHandle>) {
    if !handle.gate.try_acquire_driver() {
        return;
    }
    let handle = handle.clone();
    let runtime = handle.runtime.clone();
    let done = handle.done_tx.clone();
    let gate = handle.gate.clone();
    let channels = handle.channels.clone();
    let goal_driver = handle.goal_driver.clone();
    let plan_mode = handle.plan_mode.clone();
    tokio::spawn(async move {
        loop {
            // Ok(None) = 无轮可开（空输入且无排队消息，正常空转）；Err = 状态机
            // 拒绝开轮，必须给 stderr 反馈，否则 spawn 静默死亡、等待者悬挂。
            let report = match runtime.run(Vec::new()).await {
                Ok(Some(report)) => report,
                Ok(None) => break,
                Err(error) => {
                    eprintln!("[onion-server] turn 启动失败：{error}");
                    break;
                }
            };
            // 先把本轮摘要落到 `last_report`，再推进闸门：`wait=true` 的 HTTP
            // 请求在 `complete_one()` 唤醒后立刻读摘要，若等异步 `done` 消费者
            // 写入会读到上一轮（或 `None`）。
            channels.set_last_report(TurnSummary {
                turn: report.turn,
                steps: report.steps,
                reason: report.reason_label(),
                usage: report.usage,
            });
            // 本轮完成：推进计数并在追平时唤醒 `wait=true` 等待者。
            gate.complete_one();
            if matches!(report.reason, onion::kernel::TurnEndReason::MaxTokens)
                && goal_driver.disarm()
            {
                eprintln!("[onion-server] goal 因 max-tokens 结束已 disarm");
            }
            plan_mode.flush_pending();
            let turn = report.turn;
            // 每轮结束尝试命名（幂等：已有标题即跳过），不阻塞闸门与下一轮。
            crate::title::spawn_auto_title(handle.clone());
            if done.send(report).is_err() {
                break;
            }
            if runtime.inbox().has_next_turn() {
                eprintln!("[onion-server] next-turn 有排队消息，开新一轮 t{}", turn + 1);
                continue;
            }
            if runtime.inbox().has_next_step() && runtime.inbox().take_wakeup() {
                eprintln!("[onion-server] next-step 有引导消息，开新一轮 t{}", turn + 1);
                continue;
            }
            // goal 自动续轮。
            if goal_driver.should_continue() {
                if let Some(message) = goal_driver.continuation_input() {
                    eprintln!("[onion-server] goal 自动续轮，开新一轮 t{}", turn + 1);
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
        // 收尾兜底：收敛瞬间若 inbox 仍有排队消息，释放 driver 后重抢，防
        // 「队列有 pending 但无人开轮」。
        if runtime.inbox().has_pending() {
            gate.release_driver();
            spawn_turn(&handle);
            return;
        }
        // 无排队：追平计数并唤醒等待者（防 `wait=true` 悬挂），再释放 driver。
        gate.drain_remaining();
        gate.release_driver();
    });
}

/// 非交互会话的确认请求路由：有 `reportTo` 且对端在 peers 表才算「有委派方」。
async fn route_for(
    channels: &SessionChannels,
    bridge: &crate::bridge::Bridge,
    report_to: &tokio::sync::Mutex<Option<serde_json::Value>>,
) -> Option<crate::bridge::Route> {
    if channels.interactive {
        return None;
    }
    let target = report_to.lock().await.clone();
    bridge.route(target.as_ref())
}

/// 直推确认请求到委派方裁决会话（携带本会话最近一次 task id）。
async fn push_confirm(
    bridge: &crate::bridge::Bridge,
    route: &crate::bridge::Route,
    current_task: &tokio::sync::Mutex<Option<TaskId>>,
    request_id: &str,
    kind: &PendingKind,
) {
    let Some(task_id) = current_task.lock().await.clone() else {
        eprintln!("[onion-server] 确认请求未推送：会话无在飞 task");
        return;
    };
    bridge.push_confirm(route, task_id.as_str(), request_id, kind);
}

/// 会话循环：审批 / plan-review / 问询 / turn 完成 / live 事件 → 会话帧扇出。
///
/// `interactive=false`（A2A 会话）：工具执行实例内自动放行；`plan-review` /
/// `ask-user` 若有 `reportTo` 且对端在 peers 表，则登记待回填并直推委派方裁决，
/// 否则保持原 fail-closed（plan KeepPlanning / 自动答复）。
/// `interactive=true`（REST 会话）把请求挂到待回填表，等 REST 端点回填。
pub(crate) fn spawn_session_loops(
    channels: Arc<SessionChannels>,
    bridge: Arc<crate::bridge::Bridge>,
    report_to: Arc<tokio::sync::Mutex<Option<serde_json::Value>>>,
    current_task: Arc<tokio::sync::Mutex<Option<TaskId>>>,
    mut live_rx: broadcast::Receiver<LiveEvent>,
    mut approval_rx: mpsc::UnboundedReceiver<ApprovalRequest>,
    mut plan_review_rx: mpsc::UnboundedReceiver<onion::plan::service::PlanReviewRequest>,
    mut question_rx: mpsc::UnboundedReceiver<onion::ask_user::service::AskUserRequest>,
    mut done_rx: mpsc::UnboundedReceiver<TurnReport>,
    handle: Arc<SessionHandle>,
) {
    // 审批请求。
    {
        let channels = channels.clone();
        tokio::spawn(async move {
            while let Some(request) = approval_rx.recv().await {
                if !channels.interactive {
                    // 工具执行在实例内自动放行（范围收窄：跨实例只推人机交互项）。
                    eprintln!(
                        "[onion-server] ⏸ 待审批 {} — 非交互会话，实例内自动放行",
                        request.name
                    );
                    let _ = request.reply.send(onion::approval::ApprovalDecision::AllowedOnce);
                    continue;
                }
                let (tx, rx) = oneshot::channel();
                let kind = PendingKind::Approval {
                    tool: request.name.clone(),
                    input: request.input.clone(),
                    reason: request.reason.clone(),
                };
                let request_id = channels.push(kind, ReplySender::Approval(tx));
                channels.notice(SessionNotice::ApprovalRequested {
                    request_id,
                    tool: request.name.clone(),
                    input: request.input.clone(),
                    reason: request.reason.clone(),
                });
                let reply = request.reply;
                tokio::spawn(async move {
                    if let Ok(decision) = rx.await {
                        let _ = reply.send(decision);
                    }
                });
            }
        });
    }
    // plan-review 请求。
    {
        let channels = channels.clone();
        let bridge = bridge.clone();
        let report_to = report_to.clone();
        let current_task = current_task.clone();
        tokio::spawn(async move {
            while let Some(review) = plan_review_rx.recv().await {
                let route = route_for(&channels, &bridge, &report_to).await;
                if !channels.interactive && route.is_none() {
                    eprintln!(
                        "[onion-server] ⏸ 待审批 exit_plan_mode — 非交互会话无委派方，保持 plan 模式"
                    );
                    let _ = review.reply.send(onion::plan::service::PlanVerdict::KeepPlanning);
                    continue;
                }
                let (tx, rx) = oneshot::channel();
                let kind = PendingKind::PlanReview {
                    plan: review.plan.clone(),
                };
                let request_id = channels.push(kind.clone(), ReplySender::PlanReview(tx));
                channels.notice(SessionNotice::PlanReviewRequested {
                    request_id: request_id.clone(),
                    plan: review.plan.clone(),
                });
                if let Some(route) = route {
                    push_confirm(&bridge, &route, &current_task, &request_id, &kind).await;
                }
                let reply = review.reply;
                tokio::spawn(async move {
                    if let Ok(verdict) = rx.await {
                        let _ = reply.send(verdict);
                    }
                });
            }
        });
    }
    // ask_user 问询请求。
    {
        let channels = channels.clone();
        tokio::spawn(async move {
            while let Some(request) = question_rx.recv().await {
                let route = route_for(&channels, &bridge, &report_to).await;
                if !channels.interactive && route.is_none() {
                    eprintln!(
                        "[onion-server] ⏸ 待人工问询 {} — 非交互会话无委派方，自动答复",
                        request.call_id
                    );
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
                    continue;
                }
                let (tx, rx) = oneshot::channel();
                let kind = PendingKind::Question {
                    call_id: request.call_id.clone(),
                    questions: request.questions.clone(),
                };
                let request_id = channels.push(kind.clone(), ReplySender::Question(tx));
                channels.notice(SessionNotice::QuestionRequested {
                    request_id: request_id.clone(),
                    call_id: request.call_id.clone(),
                    questions: request.questions.clone(),
                });
                if let Some(route) = route {
                    push_confirm(&bridge, &route, &current_task, &request_id, &kind).await;
                }
                let reply = request.reply;
                let call_id = request.call_id.clone();
                let questions = request.questions.clone();
                let handle = handle.clone();
                tokio::spawn(async move {
                    let Ok(answer) = rx.await else { return };
                    if reply.send(answer.clone()).is_ok() {
                        return;
                    }
                    // timed 前台等待已逾限（工具侧 receiver 已 drop）：迟到答复转为
                    // `AnswerToPending` user 消息注入 inbox。
                    let message =
                        onion::ask_user::answer_to_pending_message(&call_id, &questions, &answer);
                    if let Err(error) = handle.runtime.inbox().steer(message) {
                        eprintln!("[onion-server] 迟到答复注入失败：{error}");
                        return;
                    }
                    // 会话可能已空闲（timed 逾限那一轮已结束）：唤醒 driver 开轮
                    // 消费 next-step 分区。
                    spawn_turn(&handle);
                });
            }
        });
    }
    // turn 完成通知。
    {
        let channels = channels.clone();
        tokio::spawn(async move {
            while let Some(report) = done_rx.recv().await {
                let reason = report.reason_label();
                eprintln!(
                    "[onion-server] [turn] t{} · {} steps · {reason}",
                    report.turn, report.steps
                );
                channels.notice(SessionNotice::TurnReport {
                    turn: report.turn,
                    steps: report.steps,
                    reason,
                    usage: report.usage,
                });
            }
        });
    }
    // live 事件转发（不持有 SessionHandle，避免会话删除后泄漏）。
    tokio::spawn(async move {
        loop {
            match live_rx.recv().await {
                Ok(LiveEvent::Durable(event)) => channels.notice(SessionNotice::Durable {
                    ts: onion::session::epoch_millis(),
                    event,
                }),
                Ok(event) => channels.notice(SessionNotice::Live { event }),
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    eprintln!("[onion-server] notice 订阅滞后，跳过 {skipped} 个事件");
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

/// 写入容器 ↔ 工作区映射（`<sessions_dir>/cwd`）。
///
/// 会话目录名由工作区路径归一化得到（有损），而会话历史要按工作区分组展示，
/// 因此在会话创建时把绝对路径写入容器；同容器重复创建幂等覆盖，写入失败不阻塞创建。
pub(crate) fn write_workspace_marker(sessions_dir: &Path, cwd: &Path) {
    let _ = std::fs::create_dir_all(sessions_dir)
        .and_then(|()| std::fs::write(sessions_dir.join("cwd"), cwd.to_string_lossy().as_bytes()));
}

/// 读取容器 ↔ 工作区映射（`<sessions_dir>/cwd`）；缺失或空白返回 `None`。
pub(crate) fn read_workspace_marker(sessions_dir: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(sessions_dir.join("cwd")).ok()?;
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

/// 落盘容器 → 权限根（纯函数，便于单测）。
///
/// 容器名是 `normalize_cwd(root)` 的有损转义（分隔符/冒号/空白统一变 `-`），
/// 无法反解，故冷启动以创建期写入的 `cwd` 标记为准。标记缺失、内容非绝对路径
/// 或目录已不存在时回落兜底根（`cd` 仍受该根约束，不会越界）。
fn container_root(container: &Path, default_dir: &Path, default_root: &Path) -> PathBuf {
    if container == default_dir {
        return default_root.to_path_buf();
    }
    read_workspace_marker(container)
        .filter(|path| path.is_absolute() && path.is_dir())
        .unwrap_or_else(|| default_root.to_path_buf())
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
