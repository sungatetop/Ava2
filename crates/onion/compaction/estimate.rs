//! 启发式 token 测量（chars/4 近似；无 tokenizer 依赖）。
//!
//! 采用保守估算（estimate ≥ threshold → 触发）。

use crate::context::Context;
use crate::session::SessionEvent;
use crate::types::{AgentMessage, ContentBlock, ToolDefinition};

/// 4 字符 ≈ 1 token（英文近似；中文更保守，教学示例可接受）。
const CHARS_PER_TOKEN: f64 = 4.0;

/// 单张图片进入模型上下文的固定 token 估算（预算后单图封顶，2 的幂保守取整）。
///
/// 契约：`media` 模块的预算管线保证进入模型的图片像素 ≤ 1.15MP（1.15M）——
/// 按 Claude 口径（px/750）≈1.5k tokens，48/v1 网关实测 1-4k，向上保守取整
/// 到 2048。估算必须面向预算后的请求视图，而非 state 中的原始 data URL：
/// 按 data URL 字符数折算会把一张 1920×1920 截图估成 ~100k+ tokens（实际
/// 1-4k），令压力判断与图片卸载误杀正常截图（2026-11-08 事故）。
pub const IMAGE_TOKEN_ESTIMATE: u64 = 2_048;

/// 单条消息 token 估算（委托 [`estimate_blocks`]）。
pub fn estimate_message(message: &AgentMessage) -> u64 {
    estimate_blocks(&message.blocks)
}

/// 平铺块序列 token 估算（消息块 / 工具结果内容共用口径）。
pub fn estimate_blocks(blocks: &[ContentBlock]) -> u64 {
    let mut chars: f64 = 0.0;
    let mut images: u64 = 0;
    for block in blocks {
        match block {
            ContentBlock::Text { text } => chars += text.chars().count() as f64,
            ContentBlock::Thinking { thinking } => chars += thinking.chars().count() as f64,
            ContentBlock::Image { .. } => images += 1,
            ContentBlock::ToolUse { id, name, input } => {
                chars += id.chars().count() as f64
                    + name.chars().count() as f64
                    + input.to_string().chars().count() as f64;
            }
            ContentBlock::ToolResult { tool_call_id, content, .. } => {
                chars += tool_call_id.chars().count() as f64;
                for inner in content {
                    match inner {
                        ContentBlock::Text { text } => chars += text.chars().count() as f64,
                        ContentBlock::Image { .. } => images += 1,
                        _ => {}
                    }
                }
            }
        }
    }
    // role + 块结构开销：每消息固定 4 token；图片按预算后固定单图估算。
    4 + images * IMAGE_TOKEN_ESTIMATE + (chars / CHARS_PER_TOKEN).ceil() as u64
}

/// Context 全量估算（system + 所有消息 + 工具 schema）。
pub fn estimate_context(context: &Context) -> u64 {
    let mut total: u64 = context
        .system
        .as_deref()
        .map(|s| (s.chars().count() as f64 / CHARS_PER_TOKEN).ceil() as u64)
        .unwrap_or(0);
    for message in &context.messages {
        total = total.saturating_add(estimate_message(message));
    }
    for tool in &context.tools {
        let schema_chars = serde_json::to_string(tool)
            .map(|s| s.chars().count() as f64)
            .unwrap_or(0.0);
        total = total.saturating_add((schema_chars / CHARS_PER_TOKEN).ceil() as u64);
    }
    total
}

/// 请求级开销估算（system 文本 + 工具 schema，不含消息）。
pub fn estimate_overhead(system: Option<&str>, tools: &[ToolDefinition]) -> u64 {
    let mut total: u64 = system
        .map(|s| (s.chars().count() as f64 / CHARS_PER_TOKEN).ceil() as u64)
        .unwrap_or(0);
    for tool in tools {
        let schema_chars = serde_json::to_string(tool)
            .map(|s| s.chars().count() as f64)
            .unwrap_or(0.0);
        total = total.saturating_add((schema_chars / CHARS_PER_TOKEN).ceil() as u64);
    }
    total
}

/// durable 事件流的上下文估算（会话上下文占比的唯一口径，chars/4）。
///
/// 宿主（TUI 状态栏 / 桌面端占比条）统一经此函数估算，避免两端口径漂移。
pub fn estimate_events(events: &[SessionEvent]) -> u64 {
    let mut chars: f64 = 0.0;
    for event in events {
        match event {
            SessionEvent::SystemMessage { message, .. } => {
                chars += message.chars().count() as f64;
            }
            SessionEvent::UserMessage { message, .. } => {
                // 压缩检查点消息不计入：它是替换投影的语义标记，正文与被
                // shadowed 区间重叠，计入会重复估算（CompactionSummary 亦不计）。
                if !matches!(message.source, Some(crate::types::MessageSource::CompactCheckpoint { .. }))
                {
                    chars += message_text_len(message) as f64;
                }
            }
            SessionEvent::AssistantMessage { message, .. } => {
                chars += message_text_len(message) as f64;
            }
            SessionEvent::ToolResult { result, .. } => {
                chars += result.text_content().chars().count() as f64;
            }
            SessionEvent::ToolCall { call, .. } => {
                chars += call.name.chars().count() as f64;
                chars += call.input.to_string().chars().count() as f64;
            }
            _ => {}
        }
    }
    (chars / CHARS_PER_TOKEN).ceil() as u64
}

/// AgentMessage 文本长度（Text / Thinking 块）。
fn message_text_len(message: &AgentMessage) -> usize {
    message
        .blocks
        .iter()
        .map(|block| match block {
            ContentBlock::Text { text } => text.chars().count(),
            ContentBlock::Thinking { thinking } => thinking.chars().count(),
            _ => 0,
        })
        .sum()
}

#[cfg(test)]
#[path = "estimate_tests.rs"]
mod tests;
