//! 压缩区域事务纯逻辑：范围选择 + 配对平衡 + 收缩校验。
//!
//! 语义要点：
//! - system head 保护：system 是 `Option<String>` 独立字段，天然不在
//!   `messages` 内，首个可压缩索引恒为 0；
//! - 尾部 retainTokens 逆序累积；
//! - tool-pairing 平衡回退（切点前 tool_use 与
//!   tool_result 数量平衡才可切）；
//! - 事务编排（start → summarize → summary → replace → end）在 kernel 执行。

use crate::types::{AgentMessage, ContentBlock};

use super::config::CompactSpec;
use super::estimate::estimate_message;
use super::summarizer::COMPACTION_INSTRUCTION;

/// 被压缩区间：投影 `messages` 的半开区间 `[start, start + count)`。
/// 起点恒为 0（头部 system 不在 messages 内）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShadowedRange {
    pub start: usize,
    pub count: usize,
}

/// 在第 `idx` 条消息之前切割是否 tool-pairing 平衡
/// （前缀中 ToolUse 与 ToolResult 数量相等）。
pub fn cut_balanced_before(messages: &[AgentMessage], idx: usize) -> bool {
    let mut in_progress: i64 = 0;
    for message in &messages[..idx] {
        for block in &message.blocks {
            match block {
                ContentBlock::ToolUse { .. } => in_progress += 1,
                ContentBlock::ToolResult { .. } => in_progress -= 1,
                _ => {}
            }
        }
    }
    in_progress == 0
}

/// 范围选择：
/// 尾部 retainTokens 逆序累积 → tool-pairing 平衡回退 → 空区间 None。
pub fn select_compactable_range(
    messages: &[AgentMessage],
    spec: &CompactSpec,
) -> Option<ShadowedRange> {
    if messages.is_empty() {
        return None;
    }
    let mut accumulated: u64 = 0;
    let mut keep_from = messages.len();
    for index in (0..messages.len()).rev() {
        accumulated = accumulated.saturating_add(estimate_message(&messages[index]));
        keep_from = index;
        if accumulated >= spec.retain_tokens {
            break;
        }
    }
    // keep_from == 0：全部消息落在保留区，无可压缩区域。
    if keep_from == 0 {
        return None;
    }
    // tool-pairing 回退：切点若落在配对中间，向前退到平衡切点；退无可退 → None。
    while keep_from > 0 && !cut_balanced_before(messages, keep_from) {
        keep_from -= 1;
    }
    if keep_from == 0 {
        return None;
    }
    Some(ShadowedRange { start: 0, count: keep_from })
}

/// 压缩输入装配（裁剪版）：
/// shadowed 区域消息重放 + instruction 作为最后 user 消息
/// （replay 对话前缀复用 KV cache）。
pub fn build_summarization_input(
    messages: &[AgentMessage],
    range: ShadowedRange,
) -> Vec<AgentMessage> {
    let mut replay: Vec<AgentMessage> =
        messages[range.start..range.start + range.count].to_vec();
    replay.push(AgentMessage::user(COMPACTION_INSTRUCTION));
    replay
}

/// 发起摘要事务的 shadowed region 估算下限（2 的幂，对齐单图估算口径）。
///
/// 框架摘要（`<compaction>` 标签包裹）自身就有数百 token——被替换区域低于
/// 该下限时，事务注定以 no-shrink 失败收场（2026-11-08 实证：E1.5 卸载图片
/// 后 shadowed 仅 80 tok 仍发起摘要 → framed summary (513 tok) does not
/// shrink the shadowed region (80 tok)，白费一次 LLM 调用后降级继续）。
/// 事务发起前校验，低于下限直接跳过（bracket 未开，无 CompactionStart/End）。
pub const MIN_SHADOWED_TOKENS: u64 = 2_048;

/// 收缩校验（commit 前检查）：框架摘要 token 估算必须严格小于被替换区域 token 估算。
pub fn assert_summary_shrinks(
    summary_tokens: u64,
    shadowed_tokens: u64,
) -> Result<(), String> {
    if summary_tokens >= shadowed_tokens {
        return Err(format!(
            "compaction: framed summary ({summary_tokens} tok) does not shrink the shadowed region ({shadowed_tokens} tok)"
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "region_tests.rs"]
mod tests;
