//! 确定性裁剪（工具结果裁剪 + 图片卸载）。
//!
//! 视图级变换：持久全史（Context 投影）不动，每次请求装配期确定性重放同一
//! 变换——恢复路径零成本，无 seq-keyed 账本与持久化 overrides 表。
//!
//! - 文本裁剪（E1）：单条工具结果文本总字符超 [`PRUNE_THRESHOLD_CHARS`] 时，
//!   保留 head（[`PRUNE_HEAD_CHARS`]）+ [`PRUNE_MARKER`] + tail
//!   （[`PRUNE_TAIL_CHARS`]），中段丢弃——head 覆盖开头状态、tail 覆盖末尾
//!   结论，中段（长文件 dump / 命令日志）对后续任务价值最低。
//! - 图片卸载（E1.5）：溢出路径把 ToolResult 中的图片块替换为占位文本，从
//!   最老消息起逐条卸载直至估算入窗。

use super::estimate::{IMAGE_TOKEN_ESTIMATE, estimate_message};
use crate::types::{AgentMessage, ContentBlock, Role};

/// 裁剪标记（固定文本）。
pub const PRUNE_MARKER: &str = "\n\n[... tool result middle pruned ...]\n\n";

/// 触发裁剪的单条工具结果文本总字符阈值。
pub const PRUNE_THRESHOLD_CHARS: usize = 8_192;

/// 裁剪保留的头部字符数。
pub const PRUNE_HEAD_CHARS: usize = 4_096;

/// 裁剪保留的尾部字符数。
pub const PRUNE_TAIL_CHARS: usize = 1_024;

/// 图片卸载占位文本。
pub const IMAGE_OFFLOAD_PLACEHOLDER: &str = "[image content removed to fit context window]";

/// 一次裁剪的明细：被裁消息在输入切片中的索引 + 裁剪后消息估算价。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrunedEntry {
    pub index: usize,
    pub price: u64,
}

/// E1：全部 ToolResult 一视同仁的 head/marker/tail 裁剪（视图级、幂等）。
///
/// 返回 `(新视图, 裁剪明细)`；非工具结果消息原样透传，工具结果内非文本块
/// 原位保留，配对字段（tool_call_id/is_error）不变（API 配对不变量）。
/// 明细按输入切片坐标记录被裁消息；裁剪幂等——已裁结果重跑不再进明细。
pub fn prune_tool_results(messages: &[AgentMessage]) -> (Vec<AgentMessage>, Vec<PrunedEntry>) {
    let mut pruned = Vec::new();
    let view = messages
        .iter()
        .enumerate()
        .map(|(idx, m)| match prune_tool_result(m) {
            Some(pruned_message) => {
                let price = estimate_message(&pruned_message);
                pruned.push(PrunedEntry { index: idx, price });
                pruned_message
            }
            None => m.clone(),
        })
        .collect();
    (view, pruned)
}

/// E1.5：溢出路径图片卸载，从最老消息起把 ToolResult 中的 Image 块换占位
/// 文本直至估算 ≤ limit。
///
/// 返回 `(新视图, 卸载明细)`；明细非空即发生卸载。卸尽仍超限时返回已卸载
/// 视图（估算减少量按图片逐条扣减），由调用方落摘要管线兜底。非工具结果
/// 消息与工具结果内非图片块不动，配对字段保留。
pub fn offload_images_until_fit(
    messages: &[AgentMessage],
    overhead_tokens: u64,
    limit_tokens: u64,
) -> (Vec<AgentMessage>, Vec<PrunedEntry>) {
    let mut est = overhead_tokens.saturating_add(estimate_messages_tokens(messages));
    if est <= limit_tokens {
        return (messages.to_vec(), Vec::new());
    }
    let mut view = messages.to_vec();
    let mut entries = Vec::new();
    for (idx, m) in view.iter_mut().enumerate() {
        if est <= limit_tokens {
            break;
        }
        let saved: u64 = m
            .blocks
            .iter()
            .flat_map(|b| match b {
                ContentBlock::ToolResult { content, .. } => content.iter().collect::<Vec<_>>(),
                other => std::slice::from_ref(other).iter().collect::<Vec<_>>(),
            })
            .filter(|b| matches!(b, ContentBlock::Image { .. }))
            .count() as u64
            * IMAGE_TOKEN_ESTIMATE;
        if saved == 0 {
            continue;
        }
        let blocks = m
            .blocks
            .iter()
            .map(|b| match b {
                ContentBlock::ToolResult { tool_call_id, is_error, content } => {
                    ContentBlock::ToolResult {
                        tool_call_id: tool_call_id.clone(),
                        is_error: *is_error,
                        content: content
                            .iter()
                            .map(|b| match b {
                                ContentBlock::Image { .. } => {
                                    ContentBlock::Text { text: IMAGE_OFFLOAD_PLACEHOLDER.into() }
                                }
                                other => other.clone(),
                            })
                            .collect(),
                    }
                }
                other => other.clone(),
            })
            .collect();
        *m = AgentMessage { role: m.role, blocks, source: m.source.clone() };
        let price = estimate_message(m);
        entries.push(PrunedEntry { index: idx, price });
        est = est.saturating_sub(saved);
    }
    (view, entries)
}

