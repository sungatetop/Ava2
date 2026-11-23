//! REST 请求/响应 DTO（camelCase；与 `crate::a2a` 的 A2A 域类型相互独立）。
//!
//! 唯一跨契约共享的是「消息内容 → `AgentMessage`」转换（见
//! [`parts_to_agent_message`]），语义与 A2A `parts` 一致（text / file / data）。

use serde::{Deserialize, Serialize};

use base64::Engine as _;

use crate::a2a::TaskState;
use crate::session::TurnSummary;
use crate::rest::error::ApiError;

use onion::ask_user::types::AskUserQuestion;
use onion::goal::types::{GoalActivation, GoalView};

// ---------------- 请求 DTO ----------------

/// `POST /v1/sessions`。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateSessionRequest {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub interactive: Option<bool>,
    /// 会话工作目录（绝对路径）；缺省 = 兜底工作目录。
    #[serde(default)]
    pub cwd: Option<String>,
}

/// 消息内容块（`kind` 标签：`text` / `file` / `data`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum ContentPart {
    Text { text: String },
    File { file: FilePart },
    Data { data: serde_json::Value },
}

/// 文件内容块载荷（`uri` 为主，`bytes` 留作 base64 扩展位）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FilePart {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<String>,
}

/// `POST /v1/sessions/{id}/messages`。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PostMessageRequest {
    pub parts: Vec<ContentPart>,
    #[serde(default)]
    pub wait: Option<bool>,
    #[serde(default)]
    pub task_id: Option<String>,
}

/// `POST /v1/sessions/{id}/approvals/{requestId}`。
#[derive(Debug, Deserialize)]
pub(crate) struct ApprovalReply {
    pub decision: String,
}

/// `POST /v1/sessions/{id}/plan-reviews/{requestId}`。
#[derive(Debug, Deserialize)]
pub(crate) struct PlanReviewReply {
    pub verdict: String,
}

/// `POST /v1/sessions/{id}/questions/{requestId}`。
#[derive(Debug, Deserialize)]
pub(crate) struct QuestionReply {
    pub answers: Vec<AnswerInput>,
}

/// 单题答复。
#[derive(Debug, Deserialize)]
pub(crate) struct AnswerInput {
    pub id: String,
    #[serde(default)]
    pub selected: Vec<String>,
    #[serde(default)]
    pub custom: Option<String>,
}

/// `PUT /v1/sessions/{id}/agent`。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetAgentRequest {
    pub agent_id: String,
}

/// `PUT /v1/sessions/{id}/model`。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetModelRequest {
    /// provider-owned 模型 id（须在供应商目录内）。
    pub model: String,
}

/// `PUT /v1/sessions/{id}/approval`。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetApprovalRequest {
    /// 审批预设名（`workspace-write` / `auto` / `danger-full-access`）。
    pub preset: String,
}

/// `PUT /v1/sessions/{id}/goal`：带 `revision` 为编辑，否则创建。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetGoalRequest {
    #[serde(default)]
    pub objective: Option<String>,
    #[serde(default)]
    pub max_goal_rounds: Option<u32>,
    #[serde(default)]
    pub revision: Option<u32>,
}

/// `POST /v1/sessions/{id}/goal/actions`。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GoalActionRequest {
    pub action: String,
    pub revision: u32,
    #[serde(default)]
    pub reason: Option<String>,
}

/// `POST /v1/sessions/{id}/plan`。
#[derive(Debug, Deserialize)]
pub(crate) struct SetPlanRequest {
    pub active: bool,
}

/// `POST /v1/sessions/{id}/commands`：执行一条斜杠命令。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunCommandRequest {
    /// 命令名（不含前导 `/`）。
    pub name: String,
    /// 参数原文（可为空）。
    #[serde(default)]
    pub args: String,
}

/// 命令执行结果（`kind` = `success` / `error`）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommandResultInfo {
    pub command_id: String,
    pub kind: String,
    pub text: String,
    /// 本命令产生的首条领域事件在 durable 日志中的下标；无领域事件时省略。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_event_seq: Option<u64>,
}

