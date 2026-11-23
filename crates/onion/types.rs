//! 词汇表：角色 / 消息 / 内容块 / 工具调用与结果 / 用量 / 停止原因 / 流增量。

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 消息角色（system prompt 作为请求级配置，不进 Role）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    Tool,
}

/// 消息内容块（ToolResult 可嵌套文本/图片块）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text { text: String },
    Thinking { thinking: String },
    Image { image_url: String },
    ToolUse { id: String, name: String, input: Value },
    ToolResult { tool_call_id: String, is_error: bool, content: Vec<ContentBlock> },
}

/// 一条角色消息。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgentMessage {
    pub role: Role,
    pub blocks: Vec<ContentBlock>,
    /// 来源标记（`source.kind` 教学子集；None = 普通 user/assistant 消息）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<MessageSource>,
}

/// 消息来源标记（`source.kind` 教学子集）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MessageSource {
    /// 工作空间指令基线（agent-instructions baseline）。
    WorkspaceBaseline { files: Vec<String> },
    /// 工作空间指令变更（agent-instructions changes）。
    WorkspaceDelta { files: Vec<String> },
    /// 钩子注入的上下文（hooks-claude-code 桥）。
    HooksContext,
    /// 运行时快照（runtime-context）。
    RuntimeContext,
    /// subagent relay：subagent 最终 assistant 消息投递进父会话 inbox。
    SubagentMessage { agent_id: String },
    /// subagent settle 通知：运行时对「子代理如何结束」的一行账目（notice 形态）。
    /// 与 relay 严格区分：relay 是子代理
    /// **自己写的**内容，notice 是运行时**关于**子代理的陈述。
    SubagentSettled {
        agent_id: String,
        /// 结束原因（completed / aborted / max-tokens / refusal / error）。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stop_reason: Option<String>,
        /// 送给父代理的一行摘要（`summary`；与 blocks 首段文本同源）。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        summary: Option<String>,
    },
    /// goal 续轮：自动续跑轮。
    /// round 归属 + 权威判定的锚点：id/revision 匹配 + round 恰好为下一轮。
    GoalRound { goal_id: String, revision: u32, round: u32 },
    /// schedule 到期递送（`source: { kind: 'schedule' }`）。
    Schedule,
    /// 会话技能目录（`source: { kind: 'skill-catalog' }`）：`<available_skills>`
    /// 目录注入。`update` 标记替换目录而非本会话首次发布；`entries` 精确记录
    /// 本次发布的条目（catalog 序），供非模型消费者读取而不必重解析 prose。
    SkillCatalog {
        /// 替换目录（非本会话首次发布）。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        update: Option<bool>,
        /// 本次发布的条目（catalog 序）。
        entries: Vec<SkillCatalogEntry>,
    },
    /// 用户显式 `/name` 手势注入（`source: { kind: 'skill-invocation' }`）：
    /// 注入的技能 body 是给模型的 instructions。
    SkillInvocation { name: String },
    /// 模型切换提示（`source: { kind: 'model-selection', form: 'notice' }`）：
    /// pre-step 注入，声明上方 assistant 轮次由旧模型生成、会话以新模型继续。
    ModelSelection,
    /// 迟到回答（`answer_to_pending_question`）：timed ask_user 前台等待
    /// 超时返回 pending 后，用户晚到的答复作为 user 消息注入，携带原 tool call id。
    AnswerToPending { call_id: String },
    /// A2A 跨实例消息（`message/send` 入站）：记录发送方身份，使外部实例消息
    /// 与本机用户消息在会话与前端可区分。`name` 为发送方自报的显示名
    /// （`ONION_INSTANCE_NAME`，缺省同 `instance_id`）。
    A2aPeer { instance_id: String, name: String },
    /// 压缩检查点（`source: { kind: 'compact-checkpoint' }`）：压缩事务
    /// 提交后紧随的 user 消息，正文即 `<compacted-summary>` 承载帧。它就是上下文
    /// 替换的真实载体（投影时取代被 shadowed 的区间），同时让非模型消费者
    /// （TUI / session_query）无需解析正文即可识别检查点，不与真实用户消息混淆。
    CompactCheckpoint {
        /// 所属压缩事务 id（与 `compaction_summary` 事件配对）。
        compaction_id: String,
    },
}

/// 技能目录条目（`SkillCatalogSource['entries']` 的元素）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillCatalogEntry {
    pub name: String,
    pub description: String,
}

impl AgentMessage {
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            blocks: vec![ContentBlock::Text { text: text.into() }],
            source: None,
        }
    }

    pub fn assistant(blocks: Vec<ContentBlock>) -> Self {
        Self { role: Role::Assistant, blocks, source: None }
    }

    pub fn tool_result(result: &ToolResult) -> Self {
        Self {
            role: Role::Tool,
            blocks: vec![ContentBlock::ToolResult {
                tool_call_id: result.tool_call_id.clone(),
                is_error: result.is_error,
                content: result.content.clone(),
            }],
            source: None,
        }
    }

    /// 全部文本块拼接（观测打印用）。
    pub fn text_content(&self) -> String {
        self.blocks
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    /// 本消息携带的工具调用（按块序）。
    pub fn tool_calls(&self) -> Vec<ToolCall> {
        self.blocks
            .iter()
            .filter_map(|block| match block {
                ContentBlock::ToolUse { id, name, input } => Some(ToolCall {
                    id: id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                }),
                _ => None,
            })
            .collect()
    }
}

/// 一次工具调用（assistant 的 ToolUse 块）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: Value,
}

/// 一次工具执行结果。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub name: String,
    pub is_error: bool,
    pub content: Vec<ContentBlock>,
}

impl ToolResult {
    pub fn text(call: &ToolCall, text: impl Into<String>) -> Self {
        Self {
            tool_call_id: call.id.clone(),
            name: call.name.clone(),
            is_error: false,
            content: vec![ContentBlock::Text { text: text.into() }],
        }
    }

    pub fn error(call: &ToolCall, text: impl Into<String>) -> Self {
        Self {
            tool_call_id: call.id.clone(),
            name: call.name.clone(),
            is_error: true,
            content: vec![ContentBlock::Text { text: text.into() }],
        }
    }

    /// 全部文本块拼接（钩子 payload / 观测用）。
    pub fn text_content(&self) -> String {
        self.content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }
}

/// 工具定义（注册表下发给模型的 schema）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// 模型用量（流末 usage 块）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
}

/// 落定停止原因。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    Cancelled,
}

impl StopReason {
    /// 供应商 finish_reason 字符串归一化。
    pub fn from_provider(reason: &str) -> Self {
        match reason {
            "stop" => Self::EndTurn,
            "tool_calls" | "tool_use" => Self::ToolUse,
            "length" => Self::MaxTokens,
            _ => Self::Cancelled,
        }
    }
}

/// LLM 流式增量。
#[derive(Clone, Debug, PartialEq)]
pub enum StreamDelta {
    Text { text: String },
    Thinking { thinking: String },
    ToolCallStart { index: usize, id: String, name: String },
    ToolCallInput { index: usize, partial_json: String },
}
