//! durable 投影：Context 从 SessionLog 事件 fold 重建；derive_request 派生请求草稿。
//! 「model-visible means logged」：system prompt 也经 SystemMessage durable 事件进入投影。

use crate::session::SessionEvent;
use crate::types::{AgentMessage, MessageSource, ToolDefinition};

/// 请求草稿：内核装配后交 request 瀑布确认/改写。
#[derive(Clone, Debug, PartialEq)]
pub struct RequestDraft {
    pub model: String,
    pub system: Option<String>,
    pub messages: Vec<AgentMessage>,
    pub tools: Vec<ToolDefinition>,
    pub max_tokens: u32,
    pub temperature: Option<f32>,
    pub starts_request_series: bool,
}

/// 模型可见上下文（system + 消息 + 工具表）。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Context {
    pub system: Option<String>,
    pub messages: Vec<AgentMessage>,
    pub tools: Vec<ToolDefinition>,
}

impl Context {
    /// 单事件应用（durable → 投影）。空 assistant 消息不投影（保持配对不变式由事件层保证）。
    pub fn apply(&mut self, event: &SessionEvent) {
        match event {
            SessionEvent::SystemMessage { message, .. } => self.system = Some(message.clone()),
            SessionEvent::UserMessage { message, .. }
            | SessionEvent::AssistantMessage { message, .. } => {
                // 压缩检查点消息（source `compact-checkpoint`）是上下文替换的
                // durable 载体（紧随 `compaction/summary` 事件落盘），
                // 但替换动作已由上面的 `CompactionSummary` 分支完成——此处只保留
                // 日志事件供非模型消费者识别，不再重复投影正文。
                if matches!(message.source, Some(MessageSource::CompactCheckpoint { .. })) {
                    return;
                }
                if !message.blocks.is_empty() {
                    self.messages.push(message.clone());
                }
            }
            SessionEvent::ToolResult { result, .. } => {
                self.messages.push(AgentMessage::tool_result(result));
            }
            SessionEvent::CompactionSummary { summary, shadowed_count, compaction_id, .. } => {
                // 压缩投影：shadowed 区间 [0, shadowed_count) 替换为一条
                // `<compacted-summary>` 承载消息（头部 system 不动；尾部 retain 不动）。
                // 载体消息带 `compact-checkpoint` source（检查点语义）。
                let count = (*shadowed_count).min(self.messages.len());
                let framed =
                    crate::compaction::summarizer::frame_checkpoint(summary, compaction_id);
                self.messages.splice(0..count, std::iter::once(framed));
            }
            _ => {}
        }
    }

    /// durable 事件 fold 重建（与逐条 apply 等价——单测锁定）。
    pub fn project(events: &[SessionEvent], tools: Vec<ToolDefinition>) -> Self {
        let mut context = Self { system: None, messages: Vec::new(), tools };
        for event in events {
            context.apply(event);
        }
        context
    }

    /// 派生请求草稿：`context.messages + pending`（pending = 本步新提交、尚未投影的消息）。
    pub fn derive_request(
        &self,
        model: String,
        max_tokens: u32,
        temperature: Option<f32>,
        pending: Vec<AgentMessage>,
        starts_request_series: bool,
    ) -> RequestDraft {
        let mut messages = self.messages.clone();
        messages.extend(pending);
        RequestDraft {
            model,
            system: self.system.clone(),
            messages,
            tools: self.tools.clone(),
            max_tokens,
            temperature,
            starts_request_series,
        }
    }
}

#[cfg(test)]
#[path = "context_tests.rs"]
mod tests;