// ---------------- 消息内容转换 ----------------

/// REST 内容块 → 引擎 `AgentMessage`（空 `parts` 为非法请求）。
///
/// `FilePart.bytes`（base64 扩展位）非空时走 [`onion::attachment`] 准入：
/// image/* 合并为一批（张数/聚合字节限额 + prepare-then-commit 原子性，
/// 一批一张非法则整批拒绝、零落盘），准入后读回存储字节回填为 data URL
/// `Image` 块；其余类型按文件原样入库，以文本块描述引用（文件附件进
/// 上下文的降级形态）。`bytes` 缺省维持原行为（`uri` 透传为
/// `Image` 块）。
pub(crate) fn parts_to_agent_message(
    parts: &[ContentPart],
    attachments: &onion::attachment::AttachmentStore,
) -> Result<onion::types::AgentMessage, ApiError> {
    use onion::attachment::{
        admit_encoded_file, admit_encoded_images, EncodedFileAttachment, EncodedImageAttachment,
        ImageAttachmentLimits,
    };

    if parts.is_empty() {
        return Err(ApiError::bad_request("invalid_parts", "parts must not be empty"));
    }

    let mut blocks: Vec<onion::types::ContentBlock> = Vec::with_capacity(parts.len());
    let mut image_batch: Vec<EncodedImageAttachment> = Vec::new();
    // image_batch[i] 对应 blocks 中的占位槽位（准入通过后回填，保持块序）。
    let mut image_slots: Vec<usize> = Vec::new();

    for part in parts {
        match part {
            ContentPart::Text { text } => {
                blocks.push(onion::types::ContentBlock::Text { text: text.clone() });
            }
            ContentPart::Data { data } => {
                blocks.push(onion::types::ContentBlock::Text {
                    text: serde_json::to_string(data).unwrap_or_default(),
                });
            }
            ContentPart::File { file } => {
                let Some(bytes) = file.bytes.as_deref().filter(|b| !b.is_empty()) else {
                    blocks.push(onion::types::ContentBlock::Image {
                        image_url: file.uri.clone().unwrap_or_default(),
                    });
                    continue;
                };
                let media_type = file
                    .mime_type
                    .as_deref()
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                if media_type.starts_with("image/") {
                    image_slots.push(blocks.len());
                    blocks.push(onion::types::ContentBlock::Text { text: String::new() });
                    image_batch.push(EncodedImageAttachment {
                        media_type,
                        data: bytes.to_string(),
                        name: file.name.clone(),
                    });
                } else {
                    let stored = admit_encoded_file(
                        attachments,
                        &EncodedFileAttachment { data: bytes.to_string(), name: file.name.clone() },
                    )
                    .map_err(crate::rest::attachments::map_attachment_error)?;
                    blocks.push(onion::types::ContentBlock::Text {
                        text: format!(
                            "[file attachment stored: name={} id={} bytes={}]",
                            stored.name, stored.attachment_id, stored.bytes
                        ),
                    });
                }
            }
        }
    }

    let refs = if image_batch.is_empty() {
        Vec::new()
    } else {
        admit_encoded_images(attachments, &image_batch, &ImageAttachmentLimits::default())
            .map_err(crate::rest::attachments::map_attachment_error)?
    };
    for (slot, image) in image_slots.iter().zip(&refs) {
        // 读回复核（摘要 + 媒体 + 尺寸）后用存储的 canonical 字节构造 data URL。
        let data = attachments.read_image(image).map_err(crate::rest::attachments::map_attachment_error)?;
        blocks[*slot] = onion::types::ContentBlock::Image {
            image_url: format!(
                "data:{};base64,{}",
                image.media_type,
                base64::engine::general_purpose::STANDARD.encode(data)
            ),
        };
    }

    Ok(onion::types::AgentMessage {
        role: onion::types::Role::User,
        blocks,
        source: None,
    })
}

// ---------------- 响应 DTO ----------------

/// `GET /v1/health`。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HealthInfo {
    pub status: &'static str,
    pub version: &'static str,
    pub uptime_ms: u64,
    pub model: String,
}