/// 单条确定性裁剪重放（与 [`prune_tool_results`] 同规则；已裁结果重放返回 `None`）。
pub fn replay_prune(message: &AgentMessage) -> Option<AgentMessage> {
    prune_tool_result(message)
}

/// 组合入口：先文本裁剪，估算仍超限再图片卸载（对齐 ava-server strategy.rs 调用序）。
pub fn prune_view(
    messages: &[AgentMessage],
    limit_tokens: u64,
    overhead_tokens: u64,
) -> (Vec<AgentMessage>, Vec<PrunedEntry>) {
    let (view, text_entries) = prune_tool_results(messages);
    let (view, image_entries) = offload_images_until_fit(&view, overhead_tokens, limit_tokens);
    // 图片卸载的索引以文本裁剪后的 view 为基准（E1 段在先、E1.5 段在后）；
    // 文本裁剪明细的 price 是裁剪后消息的估算——同一消息若随后被图片卸载，
    // 其 price 需按卸载后形态重估（对齐 ava-server strategy.rs「明细合并入账」）。
    let mut entries = text_entries;
    for entry in image_entries {
        if let Some(existing) = entries.iter_mut().find(|e| e.index == entry.index) {
            existing.price = entry.price;
        } else {
            entries.push(entry);
        }
    }
    entries.sort_by_key(|e| e.index);
    (view, entries)
}

/// 消息序列估算（`estimate_message` 求和）。
fn estimate_messages_tokens(messages: &[AgentMessage]) -> u64 {
    messages.iter().map(estimate_message).sum()
}

/// 裁剪单条消息：角色为 Tool 且首块为 ToolResult 时，文本总字符超阈按
/// head/marker/tail 裁剪，返回裁剪后的消息；否则返回 `None`。
fn prune_tool_result(message: &AgentMessage) -> Option<AgentMessage> {
    if message.role != Role::Tool {
        return None;
    }
    let Some(ContentBlock::ToolResult { tool_call_id, is_error, content }) = message.blocks.first() else {
        return None;
    };
    let total: usize = content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.chars().count()),
            _ => None,
        })
        .sum();
    if total <= PRUNE_THRESHOLD_CHARS {
        return None;
    }
    let head_end = PRUNE_HEAD_CHARS;
    let tail_start = total - PRUNE_TAIL_CHARS;
    let mut consumed = 0usize;
    let mut marker_inserted = false;
    let mut content_out = Vec::with_capacity(content.len() + 1);
    for block in content {
        match block {
            ContentBlock::Text { text } => {
                let start = consumed;
                let len = text.chars().count();
                consumed = start + len;
                // 头段：与 [0, head_end) 的交叠。
                if start < head_end {
                    let take = len.min(head_end - start);
                    if take > 0 {
                        content_out.push(ContentBlock::Text { text: char_slice(text, 0, take) });
                    }
                }
                // 中段：与 [head_end, tail_start) 的交叠 → 首处插标记。
                if start < tail_start && start + len > head_end && !marker_inserted {
                    marker_inserted = true;
                    content_out.push(ContentBlock::Text { text: PRUNE_MARKER.into() });
                }
                // 尾段：与 [tail_start, total) 的交叠。
                if start + len > tail_start {
                    let from = tail_start.saturating_sub(start).min(len);
                    if from < len {
                        content_out.push(ContentBlock::Text { text: char_slice(text, from, len) });
                    }
                }
            }
            other => content_out.push(other.clone()),
        }
    }
    Some(AgentMessage {
        role: Role::Tool,
        blocks: vec![ContentBlock::ToolResult {
            tool_call_id: tool_call_id.clone(),
            is_error: *is_error,
            content: content_out,
        }],
        source: None,
    })
}

/// 按字符下标切片（`from`/`to` 为字符坐标，UTF-8 安全）。
fn char_slice(text: &str, from: usize, to: usize) -> String {
    text.chars().skip(from).take(to - from).collect()
}

#[cfg(test)]
#[path = "prune_tests.rs"]
mod tests;
