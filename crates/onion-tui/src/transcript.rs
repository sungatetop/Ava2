//! transcript.rs：append-origin 会话投影。
//!
//! 唯一事实源 = durable `SessionEvent`（`SessionLog::snapshot()`）。TUI 维护
//! 组件树投影：resume 时全量重放、运行中经 `LiveEvent::Durable` 增量更新，两条
//! 路径同一份确定性 fold。compaction 过的区间以单条 marker 呈现，不与模型可见
//! 上下文强同步；injection 消息折叠为一行注记（不展开正文）。

use onion::session::SessionEvent;
use onion::types::{AgentMessage, ContentBlock, Usage};

/// 组件树节点（Phase A 最小集：会话头/turn 分隔/消息/工具卡片/token 总计）。
#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    /// 会话头：session id + model。
    Header { session_id: String, model: String },
    /// turn 分隔线：序号 + 结束 reason。
    TurnSeparator { turn: u32, reason: Option<String> },
    /// 用户消息（injection 按折叠注记渲染）。
    UserMessage { message: AgentMessage },
    /// 系统注入折叠注记（一行 dim：类别 + 字符数，不展开正文）。
    InjectionNote { kind: String, chars: usize },
    /// assistant 块：文本 + 推理 + token 总计。
    AssistantBlock {
        text: String,
        thinking: String,
        usage: Option<Usage>,
    },
    /// 工具卡片：标题行 + head/tail 预览 + 状态着色。
    ToolCard {
        name: String,
        input_summary: String,
        output: Option<String>,
        is_error: bool,
    },
    /// assistant 尝试失败（cancelled 标记）。
    AssistantAttempt { error: String, cancelled: bool },
    /// 压缩事务卡：trigger + 摘要 + 节省 tokens。
    CompactionCard {
        trigger: String,
        summary: Option<String>,
        shadowed_count: Option<usize>,
        shadowed_tokens: Option<u64>,
        error: Option<String>,
    },
    /// goal 相位条：objective + phase。
    GoalBar {
        objective: String,
        phase: String,
        max_rounds: u32,
    },
    /// plan 徽标：active / wanted 状态。
    PlanBadge { active: bool, wanted: Option<bool> },
    /// hook 行（折叠）。
    HookLine {
        point: String,
        handler_id: String,
        decision: Option<String>,
    },
    /// workspace 事件行（折叠注记）。
    WorkspaceLine { summary: String },
    /// todo 整表快照（TodoWrite last-write-wins）。
    TodoList { todos: Vec<onion::todo::types::TodoItem> },
}

/// append-origin 投影：durable SessionEvent 流 → 组件树。
///
/// resume 全量重放与运行中增量更新共用同一 fold。
#[derive(Default)]
pub struct Transcript {
    nodes: Vec<Node>,
    turn: Option<u32>,
    /// 流式 pending assistant 块（live `StreamChunk` 累积，落定前先占位渲染，
    /// 使响应文本随 delta 逐字上屏；durable `AssistantMessage` 到达后替换）。
    streaming: Option<Streaming>,
}

/// 流式累积中的 assistant 块（`index` = 占位节点在 `nodes` 中的下标；
/// `turn`/`step` 用于 durable 落定时配对替换）。
#[derive(Clone, Copy, Debug)]
struct Streaming {
    turn: u32,
    step: u32,
    index: usize,
}

impl Transcript {
    pub fn new() -> Self {
        Self::default()
    }

    /// 开启一个流式块（`StreamChunk::Start`）：追加空 assistant 占位节点。
    pub fn begin_stream(&mut self, turn: u32, step: u32) {
        // 上一块未落定就被新块顶替（异常流）：丢弃累积状态，占位节点已流出的
        // 文本保留在树里，交由后续 durable 落定修正。
        self.streaming = None;
        self.nodes.push(Node::AssistantBlock {
            text: String::new(),
            thinking: String::new(),
            usage: None,
        });
        self.streaming = Some(Streaming { turn, step, index: self.nodes.len() - 1 });
    }

    /// 追加流式正文增量（`StreamChunk::Text`）。
    pub fn push_stream_text(&mut self, text: &str) {
        let Some(index) = self.streaming.map(|s| s.index) else { return };
        if let Some(Node::AssistantBlock { text: slot, .. }) = self.nodes.get_mut(index) {
            slot.push_str(text);
        }
    }

