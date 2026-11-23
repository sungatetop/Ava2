//! subagent 通信：relay / notice 双 source 投递到父会话 inbox。
//!
//! relay（agent-message）= subagent 最终 assistant 消息；
//! notice（subagent-settled）= 运行时关于子代理的一行账目。
//! 两条通道的分区由 [`Delivery`] 决定：
//! 父代理**空闲**时落 next-turn 排队，**运行中**时落 next-step 插队；
//! 投递后一律唤醒父会话（waking 语义）。

use crate::inbox::Inbox;
use crate::types::{AgentMessage, ContentBlock, MessageSource};

use super::types::SubagentError;

/// 投递分区（`queue` | `steer`）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
    /// 落 next-turn：等父代理下一个 turn 被接纳时，与用户输入一并消费。
    Queue,
    /// 落 next-step：插到父代理最近一步之前（运行中即时生效）。
    Steer,
}

impl Delivery {
    pub fn label(self) -> &'static str {
        match self {
            Self::Queue => "queue",
            Self::Steer => "steer",
        }
    }

    /// 由投递方显式声明（`send_message` 的 `delivery` 参数）解析，缺省 `queue`。
    pub fn parse(raw: Option<&str>) -> Result<Self, SubagentError> {
        match raw {
            None | Some("queue") => Ok(Self::Queue),
            Some("steer") => Ok(Self::Steer),
            Some(other) => Err(SubagentError::Invalid(format!(
                "delivery must be \"queue\" or \"steer\", got \"{other}\""
            ))),
        }
    }

    /// 运行时自动投递的选择（父空闲 → queue，运行中 → steer）。
    pub fn for_parent_idle(parent_idle: bool) -> Self {
        if parent_idle { Self::Queue } else { Self::Steer }
    }

    fn push(self, inbox: &Inbox, message: AgentMessage) -> Result<(), SubagentError> {
        match self {
            Self::Queue => inbox.followup(message)?,
            Self::Steer => inbox.steer(message)?,
        }
        Ok(())
    }
}

/// relay：subagent 最终 assistant 消息 → 父会话 inbox（分区见 [`Delivery`]）。
///
/// source = `SubagentMessage { agent_id }`；投递失败透传 `InboxError`（如分区满）。
pub fn relay(
    inbox: &Inbox,
    agent_id: &str,
    final_assistant: AgentMessage,
    delivery: Delivery,
) -> Result<(), SubagentError> {
    let message = AgentMessage {
        role: final_assistant.role,
        blocks: final_assistant.blocks,
        source: Some(MessageSource::SubagentMessage {
            agent_id: agent_id.to_string(),
        }),
    };
    delivery.push(inbox, message)?;
    Ok(())
}

/// settle 通知的首行摘要：用父代理自己
/// 的任务词汇说明「后台子代理结束了、为什么」。
fn settlement_summary(name: &str, stop_reason: &str, continuable: bool) -> String {
    let subject = format!("Background subagent {name}");
    match stop_reason {
        "completed" if continuable => {
            format!("{subject} finished and will do no further work unless you send it more.")
        }
        "completed" => format!("{subject} finished. It cannot receive follow-up messages."),
        "aborted" => format!("{subject} was stopped before it finished."),
        "max-tokens" => format!("{subject} ran out of room before it finished."),
        "refusal" => format!("{subject} declined the task."),
        "error" => format!("{subject} failed before it finished."),
        other => format!("{subject} ended abnormally ({other}) before it finished."),
    }
}

/// notice：subagent settle 事实 → 父会话 next-step 队列（`subagent-settled`
/// notice：**model-visible** 的一行账目 + 结束原因）。
///
/// 与 relay 的分工（continuation 消息语义）：
/// - `closing_delivered = true`：relay 已把子代理收尾消息单独投递，notice 不再重复，
///   改注一句「它结束前给你发了消息，收尾内容已省略」；
/// - `closing_delivered = false`：子代理没有收尾消息，notice 明确说明。
///
/// source = `SubagentSettled { agent_id, stop_reason, summary }`（durable 归因）。
pub fn notice(
    inbox: &Inbox,
    agent_id: &str,
    name: &str,
    stop_reason: &str,
    continuable: bool,
    closing_delivered: bool,
    delivery: Delivery,
) -> Result<(), SubagentError> {
    let summary = settlement_summary(name, stop_reason, continuable);
    let tail = if closing_delivered {
        "It sent you a message just before finishing, so its closing message is omitted."
    } else {
        "It left no closing message."
    };
    let message = AgentMessage {
        role: crate::types::Role::User,
        blocks: vec![
            ContentBlock::Text { text: summary.clone() },
            ContentBlock::Text { text: tail.to_string() },
        ],
        source: Some(MessageSource::SubagentSettled {
            agent_id: agent_id.to_string(),
            stop_reason: Some(stop_reason.to_string()),
            summary: Some(summary),
        }),
    };
    delivery.push(inbox, message)?;
    Ok(())
}

/// 便捷构造：从 subagent 最终 assistant 消息提取文本（relay 内容承载）。
pub fn final_assistant_text(text: impl Into<String>) -> AgentMessage {
    AgentMessage {
        role: crate::types::Role::Assistant,
        blocks: vec![ContentBlock::Text { text: text.into() }],
        source: None,
    }
}

/// 提取消息里的纯文本拼接（`subagent/end` 的 `lastAssistantMessage` 摘要）。
pub fn final_message_text(message: &AgentMessage) -> String {
    message
        .blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
#[path = "relay_tests.rs"]
mod tests;
