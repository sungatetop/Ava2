//! A2A v1.0 契约域类型（onion-server 内部实现，不依赖 ava-a2a）。
//!
//! 遵循 A2A（Agent2Agent）协议 v1.0 spec：
//! - `AgentCard`：能力发现（well-known URI）
//! - `Message` / `Task` / `Artifact`：消息 / 有状态工作单元 / 产出物
//!
//! 序列化遵循 camelCase（proto-json 风格）。

use serde::{Deserialize, Serialize};

/// A2A AgentCard：能力发现文档。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentCard {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<AgentProvider>,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation_url: Option<String>,
    pub capabilities: AgentCapabilities,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub default_input_modes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub default_output_modes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<AgentSkill>,
    /// onion 扩展：实例身份标识（env `ONION_INSTANCE_ID`，缺省 `default`）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
}

/// A2A AgentProvider：提供方信息。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentProvider {
    pub organization: String,
    pub url: String,
}

/// A2A AgentCapabilities：能力声明。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub streaming: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub push_notifications: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_transition_history: Option<bool>,
}

/// A2A AgentSkill：技能声明。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentSkill {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<String>,
}

impl AgentSkill {
    /// 构造一个技能（tags 必填：A2A spec 要求非空）。
    pub fn new(id: impl Into<String>, name: impl Into<String>, description: impl Into<String>, tags: Vec<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: Some(description.into()),
            tags,
            examples: Vec::new(),
        }
    }
}

/// A2A Part：消息内容块（text / file / data）。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Part {
    Text {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        metadata: Option<serde_json::Value>,
    },
    File {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        file: Option<A2aFile>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        metadata: Option<serde_json::Value>,
    },
    Data {
        data: serde_json::Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        metadata: Option<serde_json::Value>,
    },
}

/// A2A File：文件载荷（bytes 或 uri 二选一）。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct A2aFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
}

impl Part {
    /// 纯文本 Part。
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into(), metadata: None }
    }
}

/// A2A Role：消息发送方角色。
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Agent,
}

/// A2A Message：单轮交互单元（用户↔智能体）。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub role: Role,
    pub parts: Vec<Part>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<TaskId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_id: Option<ContextId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// A2A TaskId：任务标识。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct TaskId(pub String);

/// A2A ContextId：上下文标识（多轮会话）。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct ContextId(pub String);

impl TaskId {
    pub fn new(v: impl Into<String>) -> Self {
        Self(v.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ContextId {
    pub fn new(v: impl Into<String>) -> Self {
        Self(v.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A2A TaskState：任务状态机。
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TaskState {
    Submitted,
    Working,
    InputRequired,
    Completed,
    Canceled,
    Failed,
    Rejected,
    AuthRequired,
    Unknown,
}

impl TaskState {
    /// 终态（completed/canceled/failed/rejected）。
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Canceled | Self::Failed | Self::Rejected)
    }
}

/// A2A TaskStatus：任务状态 + 消息快照。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TaskStatus {
    pub state: TaskState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}

/// A2A Artifact：任务产出物（不可变）。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub artifact_id: String,
    pub name: Option<String>,
    pub parts: Vec<Part>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// A2A Task：有状态工作单元。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: TaskId,
    pub context_id: ContextId,
    pub status: TaskStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<Artifact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history: Option<Vec<Message>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// A2A MessageSendParams：message/send 参数。
///
/// 字段对齐 A2A 线格式；当前实现只消费 `message`，其余字段保留以完整承接客户端入参。
#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageSendParams {
    pub message: Message,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration: Option<MessageSendConfiguration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// A2A MessageSendConfiguration：send 配置（history 透传、push 配置）。
#[allow(dead_code)]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageSendConfiguration {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_output_modes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_length: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub push_notification_config: Option<PushNotificationConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocking: Option<bool>,
}

/// A2A PushNotificationConfig：webhook 推送配置。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PushNotificationConfig {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authentication: Option<serde_json::Value>,
}

/// A2A TaskQueryParams：tasks/get 参数。
///
/// `history_length` / `metadata` 属线格式字段，当前实现按全量快照返回。
#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskQueryParams {
    pub id: TaskId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_length: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// A2A TaskIdParams：tasks/cancel 等参数。
#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskIdParams {
    pub id: TaskId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// A2A TaskPushNotificationConfig：pushNotificationConfig/set 参数。
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskPushNotificationConfig {
    pub id: TaskId,
    pub push_notification_config: PushNotificationConfig,
}

/// A2A TaskStatusUpdateEvent：SSE 状态更新事件（message/stream result）。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TaskStatusUpdateEvent {
    pub task_id: TaskId,
    pub context_id: ContextId,
    pub status: TaskStatus,
    /// true = 终态事件（completed/canceled/failed/rejected）。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_final: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// A2A TaskArtifactUpdateEvent：SSE 产出物更新事件。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TaskArtifactUpdateEvent {
    pub task_id: TaskId,
    pub context_id: ContextId,
    pub artifact: Artifact,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub append: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_chunk: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}