/// `GET /v1/meta`。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MetaInfo {
    pub protocol: &'static str,
    pub version: &'static str,
    pub workspace: String,
    pub sessions_dir: String,
    pub interactive_default: bool,
    pub features: Vec<&'static str>,
    /// 实例身份标识（env `ONION_INSTANCE_ID`，缺省 `default`）。
    pub instance_id: String,
}

/// 对端实例条目（`ONION_A2A_PEERS` 静态实例表）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PeerInfo {
    pub name: String,
    pub url: String,
}

/// `GET /v1/peers`。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PeersResponse {
    pub peers: Vec<PeerInfo>,
}

/// `GET /v1/config`（脱敏：不含 `apiKey`）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConfigInfo {
    pub model: String,
    pub base_url: String,
    pub max_tokens: u32,
    pub approval_preset: String,
    pub agent_preset: String,
}

/// `GET /v1/models`。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelsInfo {
    pub default_model: String,
    pub providers: Vec<ProviderInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub models: Vec<ModelInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelInfo {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// 轮次摘要。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TurnInfo {
    pub turn: u32,
    pub steps: u32,
    pub reason: String,
    pub usage: TurnUsageInfo,
}

/// 轮次 token 用量（camelCase 投影）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TurnUsageInfo {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
}

impl TurnUsageInfo {
    pub(crate) fn from_usage(usage: onion::types::Usage) -> Self {
        Self {
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            cache_read_tokens: usage.cache_read_tokens,
            cache_write_tokens: usage.cache_write_tokens,
        }
    }
}

impl TurnInfo {
    pub(crate) fn from_summary(summary: TurnSummary) -> Self {
        Self {
            turn: summary.turn,
            steps: summary.steps,
            reason: summary.reason,
            usage: TurnUsageInfo::from_usage(summary.usage),
        }
    }
}

/// plan 模式状态。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlanInfo {
    pub active: bool,
    pub pending: Option<bool>,
}

/// goal 视图（camelCase 投影）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GoalInfo {
    pub id: String,
    pub revision: u32,
    pub objective: String,
    pub phase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<BlockedReasonInfo>,
    pub max_goal_rounds: u32,
    pub rounds_started: u32,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub armed: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BlockedReasonInfo {
    pub code: String,
    pub message: String,
}

impl GoalInfo {
    pub(crate) fn from_view(view: &GoalView) -> Self {
        Self {
            id: view.snapshot.id.clone(),
            revision: view.snapshot.revision,
            objective: view.snapshot.objective.clone(),
            phase: view.snapshot.phase.label().to_string(),
            blocked_reason: view.snapshot.blocked_reason.as_ref().map(|reason| {
                BlockedReasonInfo { code: reason.code.clone(), message: reason.message.clone() }
            }),
            max_goal_rounds: view.snapshot.max_goal_rounds,
            rounds_started: view.rounds_started,
            created_at_ms: view.created_at_ms,
            updated_at_ms: view.updated_at_ms,
            armed: matches!(view.activation, GoalActivation::Armed),
        }
    }
}

/// 会话详情。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionInfo {
    pub id: String,
    pub agent: String,
    pub interactive: bool,
    pub busy: bool,
    pub working_dir: String,
    /// 会话权限根（装配期固定）。
    pub root: String,
    /// 下一请求模型目标（运行期 `/model` 切换经共享 ModelTargetRef 生效）。
    pub model: String,
    /// 当前审批预设（`/approval` 切换写入口）。
    pub approval_preset: String,
    pub created_at_ms: u64,
    pub event_count: usize,
    /// 当前上下文估算 tokens（durable 事件 chars/4，与 TUI 状态栏同口径）。
    pub context_tokens: u64,
    /// 上下文预算（Config::context_window_tokens）。
    pub context_window_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_turn: Option<TurnInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goal: Option<GoalInfo>,
    pub plan: PlanInfo,
}