    /// 追加流式推理增量（`StreamChunk::Thinking`）。
    pub fn push_stream_thinking(&mut self, thinking: &str) {
        let Some(index) = self.streaming.map(|s| s.index) else { return };
        if let Some(Node::AssistantBlock { thinking: slot, .. }) = self.nodes.get_mut(index) {
            slot.push_str(thinking);
        }
    }

    /// 应用一条 durable 事件（增量路径）。
    pub fn apply(&mut self, event: &SessionEvent) {
        match event {
            SessionEvent::SessionStarted { session_id, model } => {
                self.nodes.push(Node::Header {
                    session_id: session_id.clone(),
                    model: model.clone(),
                });
            }
            SessionEvent::TurnStart { turn } => {
                self.turn = Some(*turn);
            }
            SessionEvent::TurnEnd { turn, reason } => {
                self.nodes.push(Node::TurnSeparator {
                    turn: *turn,
                    reason: Some(reason.clone()),
                });
            }
            SessionEvent::UserMessage { message, .. } => {
                // 注入折叠（§13「注入按 source 折叠」）：带 source 标记或文本
                // 启发式命中系统注入的消息只渲染一行注记，不展开正文（界面
                // 清爽；全文仍在 durable，session_query 可查）。
                if let Some(kind) = injection_kind(message) {
                    let chars: usize = message
                        .blocks
                        .iter()
                        .filter_map(|b| match b {
                            ContentBlock::Text { text } => Some(text.chars().count()),
                            _ => None,
                        })
                        .sum();
                    self.nodes.push(Node::InjectionNote { kind: kind.to_string(), chars });
                } else {
                    self.nodes.push(Node::UserMessage { message: message.clone() });
                }
            }
            SessionEvent::AssistantMessage { turn, step, message, usage } => {
                let text = message
                    .blocks
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<String>();
                let thinking = message
                    .blocks
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Thinking { thinking } => Some(thinking.as_str()),
                        _ => None,
                    })
                    .collect::<String>();
                let settled = Node::AssistantBlock { text, thinking, usage: Some(*usage) };
                // 流式占位节点原地替换（不追加）：durable 是权威文本，避免
                // 「流式块 + 落定块」重复渲染。
                match self.streaming.take() {
                    Some(s) if s.turn == *turn && s.step == *step => {
                        if let Some(slot) = self.nodes.get_mut(s.index) {
                            *slot = settled;
                        }
                    }
                    _ => self.nodes.push(settled),
                }
            }
            SessionEvent::AssistantAttempt { error, cancelled, .. } => {
                self.nodes.push(Node::AssistantAttempt { error: error.clone(), cancelled: *cancelled });
            }
            SessionEvent::ToolCall { call, .. } => {
                self.nodes.push(Node::ToolCard {
                    name: call.name.clone(),
                    input_summary: summarize_input(&call.input),
                    output: None,
                    is_error: false,
                });
            }
            SessionEvent::ToolResult { result, .. } => {
                // append-origin：ToolResult 落定最近一张同名 pending 卡（简化：直接
                // 追加 settled 卡；流式 pending 与 durable 复用组件的语义在 Phase A
                // 以「事件序自然对齐」呈现——ToolCall 先建 pending 卡，ToolResult
                // 落定该卡（head/tail + is_error）。
                self.nodes.push(Node::ToolCard {
                    name: result.name.clone(),
                    input_summary: String::new(),
                    output: Some(result.text_content()),
                    is_error: result.is_error,
                });
            }
            SessionEvent::CompactionStart { trigger, .. } => {
                self.nodes.push(Node::CompactionCard {
                    trigger: trigger.clone(),
                    summary: None,
                    shadowed_count: None,
                    shadowed_tokens: None,
                    error: None,
                });
            }
            SessionEvent::CompactionSummary { summary, shadowed_count, shadowed_tokens, .. } => {
                // marker 行：替代 Start 事务卡的 body（append-origin：历史区间仍可读）。
                if let Some(Node::CompactionCard {
                    summary: s,
                    shadowed_count: c,
                    shadowed_tokens: t,
                    ..
                }) = self.nodes.last_mut()
                {
                    *s = Some(summary.clone());
                    *c = Some(*shadowed_count);
                    *t = Some(*shadowed_tokens);
                }
            }
            SessionEvent::CompactionEnd { error, .. } => {
                if let Some(Node::CompactionCard { error: e, .. }) = self.nodes.last_mut() {
                    *e = error.clone();
                }
            }
            SessionEvent::GoalChange { snapshot, .. } => {
                if let Some(snapshot) = snapshot {
                    self.nodes.push(Node::GoalBar {
                        objective: snapshot.objective.clone(),
                        phase: snapshot.phase.label().to_string(),
                        max_rounds: snapshot.max_goal_rounds,
                    });
                }
            }
            SessionEvent::PlanMode { active } => {
                self.nodes.push(Node::PlanBadge { active: *active, wanted: None });
            }
            SessionEvent::PlanWanted { active } => {
                if let Some(Node::PlanBadge { wanted, .. }) = self.nodes.last_mut() {
                    *wanted = Some(*active);
                }
            }
            SessionEvent::PlanRunDone {} => {}
            SessionEvent::SystemMessage { .. } => {
                // system prompt 落盘事件（kernel 会话开始写入）：正文不渲染——
                // 对界面无信息量且动辄数千字（保持界面清爽）；durable 完整，
                // session_query 仍可查。
            }
            SessionEvent::HookInvoked { point, handler_id, .. } => {
                self.nodes.push(Node::HookLine {
                    point: point.clone(),
                    handler_id: handler_id.clone(),
                    decision: None,
                });
            }
            SessionEvent::HookResult { point, handler_id, decision, .. } => {
                self.nodes.push(Node::HookLine {
                    point: point.clone(),
                    handler_id: handler_id.clone(),
                    decision: Some(decision.clone()),
                });
            }
            SessionEvent::WorkspaceInstructions { files, bytes, .. } => {
                self.nodes.push(Node::WorkspaceLine {
                    summary: format!("workspace instructions: {} files, {} bytes", files.len(), bytes),
                });
            }
            SessionEvent::WorkspaceChanged { added, removed, changed } => {
                self.nodes.push(Node::WorkspaceLine {
                    summary: format!(
                        "workspace changed: +{} -{} ~{}",
                        added.len(),
                        removed.len(),
                        changed.len()
                    ),
                });
            }
            SessionEvent::SessionEnd { reason } => {
                self.nodes.push(Node::TurnSeparator {
                    turn: self.turn.take().unwrap_or_default(),
                    reason: Some(format!("session end: {reason}")),
                });
            }
            SessionEvent::StepStart { .. } | SessionEvent::StepEnd { .. } => {}
            SessionEvent::CompactionPruned { pruned, saved_tokens } => {
                self.nodes.push(Node::WorkspaceLine {
                    summary: format!("compaction pruned: {pruned} messages, {saved_tokens} tokens saved"),
                });
            }
            SessionEvent::ApprovalAudited { tool, decision, via } => {
                self.nodes.push(Node::WorkspaceLine {
                    summary: format!("approval {decision} ({via}): {tool}"),
                });
            }
            SessionEvent::TodoWrite { todos } => {
                // todo 整表快照（last-write-wins）：覆盖上一张（若有）。
                if let Some(Node::TodoList { todos: prev }) = self.nodes.last_mut() {
                    *prev = todos.clone();
                } else {
                    self.nodes.push(Node::TodoList { todos: todos.clone() });
                }
            }
            SessionEvent::Presented { files, .. } => {
                self.nodes.push(Node::WorkspaceLine {
                    summary: format!("presented {} files", files.len()),
                });
            }
            SessionEvent::WorkingDir { cwd } => {
                self.nodes.push(Node::WorkspaceLine {
                    summary: format!("cwd: {cwd}"),
                });
            }
            SessionEvent::WorktreeChange { path } => {
                self.nodes.push(Node::WorkspaceLine {
                    summary: format!("worktree → {path}"),
                });
            }
            SessionEvent::ModelSelection { model, base_url, effort } => {
                let effort = effort.as_deref().unwrap_or("-");
                self.nodes.push(Node::WorkspaceLine {
                    summary: format!("model → {model} ({base_url}) effort={effort}"),
                });
            }
            // agent-team 事件：log-only，TUI 不渲染（TeamService 独立投影持有）。
            SessionEvent::TeamMemberChange { .. } => {}
            SessionEvent::TeamTaskChange { .. } => {}
            // 自动标题：仅落盘供会话列表使用，转录区不渲染。
            SessionEvent::SessionTitle { .. } => {}
            // 斜杠命令：TUI 由命令面板直接回执，转录区不重复渲染。
            SessionEvent::CommandRun { .. } => {}
            SessionEvent::CommandDone { .. } => {}
            // 子代理描述符：log-only 元信息，转录区不渲染。
            SessionEvent::SubagentDescriptor { .. } => {}
            // 子代理模型选择策略：log-only 元信息，转录区不渲染。
            SessionEvent::SubagentModelSelectionPolicy { .. } => {}
            // 用户反馈：log-only 产品信号，转录区不渲染。
            SessionEvent::FeedbackRecord { .. } => {}
            // 审批预设变更：log-only 配置事实（子代理播种标记亦然），不渲染。
            SessionEvent::ApprovalPreset { .. } => {}
            // fork 播种标记：log-only 血缘元信息（前缀已作为普通消息重放），不渲染。
            SessionEvent::ForkSeed { .. } => {}
        }
    }

    /// 全量重放（resume 路径）：清空后按序 fold。
    pub fn replay(&mut self, events: &[SessionEvent]) {
        self.nodes.clear();
        self.turn = None;
        self.streaming = None;
        for event in events {
            self.apply(event);
        }
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// 视图级清空（`Ctrl+U`）：只清投影，durable 日志不动——下一次 resume
    /// 重放仍能恢复完整历史。
    pub fn clear_view(&mut self) {
        self.nodes.clear();
        self.turn = None;
        self.streaming = None;
    }
}

