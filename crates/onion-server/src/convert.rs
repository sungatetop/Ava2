//! 消息内容转换：A2A `Part` ↔ onion `ContentBlock`（A2A 与 REST 共用）。
//!
//! `parts` 语义与 A2A 一致：`text` / `file`（URI → 图片块）/ `data`（JSON 序列化
//! 进文本块）。REST 的 `POST /v1/sessions/{id}/messages` 复用同一转换，避免两套
//! 消息语义漂移。

use crate::a2a::{A2aFile, Message, Part};

/// `Part` 序列 → 一条 user `AgentMessage`。
pub(crate) fn parts_to_agent_message(parts: &[Part]) -> onion::types::AgentMessage {
    let blocks: Vec<onion::types::ContentBlock> = parts
        .iter()
        .map(|part| match part {
            Part::Text { text, .. } => onion::types::ContentBlock::Text { text: text.clone() },
            Part::File { file, .. } => {
                let uri = file.as_ref().and_then(|f| f.uri.clone()).unwrap_or_default();
                onion::types::ContentBlock::Image { image_url: uri }
            }
            Part::Data { data, .. } => onion::types::ContentBlock::Text {
                text: serde_json::to_string(data).unwrap_or_default(),
            },
        })
        .collect();
    onion::types::AgentMessage {
        role: onion::types::Role::User,
        blocks,
        source: None,
    }
}

/// A2A `Message` → 一条 user `AgentMessage`。
///
/// `message.metadata.sender`（由发送侧 `a2a_send` 写入）承载发送方身份：
/// - `source = A2aPeer { instance_id, name }`，供会话与前端区分外部实例消息；
/// - 首个文本块插入 `A2A message from {name}:` 说话人标注（对齐 agent-team
///   的 sender framing），使接收模型知道谁在说话。
pub(crate) fn message_to_agent_message(message: &Message) -> onion::types::AgentMessage {
    let mut agent = parts_to_agent_message(&message.parts);
    let Some(sender) = message.metadata.as_ref().and_then(|metadata| metadata.get("sender")) else {
        return agent;
    };
    let instance_id = sender
        .get("instanceId")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_string();
    let name = sender
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            if instance_id.is_empty() { "peer".to_string() } else { instance_id.clone() }
        });
    agent.blocks.insert(
        0,
        onion::types::ContentBlock::Text { text: format!("A2A message from {name}:") },
    );
    agent.source = Some(onion::types::MessageSource::A2aPeer { instance_id, name });
    agent
}

/// onion `ContentBlock` 序列 → A2A `Part` 序列。
pub(crate) fn blocks_to_parts(blocks: &[onion::types::ContentBlock]) -> Vec<Part> {
    blocks
        .iter()
        .map(|block| match block {
            onion::types::ContentBlock::Text { text } => Part::text(text.clone()),
            onion::types::ContentBlock::Thinking { thinking } => {
                Part::text(format!("(thinking) {thinking}"))
            }
            onion::types::ContentBlock::Image { image_url } => Part::File {
                file: Some(A2aFile {
                    name: None,
                    mime_type: Some("image/*".to_string()),
                    bytes: None,
                    uri: Some(image_url.clone()),
                }),
                metadata: None,
            },
            onion::types::ContentBlock::ToolUse { id, name, input } => Part::Data {
                data: serde_json::json!({ "tool_use": { "id": id, "name": name, "input": input } }),
                metadata: None,
            },
            onion::types::ContentBlock::ToolResult {
                tool_call_id,
                is_error,
                content,
            } => Part::Data {
                data: serde_json::json!({ "tool_result": { "tool_call_id": tool_call_id, "is_error": is_error, "content": content } }),
                metadata: None,
            },
        })
        .collect()
}
