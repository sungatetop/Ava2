//! 会话持久日志：durable 事实（内存投影 + JSONL 追加）与 live 广播总线。
//!
//! 「model-visible means logged」：模型可见历史只能从 durable 日志投影（见 context.rs）。

use std::fs::{File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::broadcast;

use crate::listener::EventSink;
use crate::types::{AgentMessage, ToolCall, ToolResult, Usage};

/// durable 会话事实（回放重建 Context 的唯一权威）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    SessionStarted { session_id: String, model: String },
    /// 模型目标切换（`model/selection`）。log-only：投影不消费
    /// （请求装配目标由 ModelTargetRef 持有，本事件仅作审计/回放视图）。
    ModelSelection { model: String, base_url: String, effort: Option<String> },
    TurnStart { turn: u32 },
    SystemMessage { turn: u32, step: u32, message: String },
    UserMessage { turn: u32, step: u32, message: AgentMessage },
    AssistantMessage { turn: u32, step: u32, message: AgentMessage, usage: Usage },
    AssistantAttempt { turn: u32, step: u32, error: String, cancelled: bool },
    ToolCall { turn: u32, step: u32, call: ToolCall },
    ToolResult { turn: u32, step: u32, result: ToolResult },
    StepStart { turn: u32, step: u32 },
    StepEnd { turn: u32, step: u32 },
    TurnEnd { turn: u32, reason: String },
    SessionEnd { reason: String },
    /// 压缩事务开始（`compaction/start`：持久化锁）。
    CompactionStart { turn: u32, compaction_id: String, trigger: String },
    /// 压缩摘要提交（`compaction/summary`：替换区间 + 完整调用信封）。
    CompactionSummary { turn: u32, compaction_id: String, summary: String,
        shadowed_count: usize, shadowed_tokens: u64, usage: Usage },
    /// 压缩事务结束（`compaction/end`：释放锁，`error?` 记录失败尝试）。
    CompactionEnd { turn: u32, compaction_id: String, error: Option<String> },
    /// 压缩裁剪（`compaction/prune`）。
    /// log-only：投影不消费（视图级变换在请求装配期确定性重放）。
    CompactionPruned { pruned: u32, saved_tokens: u64 },
    /// 命令钩子被调用（`hook/invoked`）。log-only：投影不消费。
    HookInvoked {
        point: String,
        handler_id: String,
        matcher: Option<String>,
    },
    /// 命令钩子执行结果（与 `HookInvoked` 由 `handler_id` 配对；`hook/result`）。
    /// log-only：投影不消费。
    HookResult {
        point: String,
        handler_id: String,
        decision: String,
        exit_code: Option<i32>,
        stderr_summary: Option<String>,
        duration_ms: u64,
    },
    /// 工作空间指令基线注入（agent-instructions baseline）。
    /// log-only：投影不消费（消息本体经 UserMessage 投影）。
    WorkspaceInstructions { files: Vec<String>, bytes: usize, version: String },
    /// 工作空间指令变更（触点对账结果；log-only）。
    WorkspaceChanged { added: Vec<String>, removed: Vec<String>, changed: Vec<String> },
    /// goal 全量快照变更（`goal/change`；每次变更一条；clear 时 snapshot 为 None）。
    GoalChange {
        operation: String,
        snapshot: Option<crate::goal::types::GoalSnapshot>,
        rounds_started: u32,
        created_at_ms: u64,
        updated_at_ms: u64,
    },
    /// plan 模式开关（log-only 全值替换；最后一条生效，resume/fork 折叠恢复）。
    PlanMode { active: bool },
    /// plan 模式「期望状态」（log-only；宿主 `/plan` 命令写入，pre-step 边界
    /// 追加，pending 可重放推导：`wanted != active` 即有待生效的切换）。
    PlanWanted { active: bool },
    /// plan 回合结束（log-only；`plan/run-done`，投影 running 置 false）。
    PlanRunDone {},
    /// todo 整表快照（`todo/write`；last-write-wins，`turn/start` 清空）。
    TodoWrite { todos: Vec<crate::todo::types::TodoItem> },
    /// 审批审计（log-only；dispatch/audit 语义）：每次工具准入裁决一条。
    /// `decision` = `allowed` / `denied`；`via` = 裁决来源
    /// （`sandbox` 允许集直通 / `restriction` 工具限制规则 / `policy` 审批策略
    /// / `operator` 人工复决 / `reviewer` 裁决者 seam / `fail-closed` 异常拒绝）。
    /// 投影不消费——审计是运维事实，不进模型上下文。
    ApprovalAudited { tool: String, decision: String, via: String },
    /// 交付物声明（`deliverables/presented`）：`present` 工具成功后
    /// append，记录声明的文件（path + 可选 description）与归属回合。
    /// log-only：投影不消费（工具结果本身已含 files，模型侧可见）。
    Presented { turn: u64, call_id: String, files: Vec<serde_json::Value> },
    /// 会话工作目录变更（`working_directory`）：`cd` 成功后 append，
    /// 记录变更后的 cwd。投影 last-write-wins（重放恢复当前 cwd）；log-only。
    WorkingDir { cwd: String },
    /// git worktree 创建（`create_worktree`）：成功后 append，记录
    /// 切换后的新 checkout 路径。投影 last-write-wins（重放恢复当前 cwd）；
    /// log-only（工具结果本身已含 path/branch）。
    WorktreeChange { path: String },
    /// agent-team 成员全量快照变更（`team/member`）。log-only：
    /// 投影不消费（团队状态由 TeamService 独立投影持有）。
    TeamMemberChange { team_id: String, member: crate::agent_team::types::TeamMemberSnapshot },
    /// agent-team 任务全量快照变更（`team/task`）。log-only。
    TeamTaskChange { team_id: String, task: crate::agent_team::types::TeamTaskSnapshot },
    /// 会话标题（`session/title`）：自动命名为首轮完成后的一次轻量
    /// 模型调用结果，也可由宿主手工重命名写入。log-only 全值替换（最后一条
    /// 生效）；列表标题优先取该事件，缺省回落到首条用户消息截断。
    SessionTitle { title: String },

    /// fork 播种标记（`session/end-seed { inherited: true }`）：子会话
    /// 日志在继承前缀（源会话 `[1..=boundary]` 事件复制）之后追加一条，声明
    /// 血缘与继承规模（`inheritedEventCount` 折叠进本事件持久化）。log-only：
    /// 投影不消费（前缀事件本身已重建上下文投影）；查询面据此输出 lineage。
    ForkSeed {
        /// 源（父）会话 id。
        parent_session_id: String,
        /// 源会话继承边界 seq（含；1-based，复制 `[1..=boundary]` 前缀）。
        parent_boundary_seq: u64,
        /// 继承前缀条数（不含本标记与其后的合成关闭事件）。
        inherited_events: u64,
    },

    /// 斜杠命令开始（`command/run`）：宿主执行命令前 append。
    /// log-only：投影不消费（命令不进入模型上下文）。
    CommandRun {
        command_id: String,
        name: String,
        args: Option<String>,
    },
    /// 斜杠命令结束（`command/done`）：`kind` = `success` / `error`，
    /// `text` 为面向人的回执。log-only。
    ///
    /// `source_event_seq` = 本命令产生的**首条**领域事件在 durable 日志中的下标
    /// （对齐 `sourceEventSeq` 契约）：客户端据此渲染富卡片（压缩标记 / 目标横幅 /
    /// 计划状态）而非回执文本；命令未产生领域事件时为 None。
    CommandDone {
        command_id: String,
        kind: String,
        text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source_event_seq: Option<u64>,
    },
    /// 审批预设变更（`approval/policy` + `sandbox/mode` +
    /// `permission/preset` 三事件的 onion 折叠形态：预设即「沙箱级别 + 审批策略
    /// + 工具限制」的组合，一条事件即可重建会话有效预设）。
    /// log-only：投影不消费（不进模型上下文）；最后一条生效。
    ///
    /// `source = Some("delegation")` 表示该预设是委派时播种进**子会话**的覆盖
    /// （对齐 `source?: 'delegation'` 契约）；缺省为该会话自身的运行期切换。
    ApprovalPreset {
        preset: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<String>,
    },
    /// 用户反馈（`feedback/record`）：`/feedback <text>` 写入的一条
    /// log-only durable 事实。正文按去首尾空白后记录，空文本记为缺失（省略）；
    /// `category` 为可选分类（onion 命令面暂不暴露分类参数）。
    /// 投影不消费——反馈是产品侧信号，不进模型上下文。
    FeedbackRecord {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        category: Option<String>,
    },
    /// 子代理 LLM 路由白名单（`subagent/model-selection-policy`）：
    /// 记录本会话允许显式选择的子代理 provider/model 精确路由，在首个模型请求
    /// 之前 append 一次；**缺失**时显式选择只允许父会话当前路由。子会话继承
    /// 父会话的白名单。log-only：投影不消费（不进模型上下文）。
    SubagentModelSelectionPolicy {
        allowed_models: Vec<crate::subagent::model_selection::AllowedModelRoute>,
    },
    /// 子代理描述符（`subagent/descriptor`）：写入**子会话自己的**日志，
    /// 在首个请求之前 append 一次。log-only：投影不消费（不进模型上下文），
    /// 但冷恢复后据此可枚举出子代理的元信息（不再依赖父进程内存）。
    ///
    /// 字段是显式快照而非可扩展对象（无关字段不得让恢复失败）；
    /// `version` 变更即格式变更。`mode = "continuable"` 表示该子会话可冷恢复续聊；
    /// 历史日志里的 `one-shot` 形态仍可回放，但恢复时按不可续跑处理
    /// （见 [`crate::subagent::runtime::SubagentRuntime::restore`]）。
    SubagentDescriptor {
        version: u32,
        mode: String,
        /// 建立该子代理的 provider 名（onion 恒为 `session`）。
        provider: String,
        /// 模型可读代理名（`name`）：冷恢复后据此补登活跃表。
        ///
        /// 历史日志无该字段（`None`）时回落到会话 id。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        /// 委派时的短期描述（`label`）：冷恢复枚举的展示标签。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        /// 子代理实际使用的模型 id（路由覆盖优先，否则继承宿主）。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent_model: Option<String>,
        /// 子代理 reasoning effort（路由覆盖声明时）。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent_reasoning_effort: Option<String>,
        /// 委派指定的工作目录（None = 继承父会话）。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cwd: Option<String>,
        /// 父会话 id 与相对深度（onion 无 header，故随描述符一起留痕）。
        parent_session_id: String,
        depth: u32,
        /// 委派方式（`delegationMode`）：`spawn` = 全新上下文、
        /// `fork` = 继承父会话已完成回合前缀。冷恢复据此区分血缘类型
        /// （fork 子会话的历史已含父前缀，恢复时只回放自身日志即可）。
        ///
        /// 历史日志无该字段（`None`）时视为 `spawn`。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        delegation_mode: Option<String>,
    },
}