/// 工具调用输入摘要（如 read_file(path) / bash(cmd…)）。
fn summarize_input(input: &serde_json::Value) -> String {
    // 关键参数摘要：取 JSON 顶层键值对的紧凑展示。
    match input {
        serde_json::Value::Object(map) => {
            let mut parts = Vec::new();
            for (k, v) in map {
                let vs = match v {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                parts.push(format!("{k}={vs}"));
            }
            parts.join(", ")
        }
        other => other.to_string(),
    }
}

/// 系统注入判定（注入按 source 折叠）：带 source 标记一律折叠；
/// 无标记的历史消息按文本启发式（对齐 `session.rs` 标题判定三条前缀）折叠。
/// 返回注记用的类别标签。
fn injection_kind(message: &AgentMessage) -> Option<&'static str> {
    use onion::types::MessageSource;
    if let Some(source) = &message.source {
        return Some(match source {
            MessageSource::WorkspaceBaseline { .. } => "workspace-baseline",
            MessageSource::WorkspaceDelta { .. } => "workspace-delta",
            MessageSource::HooksContext => "hooks-context",
            MessageSource::RuntimeContext => "runtime-context",
            MessageSource::SubagentMessage { .. } => "subagent",
            MessageSource::SubagentSettled { .. } => "subagent-settled",
            MessageSource::GoalRound { .. } => "goal-round",
            MessageSource::Schedule => "schedule",
            MessageSource::SkillCatalog { .. } => "skill-catalog",
            MessageSource::SkillInvocation { .. } => "skill-invocation",
            MessageSource::ModelSelection => "model-selection",
            MessageSource::AnswerToPending { .. } => "answer-to-pending",
            MessageSource::A2aPeer { .. } => "a2a-peer",
            MessageSource::CompactCheckpoint { .. } => "compact-checkpoint",
        });
    }
    // 历史日志兼容：无 source 标记但文本启发式命中（与 session.rs 标题判定
    // 同一套前缀；首个文本块开头即整条注入的开头）。
    let head = message.blocks.iter().find_map(|b| match b {
        ContentBlock::Text { text } => Some(text.as_str()),
        _ => None,
    });
    match head {
        Some(text) if text.starts_with("<system-reminder>") => Some("system-reminder"),
        Some(text) if text.starts_with("<environment") => Some("environment"),
        Some(text) if text.starts_with("Current runtime context.") => Some("runtime-context"),
        _ => None,
    }
}

#[cfg(test)]
#[path = "transcript_tests.rs"]
mod tests;