/// 会话列表项（进程内 live 与磁盘会话合并）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionListItem {
    pub id: String,
    pub live: bool,
    pub event_count: usize,
    pub title: String,
    pub goal_phase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at_ms: Option<u64>,
    /// 所属工作区绝对路径（会话历史按工作区分组的依据；旧日志无法判定时缺省）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,
    /// 最近活跃时间（epoch ms）：磁盘会话取日志 mtime，live 会话取装配时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at_ms: Option<u64>,
}

/// `GET /v1/sessions`。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionListResponse {
    pub sessions: Vec<SessionListItem>,
}

/// `POST /v1/sessions/{id}/messages` 回执。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MessageAck {
    pub session_id: String,
    pub task_id: String,
    pub completed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_turn: Option<TurnInfo>,
}

/// 待回填输入项。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PendingItem {
    pub request_id: String,
    pub kind: String,
    pub created_at_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
    /// 审批来源标记（`sandbox-retry` = 沙箱拒绝后的越界执行申请；缺省 = 首次审批）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub questions: Option<Vec<QuestionInfo>>,
}

/// 问询问题（camelCase 投影）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuestionInfo {
    pub id: String,
    pub question: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<String>,
    pub options: Vec<QuestionOptionInfo>,
    pub multi_select: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuestionOptionInfo {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl QuestionInfo {
    pub(crate) fn from_question(question: &AskUserQuestion) -> Self {
        Self {
            id: question.id.clone(),
            question: question.question.clone(),
            header: question.header.clone(),
            options: question
                .options
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(|option| QuestionOptionInfo {
                    label: option.label.clone(),
                    description: option.description.clone(),
                })
                .collect(),
            multi_select: question.multi_select.unwrap_or(false),
        }
    }
}

/// `GET /v1/sessions/{id}/pending`。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PendingListResponse {
    pub pending: Vec<PendingItem>,
}

/// `POST /v1/sessions/{id}/interrupt`。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CancelAck {
    pub cancelled: bool,
}

/// `POST /v1/sessions/{id}/compact`。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CompactAck {
    pub queued: bool,
}

/// `POST /v1/sessions/{id}/fork` 请求。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ForkRequest {
    /// 源会话继承边界 seq（含，复制 `[1..=boundary]` 前缀）；缺省取源日志最后一条。
    pub boundary_seq: Option<u64>,
    /// 子会话 id；缺省自动生成（`onion-<epoch_ms>`）。
    pub child_session_id: Option<String>,
}

/// `POST /v1/sessions/{id}/fork` 响应。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ForkResponse {
    /// 子会话 id（恢复以它为准，前缀中的源 `SessionStarted` 保留为历史事实）。
    pub child_session_id: String,
    /// 继承前缀条数（不含播种标记与合成关闭事件）。
    pub inherited_events: u64,
    /// 子会话日志事件总条数。
    pub events: usize,
}

/// `GET /v1/sessions/{id}/todos`。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TodoResponse {
    pub todos: Vec<onion::todo::types::TodoItem>,
}

/// `GET /v1/sessions/{id}/deliverables`：`present` 声明的交付物投影。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeliverablesResponse {
    pub deliverables: Vec<DeliverableInfo>,
}

/// 单条交付物（同一 path 只保留最后一次声明）。
#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeliverableInfo {
    /// 归属回合（`present` 调用时已记录的回合数；0 = 回合外调用）
    pub turn: u64,
    /// 触发声明的 tool call id
    pub call_id: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// 回填回执。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReplyAck {
    pub request_id: String,
    pub decision: String,
}

/// `GET|PUT /v1/sessions/{id}/agent`。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentView {
    pub active: String,
    pub agents: Vec<AgentInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentInfo {
    pub id: String,
    pub name: String,
    pub tools: Vec<String>,
}

/// `GET /v1/tools`。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolListResponse {
    pub tools: Vec<ToolInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolInfo {
    pub name: String,
    pub description: String,
    pub risk: &'static str,
    pub in_agent: bool,
}