/// `subagent/descriptor` 的当前格式版本（`SUBAGENT_DESCRIPTOR_VERSION` 语义）。
pub const SUBAGENT_DESCRIPTOR_VERSION: u32 = 1;

impl SessionEvent {
    /// 是否为 **surface 事件**（`SurfaceEventType` 白名单）。
    ///
    /// 仅 4 类事件允许携带 `surfaceOp` / `sourceEventSeqs`（`system/message`、
    /// `user/message`、`assistant/message`、`tool/result`），其余事件类型在类型层
    /// 被约束为 `surfaceOp?: never`。onion 没有 surfaceOp 字段（投影由
    /// [`crate::context::Context::apply`] 按事件类型分支完成），但「哪些事件进入
    /// 模型可见上下文」是同一份白名单——本方法把它显式化，供读面与测试判定。
    ///
    /// 注意 `CompactionSummary` 不是 surface 事件：它不新增消息，而是**改写**
    /// 已有 surface 区间（等价 `surfaceOp: replace` 动作）。
    pub fn is_surface(&self) -> bool {
        matches!(
            self,
            Self::SystemMessage { .. }
                | Self::UserMessage { .. }
                | Self::AssistantMessage { .. }
                | Self::ToolResult { .. }
        )
    }
}

/// live 瞬时事件（broadcast 分发；滞后订阅者按 Lagged 丢弃）。
/// 只实现 `Serialize`（server 侧序列化经 stdio 发给 SDK client；client 端
/// 定义同构独立类型反序列化，不依赖引擎）。
#[derive(Clone, Debug, Serialize)]
pub enum LiveEvent {
    StateChanged { from: String, to: String },
    Durable(SessionEvent),
    InboxInserted { partition: &'static str, message: AgentMessage },
    InboxClaimed { partition: &'static str, messages: Vec<AgentMessage> },
    AssistantStream(StreamChunk),
    /// subagent 生命周期开始（`subagent/start`）：瞬时、observe-only，
    /// 前端据此即时渲染子代理卡片，无需轮询 `list_agents`。
    SubagentStart {
        /// 本次运行的唯一标识（同一 start / end 共用）。
        run_id: String,
        /// subagent 会话 id（`send_message` / `wait_agent` 的寻址键）。
        agent_id: String,
        /// 模型可读代理名。
        name: String,
        depth: u32,
        /// 父会话 id（顶层派生时为宿主会话）。
        parent_session_id: String,
    },
    /// subagent 生命周期结束（`subagent/end`）：与 `SubagentStart`
    /// 以 `run_id` 配对，携带 stopReason 与最终 assistant 文本摘要。
    SubagentEnd {
        run_id: String,
        agent_id: String,
        name: String,
        depth: u32,
        parent_session_id: String,
        /// `completed` / `aborted` / `error` / `refusal` / `max-tokens`。
        stop_reason: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        last_assistant_message: Option<String>,
    },
}

/// assistant 流式帧（live）。
#[derive(Clone, Debug, Serialize)]
pub enum StreamChunk {
    Start { turn: u32, step: u32 },
    Text(String),
    Thinking(String),
    ToolCallStart { index: usize, id: String, name: String },
    ToolCallInput { index: usize, partial_json: String },
    End,
}

const BUS_CAPACITY: usize = 256;

/// 事件总线封装：send 忽略无订阅者错误。
#[derive(Clone, Debug)]
pub struct EventBus {
    tx: broadcast::Sender<LiveEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(BUS_CAPACITY);
        Self { tx }
    }

    pub fn send(&self, event: LiveEvent) {
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<LiveEvent> {
        self.tx.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

/// 会话日志：durable 事件追加（内存 + JSONL），同时镜像进 live 总线。
pub struct SessionLog {
    events: Mutex<Vec<SessionEvent>>,
    file: Mutex<Option<File>>,
    started: Mutex<bool>,
    session_id: String,
    bus: EventBus,
    sink: Mutex<Option<EventSink>>,
    /// 跨进程写租约：文件形态会话在 `attach` 时获取，
    /// `close()`/Drop 释放；`in_memory` 与只读回放恒空。
    lease: Mutex<Option<crate::session_lock::SessionWriteLease>>,
    /// 撕裂尾修复目标：回放后剩余的未提交字节
    /// 偏移，首个追加动作执行截断；失败保留待下次追加重试。
    torn_truncate_to: Mutex<Option<u64>>,
}

impl std::fmt::Debug for SessionLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // EventSink（Arc<dyn Fn>）与文件句柄无有意义的 Debug 形态，只报告存在性。
        f.debug_struct("SessionLog")
            .field("session_id", &self.session_id)
            .field("events", &lock(&self.events).len())
            .field("started", &lock(&self.started))
            .field("has_file", &lock(&self.file).is_some())
            .field("has_lease", &lock(&self.lease).is_some())
            .field("torn_truncate_to", &lock(&self.torn_truncate_to))
            .finish()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl SessionLog {
    /// 新建会话日志：在 `sessions_dir` 下建 `<encoded-session-id>/` 独立目录并
    /// 追加 `session.v1.jsonl`（磁盘布局：
    /// `<root>/sessions/<normalized-cwd>/<encoded-id>/session.vN.jsonl`）。
    pub fn open(sessions_dir: &Path) -> io::Result<Self> {
        std::fs::create_dir_all(sessions_dir)?;
        Self::attach(sessions_dir, &fresh_session_id())
    }

    /// 恢复既有会话：定位 `<sessions_dir>/<encoded-id>/session.vN.jsonl` 回放
    /// durable 事件重建内存投影（追加模式续写，文件不存在等价新建）。
    pub fn open_with_replay(sessions_dir: &Path, session_id: &str) -> io::Result<Self> {
        std::fs::create_dir_all(sessions_dir)?;
        Self::attach(sessions_dir, session_id)
    }

    /// 只读回放既有会话（不打开写句柄、不取写租约、不建目录）：
    /// 冷恢复枚举快照等「非写者」场景使用，避免与后续真正的写者实例冲突。
    /// 日志文件不存在即 `NotFound`。
    pub fn replay_only(sessions_dir: &Path, session_id: &str) -> io::Result<Self> {
        let dir = crate::paths::session_dir(sessions_dir, session_id);
        let content = std::fs::read(crate::paths::session_log_file(&dir))?;
        let mut events = Vec::new();
        Self::replay_lines(&content, &mut events)?;
        Ok(Self {
            events: Mutex::new(events),
            file: Mutex::new(None),
            started: Mutex::new(false),
            session_id: session_id.to_string(),
            bus: EventBus::new(),
            sink: Mutex::new(None),
            lease: Mutex::new(None),
            torn_truncate_to: Mutex::new(None),
        })
    }

    /// 严格回放一条事件（`ignorable` 兼容规则）：
    /// - **类型未知**且未标 `ignorable: true` → 拒绝重建（静默跳过会改变语义）；
    /// - 类型未知但标了 `ignorable: true` → 跳过（新版本写的事件，旧版本可安全忽略）；
    /// - 类型已知但字段形状不匹配（历史字段漂移）→ 跳过，保持旧日志可重建。
    fn replay_event(value: &serde_json::Value) -> io::Result<Option<SessionEvent>> {
        match serde_json::from_value::<SessionEvent>(value.clone()) {
            Ok(event) => Ok(Some(event)),
            Err(err) => {
                let ignorable = value
                    .get("ignorable")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                if err.to_string().contains("unknown variant") && !ignorable {
                    let kind = value.get("type").and_then(serde_json::Value::as_str).unwrap_or("<missing>");
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("会话日志含未知事件 `{kind}`（未标 ignorable:true），拒绝重建"),
                    ));
                }
                Ok(None)
            }
        }
    }

    /// 打开（或创建）指定 id 的会话目录与日志文件，并回放既有事件。
    fn attach(sessions_dir: &Path, session_id: &str) -> io::Result<Self> {
        let dir = crate::paths::session_dir(sessions_dir, session_id);
        std::fs::create_dir_all(&dir)?;
        // 跨进程写租约：同一会话目录的第二个写者在此被
        // 拒绝（内核锁，持有者死亡即自动释放，无陈旧锁）。
        let lease = crate::session_lock::SessionWriteLease::acquire(&dir).map_err(|err| {
            io::Error::other(format!("会话 `{session_id}` 无法获取写租约：{err}"))
        })?;
        let path = crate::paths::session_log_file(&dir);
        // 不用 append(true)：Windows 上 append 句柄只有 FILE_APPEND_DATA 权限，
        // 撕裂尾截断（set_len）会 ACCESS_DENIED（write(true) 并用也无法获得
        // FILE_WRITE_DATA）。改用 write(true) + 显式 seek(End) 追加——单写者
        // 语义由写租约保证，指针竞态不存在。
        let file = OpenOptions::new().write(true).create(true).truncate(false).open(&path)?;
        let log = Self {
            events: Mutex::new(Vec::new()),
            file: Mutex::new(Some(file)),
            started: Mutex::new(false),
            session_id: session_id.to_string(),
            bus: EventBus::new(),
            sink: Mutex::new(None),
            lease: Mutex::new(Some(lease)),
            torn_truncate_to: Mutex::new(None),
        };
        // 回放：按字节扫描既有 JSONL（`{"ts":...,"event":{...}}`），填充内存
        // 投影；崩溃残留的撕裂尾记入修复目标（首个追加动作截断），不直接消费。
        // 未知事件（未标 ignorable）→ 拒绝重建，避免「打开成功但语义丢失」。
        if let Ok(content) = std::fs::read(&path) {
            let committed_bytes = Self::replay_lines(&content, &mut lock(&log.events))?;
            if committed_bytes < content.len() as u64 {
                *lock(&log.torn_truncate_to) = Some(committed_bytes);
            }
        }
        Ok(log)
    }

    /// 逐完整行回放 durable 事件（非 strict recovery 语义）：
    /// - 未知事件（未标 ignorable）→ 拒绝重建；
    /// - JSON 损坏行 / 非 UTF-8 行跳过；
    /// - 末尾无换行的撕裂尾不消费；
    /// - 返回 `committed_bytes` = 最后一条「换行结尾且 JSON 可解析」行的末尾
    ///   偏移（`committed_bytes`），其后的字节留给首个追加动作截断。
    fn replay_lines(content: &[u8], events: &mut Vec<SessionEvent>) -> io::Result<u64> {
        let mut committed_bytes = 0u64;
        let mut offset = 0usize;
        for raw in content.split_inclusive(|byte| *byte == b'\n') {
            if !raw.ends_with(b"\n") {
                break; // 撕裂尾：最后一行未收尾（进程在写盘中途崩溃）。
            }
            let line_end = (offset + raw.len()) as u64;
            offset += raw.len();
            let Ok(text) = std::str::from_utf8(&raw[..raw.len() - 1]) else { continue };
            let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else { continue };
            if let Some(event) =
                Self::replay_event(&value.get("event").cloned().unwrap_or_default())?
            {
                events.push(event);
            }
            committed_bytes = line_end;
        }
        Ok(committed_bytes)
    }

    /// 内存态日志（测试用，不落盘）。
    pub fn in_memory() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
            file: Mutex::new(None),
            started: Mutex::new(false),
            session_id: format!("onion-mem-{}", epoch_millis()),
            bus: EventBus::new(),
            sink: Mutex::new(None),
            lease: Mutex::new(None),
            torn_truncate_to: Mutex::new(None),
        }
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn bus(&self) -> &EventBus {
        &self.bus
    }

    /// 首启发 SessionStarted（仅一次），返回是否本次触发。
    pub fn ensure_started(&self, model: &str) -> bool {
        let mut started = lock(&self.started);
        if *started {
            return false;
        }
        *started = true;
        drop(started);
        self.append(SessionEvent::SessionStarted {
            session_id: self.session_id.clone(),
            model: model.to_string(),
        });
        true
    }

    /// 注入 listener 扇出 sink（Runtime 装配期调用；未注入时事件仅进总线）。
    pub fn set_sink(&self, sink: EventSink) {
        *lock(&self.sink) = Some(sink);
    }

    fn fanout(&self, event: &LiveEvent) {
        // 克隆后调用：listener 可能回调 set_sink / snapshot，避免持锁重入。
        let sink = lock(&self.sink).clone();
        if let Some(sink) = sink.as_ref() {
            sink(event);
        }
    }

    /// 追加 durable 事件：撕裂尾修复（如有）+ 内存投影 + JSONL 行 + fsync +
    /// live 镜像 + listener 扇出。
    pub fn append(&self, event: SessionEvent) {
        if let Some(file) = lock(&self.file).as_mut() {
            // 撕裂尾修复（persistContiguous 语义）：首个追加动作先截断到
            // 最后一条完整行，再写新行——保证崩溃残留不与新追加交错。
            self.repair_torn_tail(file);
            // 显式 seek 到文件尾再写（见 attach 注释：不用 append 标志）。
            let _ = file.seek(SeekFrom::End(0));
            let line = json!({ "ts": epoch_millis(), "event": &event });
            let _ = writeln!(file, "{line}");
            let _ = file.flush();
            // fsync durable 事实（append + fsync）；
            // sync_data 跳过元数据，代价低于 sync_all。崩溃后由撕裂尾截断恢复。
            let _ = file.sync_data();
        }
        lock(&self.events).push(event.clone());
        let live = LiveEvent::Durable(event);
        self.bus.send(live.clone());
        self.fanout(&live);
    }

    /// 撕裂尾截断修复：截断到最后一条完整行，
    /// 失败保留状态待下次追加重试；残缺行留盘期间回放仍会跳过。
    fn repair_torn_tail(&self, file: &mut File) {
        let pending = lock(&self.torn_truncate_to).take();
        let Some(offset) = pending else { return };
        if file.set_len(offset).is_err() {
            tracing::warn!(offset, "会话日志撕裂尾截断失败，将于下次追加重试");
            *lock(&self.torn_truncate_to) = Some(offset);
        }
    }

    /// 非 durable live 事件（流帧等）：总线 + listener 扇出。
    pub fn emit_live(&self, event: LiveEvent) {
        self.bus.send(event.clone());
        self.fanout(&event);
    }

    /// durable 事件投影快照。
    pub fn snapshot(&self) -> Vec<SessionEvent> {
        lock(&self.events).clone()
    }

    /// 已落盘 durable 事件条数（= 下一条事件的日志下标）。
    ///
    /// 供命令面计算 `sourceEventSeq`：执行前后各取一次，差值区间即本命令
    /// 产生的领域事件。
    pub fn len(&self) -> usize {
        lock(&self.events).len()
    }

    /// 是否尚无任何 durable 事件。
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 关闭日志文件句柄并释放跨进程写租约（删除会话目录前调用）。
    ///
    /// Windows 上未关闭的句柄会让 `remove_dir_all` 报「目录不是空的」（写句柄
    /// 与锁句柄都须先释放）；关闭后 `append` 退化为仅内存投影。
    pub fn close(&self) {
        *lock(&self.file) = None;
        *lock(&self.lease) = None;
    }
}

/// 毫秒时间戳（手写，无 chrono）。
pub fn epoch_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 生成新的会话 id（`onion-<ms>-<seq>`）：毫秒 + 进程内自增序号，避免同一
/// 毫秒内快速连开两个会话时 id 撞车（写租约下会同目录冲突）。
pub fn fresh_session_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    format!("onion-{}-{}", epoch_millis(), SEQ.fetch_add(1, Ordering::Relaxed))
}

/// 会话摘要（`/resume` 列表项：标题 / 事件数 / goal 相位）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSummary {
    pub id: String,
    pub events: usize,
    pub title: String,
    pub goal_phase: String,
    /// 最近一次工作目录（`WorkingDir` / `WorktreeChange` 事件 last-write-wins）；
    /// 全程未变更过 cwd 的旧日志为空。会话历史按工作区分组的回退来源。
    pub working_dir: Option<String>,
    /// 日志文件最后修改时间（epoch ms）：会话历史按最近活跃排序的依据。
    pub updated_at_ms: u64,
}

/// 扫描会话目录列出历史会话（每会话一个 `<encoded-id>/session.vN.jsonl` 子目录，
/// 按日志文件修改时间倒序）。标题取首条真实用户输入的 user_message 文本（确定性
/// 回退；带 source 标记或 runtime-context/environment/
/// reminder 包裹的注入消息跳过，兼容旧日志的无标记注入）；goal 相位取最后一条
/// goal_change 的快照；解析失败的行跳过（损坏行局部失败，不隐藏整个候选）。
///
/// 由 onion-cli 与 onion-tui 共用，保证两个 front door 的 `/resume` 列表同源。
pub fn list_sessions(sessions_dir: &Path) -> Vec<SessionSummary> {
    let Ok(entries) = std::fs::read_dir(sessions_dir) else {
        return Vec::new();
    };
    let mut files: Vec<(std::time::SystemTime, std::path::PathBuf, String)> = entries
        .flatten()
        .filter_map(|entry| {
            // 会话以目录为单元，id = 目录名（已编码，无歧义）。
            let id = entry.file_name().into_string().ok()?;
            if !entry.path().is_dir() {
                return None;
            }
            let path = crate::paths::session_log_file(&entry.path());
            let mtime = std::fs::metadata(&path).ok()?.modified().ok()?;
            Some((mtime, path, id))
        })
        .collect();
    files.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    files
        .into_iter()
        .map(|(mtime, path, id)| {
            let mut events = 0usize;
            let mut title = String::new();
            let mut explicit_title: Option<String> = None;
            let mut goal_phase = "none".to_string();
            let mut working_dir: Option<String> = None;
            let updated_at_ms = mtime
                .duration_since(std::time::UNIX_EPOCH)
                .map(|since| since.as_millis() as u64)
                .unwrap_or_default();
            if let Ok(content) = std::fs::read_to_string(&path) {
                for line in content.lines() {
                    // 解包 `{"ts":...,"event":{...}}` 信封后类型化反序列化，
                    // 与 SessionLog::open_with_replay 回放路径同构。
                    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
                        continue;
                    };
                    let Ok(event) = serde_json::from_value::<SessionEvent>(
                        value.get("event").cloned().unwrap_or_default(),
                    ) else {
                        continue;
                    };
                    events += 1;
                    match event {
                        SessionEvent::UserMessage { message, .. } => {
                            // 系统注入（workspace/hooks/goal 续轮等带 source 标记）
                            // 与历史日志中无标记的 runtime-context/environment/
                            // system-reminder 包裹消息，均不作为标题。
                            let is_injection = message.blocks.iter().any(|block| {
                                matches!(block, crate::types::ContentBlock::Text { text }
                                    if text.starts_with("<system-reminder>")
                                        || text.starts_with("<environment")
                                        || text.starts_with("Current runtime context."))
                            });
                            if title.is_empty() && message.source.is_none() && !is_injection {
                                let text: String = message
                                    .blocks
                                    .iter()
                                    .filter_map(|block| match block {
                                        crate::types::ContentBlock::Text { text } => Some(text.as_str()),
                                        _ => None,
                                    })
                                    .collect();
                                let mut taken: String = text.chars().take(48).collect();
                                if text.chars().count() > 48 {
                                    taken.push('…');
                                }
                                title = taken;
                            }
                        }
                        SessionEvent::GoalChange { snapshot: Some(snapshot), .. } => {
                            goal_phase = snapshot.phase.label().to_string();
                        }
                        SessionEvent::WorkingDir { cwd } => working_dir = Some(cwd),
                        SessionEvent::WorktreeChange { path } => working_dir = Some(path),
                        SessionEvent::SessionTitle { title: named } => {
                            if !named.trim().is_empty() {
                                explicit_title = Some(named);
                            }
                        }
                        _ => {}
                    }
                }
            }
            // 显式标题（自动命名 / 手工重命名）优先于首条用户消息截断。
            let title = explicit_title.unwrap_or(title);
            SessionSummary { id, events, title, goal_phase, working_dir, updated_at_ms }
        })
        .collect()
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