/// `GET /v1/skills`。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillListResponse {
    pub skills: Vec<SkillInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillInfo {
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when_to_use: Option<String>,
    pub source: String,
    pub provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub model_invocable: bool,
    pub user_invocable: bool,
    /// skills-manager 视角：未被禁用为 true。列表始终含禁用技能（管理界面
    /// 需要看到全部），禁用仅影响此标记与运行期注入。
    pub enabled: bool,
}

impl SkillInfo {
    pub(crate) fn from_summary(summary: &onion::skill::SkillSummary, enabled: bool) -> Self {
        Self {
            name: summary.name.clone(),
            description: summary.description.clone(),
            when_to_use: summary.when_to_use.clone(),
            source: summary.source.as_str().to_string(),
            provider: summary.provider.clone(),
            path: summary.path.as_ref().map(|path| path.display().to_string()),
            model_invocable: summary.invocation.model_invocable,
            user_invocable: summary.invocation.user_invocable,
            enabled,
        }
    }
}

/// `GET /v1/mcp/servers`。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct McpListResponse {
    pub servers: Vec<McpServerInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct McpServerInfo {
    pub name: String,
    pub transport: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// server 最近一次工具发现状态。
    pub state: McpServerStateInfo,
    /// 已桥接注册的公开工具名（`mcp__<server>__<tool>`）。
    pub tools: Vec<String>,
}

/// server 最近一次工具发现状态（[`onion::mcp_resources::McpServerState`] 投影）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct McpServerStateInfo {
    /// `registered` / `discovery_failed` / `pending`。
    pub kind: String,
    /// registered：已桥接注册的工具数。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_count: Option<usize>,
    /// discovery_failed：最近一次发现失败原因。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl McpServerStateInfo {
    pub(crate) fn from_state(state: &onion::mcp_resources::McpServerState) -> Self {
        match state {
            onion::mcp_resources::McpServerState::Registered { tool_count } => Self {
                kind: "registered".to_string(),
                tool_count: Some(*tool_count),
                error: None,
            },
            onion::mcp_resources::McpServerState::DiscoveryFailed { error } => Self {
                kind: "discovery_failed".to_string(),
                tool_count: None,
                error: Some(error.clone()),
            },
            onion::mcp_resources::McpServerState::Pending => Self {
                kind: "pending".to_string(),
                tool_count: None,
                error: None,
            },
        }
    }
}

/// `POST /v1/mcp/servers/{name}/probe`：独立一次性连接探活结果（纯诊断，
/// 不改运行期状态）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct McpProbeResponse {
    pub ok: bool,
    /// 失败阶段（`connect` / `list`）；成功时省略。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<&'static str>,
    /// 失败原因；成功时省略。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub duration_ms: u64,
    /// 成功时的远端 raw 工具名（非公开名），超预算截断。
    pub tool_names: Vec<String>,
}

/// `POST /v1/mcp/reconcile`：配置收敛结果（挂载/卸载语义）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct McpReconcileResponse {
    /// 新增挂载的 server。
    pub added: Vec<String>,
    /// 配置变更后重建的 server。
    pub rebuilt: Vec<String>,
    /// 配置消失后卸载的 server。
    pub removed: Vec<String>,
    /// 配置未变、保持原状的 server。
    pub unchanged: Vec<String>,
    /// 发现与挂载过程中的说明性信息。
    pub notes: Vec<String>,
}

/// `POST /v1/mcp/servers/{name}/resync`：两阶段原子换装结果。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct McpResyncResponse {
    pub server: String,
    /// 本次注销的公开工具名。
    pub removed: Vec<String>,
    /// 本次注册的公开工具名。
    pub added: Vec<String>,
}

/// `GET /v1/schedules`。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScheduleListResponse {
    pub schedules: Vec<onion::schedule::types::ScheduleView>,
}

/// `GET /v1/memory`。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryListResponse {
    pub memories: Vec<MemoryInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryInfo {
    pub filename: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_type: Option<String>,
    pub mtime_ms: u64,
}

/// `GET /v1/memory/{file}`。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryFileInfo {
    pub filename: String,
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_type: Option<String>,
    pub body: String,
}

/// 任务快照。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TaskInfo {
    pub id: String,
    pub context_id: String,
    pub state: &'static str,
    pub history_count: usize,
    pub artifact_count: usize,
    /// 委派到对端实例的任务摘要（本地任务无此字段）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote: Option<RemoteTaskInfo>,
}

/// 委派任务摘要（来自 `<session>/tasks/links.jsonl`）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteTaskInfo {
    pub peer_url: String,
    pub message_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report_to: Option<serde_json::Value>,
}

impl TaskInfo {
    pub(crate) fn from_record(record: &crate::session::TaskRecord) -> Self {
        Self {
            id: record.id.as_str().to_string(),
            context_id: record.context_id.as_str().to_string(),
            state: task_state_label(record.state),
            history_count: record.history.len(),
            artifact_count: record.artifacts.len(),
            remote: None,
        }
    }

    pub(crate) fn from_link(link: &onion::tools::DelegationLink) -> Self {
        Self {
            id: link.remote_task_id.clone(),
            context_id: link.remote_context_id.clone(),
            state: link_state_label(&link.remote_state),
            history_count: 0,
            artifact_count: 0,
            remote: Some(RemoteTaskInfo {
                peer_url: link.peer_url.clone(),
                message_id: link.message_id.clone(),
                report_to: link.report_to.clone(),
            }),
        }
    }
}

/// 委派链接状态标签（A2A kebab-case）→ 稳定 REST 标签。
fn link_state_label(label: &str) -> &'static str {
    match label {
        "submitted" => "submitted",
        "working" => "working",
        "input-required" => "input_required",
        "completed" => "completed",
        "canceled" => "canceled",
        "failed" => "failed",
        "rejected" => "rejected",
        "auth-required" => "auth_required",
        _ => "unknown",
    }
}

/// `GET /v1/sessions/{id}/tasks`。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TaskListResponse {
    pub tasks: Vec<TaskInfo>,
}

/// A2A `TaskState` → 稳定 REST 标签。
pub(crate) fn task_state_label(state: TaskState) -> &'static str {
    match state {
        TaskState::Submitted => "submitted",
        TaskState::Working => "working",
        TaskState::InputRequired => "input_required",
        TaskState::Completed => "completed",
        TaskState::Canceled => "canceled",
        TaskState::Failed => "failed",
        TaskState::Rejected => "rejected",
        TaskState::AuthRequired => "auth_required",
        TaskState::Unknown => "unknown",
    }
}

/// 后台作业视图。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JobInfo {
    pub id: String,
    pub kind: String,
    pub command: String,
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<JobOutcomeInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JobOutcomeInfo {
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl JobInfo {
    pub(crate) fn from_view(view: &onion_jobs::JobView) -> Self {
        use onion_jobs::{JobOutcome, JobStatus};
        let (status, outcome) = match &view.status {
            JobStatus::Running => ("running", None),
            JobStatus::Settled(outcome) => {
                let info = match outcome {
                    JobOutcome::Exited { code } => JobOutcomeInfo {
                        kind: "exited",
                        exit_code: Some(*code),
                        error: None,
                    },
                    JobOutcome::Killed => JobOutcomeInfo {
                        kind: "killed",
                        exit_code: None,
                        error: None,
                    },
                    JobOutcome::Failed { error } => JobOutcomeInfo {
                        kind: "failed",
                        exit_code: None,
                        error: Some(error.clone()),
                    },
                };
                ("settled", Some(info))
            }
        };
        Self {
            id: view.spec.id.0.clone(),
            kind: view.spec.kind.to_string(),
            command: view.spec.command.clone(),
            status,
            outcome,
        }
    }
}

/// `GET /v1/sessions/{id}/jobs`。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JobListResponse {
    pub jobs: Vec<JobInfo>,
}

/// `GET /v1/sessions/{id}/jobs/{jobId}`。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JobDetail {
    pub job: JobInfo,
    pub output: String,
}

/// 作业终止回执。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JobKillAck {
    pub job_id: String,
    pub result: &'static str,
}

#[cfg(test)]
#[path = "dto_tests.rs"]
mod tests;
