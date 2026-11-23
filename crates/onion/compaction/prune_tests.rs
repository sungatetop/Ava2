//! `prune` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//!
//! 判定口径为字符数（code points）；
//! 裁剪坐标为多文本块拼接坐标（与单字符串语义等价）。

#![allow(clippy::unwrap_used, clippy::expect_used)]
use super::*;
use crate::compaction::estimate::{IMAGE_TOKEN_ESTIMATE, estimate_overhead, estimate_message};
use crate::types::ToolDefinition;

fn tool_message(content: Vec<ContentBlock>) -> AgentMessage {
    AgentMessage {
        role: Role::Tool,
        blocks: vec![ContentBlock::ToolResult {
            tool_call_id: "call-1".into(),
            is_error: false,
            content,
        }],
        source: None,
    }
}

fn text_block(chars: usize) -> ContentBlock {
    ContentBlock::Text { text: "x".repeat(chars) }
}

/// 工具结果各文本块的 (文本, 字符数) 列表。
fn text_parts(msg: &AgentMessage) -> Vec<(String, usize)> {
    msg.blocks
        .iter()
        .flat_map(|b| match b {
            ContentBlock::ToolResult { content, .. } => content
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } => Some((text.clone(), text.chars().count())),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect()
}

fn image_block() -> ContentBlock {
    ContentBlock::Image { image_url: "data:image/png;base64,AAAA".into() }
}

// ---- prune_tool_results：工具结果裁剪 ----

#[test]
fn marker_budget_fits_threshold() {
    // 校验语义：head + marker + tail ≤ threshold。
    assert!(
        PRUNE_HEAD_CHARS + PRUNE_TAIL_CHARS + PRUNE_MARKER.chars().count()
            <= PRUNE_THRESHOLD_CHARS
    );
}

#[test]
fn under_threshold_unchanged() {
    let messages = vec![tool_message(vec![text_block(100)])];
    let (view, pruned) = prune_tool_results(&messages);
    assert!(pruned.is_empty());
    assert_eq!(view, messages);
}

#[test]
fn exactly_threshold_unchanged() {
    let messages = vec![tool_message(vec![text_block(PRUNE_THRESHOLD_CHARS)])];
    let (view, pruned) = prune_tool_results(&messages);
    assert!(pruned.is_empty());
    assert_eq!(view, messages);
}

#[test]
fn one_over_threshold_pruned() {
    let messages = vec![tool_message(vec![text_block(PRUNE_THRESHOLD_CHARS + 1)])];
    let (view, pruned) = prune_tool_results(&messages);
    assert_eq!(pruned.len(), 1);
    let sizes: Vec<usize> = text_parts(&view[0]).into_iter().map(|(_, n)| n).collect();
    assert_eq!(sizes, vec![PRUNE_HEAD_CHARS, PRUNE_MARKER.chars().count(), PRUNE_TAIL_CHARS]);
}

#[test]
fn single_block_pruned_to_head_marker_tail() {
    let messages = vec![tool_message(vec![text_block(9_000)])];
    let (view, pruned) = prune_tool_results(&messages);
    assert_eq!(pruned.len(), 1);
    let parts = text_parts(&view[0]);
    assert_eq!(parts.len(), 3, "head + marker + tail");
    assert_eq!(parts[0].1, PRUNE_HEAD_CHARS);
    assert_eq!(parts[1].0, PRUNE_MARKER);
    assert_eq!(parts[2].1, PRUNE_TAIL_CHARS);
}

#[test]
fn multi_block_cut_positions_follow_concat_coords() {
    // [4000, 5000] 拼接坐标：head 止于 4096（次块前 96 字符），tail 起于 7976。
    let messages = vec![tool_message(vec![text_block(4_000), text_block(5_000)])];
    let (view, pruned) = prune_tool_results(&messages);
    assert_eq!(pruned.len(), 1);
    let parts = text_parts(&view[0]);
    assert_eq!(parts.len(), 4);
    assert_eq!(parts[0].1, 4_000, "首块整体落在 head 区间内");
    assert_eq!(parts[1].1, PRUNE_HEAD_CHARS - 4_000, "次块头部补齐 head 段");
    assert_eq!(parts[2].0, PRUNE_MARKER);
    assert_eq!(parts[3].1, PRUNE_TAIL_CHARS);
}

#[test]
fn marker_inserted_once_across_blocks() {
    // 两个超阈文本块：标记只在首处丢弃位置出现一次。
    let messages = vec![tool_message(vec![text_block(9_000), text_block(9_000)])];
    let (view, pruned) = prune_tool_results(&messages);
    assert_eq!(pruned.len(), 1);
    let markers = text_parts(&view[0])
        .into_iter()
        .filter(|(t, _)| t == PRUNE_MARKER)
        .count();
    assert_eq!(markers, 1);
    // 拼接保留总量恒为 head + tail。
    let kept: usize = text_parts(&view[0])
        .into_iter()
        .filter(|(t, _)| t != PRUNE_MARKER)
        .map(|(_, n)| n)
        .sum();
    assert_eq!(kept, PRUNE_HEAD_CHARS + PRUNE_TAIL_CHARS);
}

#[test]
fn non_text_blocks_preserved_in_place() {
    let image = image_block();
    let messages = vec![tool_message(vec![text_block(9_000), image.clone(), text_block(9_000)])];
    let (view, pruned) = prune_tool_results(&messages);
    assert_eq!(pruned.len(), 1);
    let blocks = &view[0].blocks;
    let ContentBlock::ToolResult { content, .. } = &blocks[0] else {
        panic!("应为工具结果块");
    };
    assert_eq!(content[2], image, "非文本块原位保留");
    let parts = text_parts(&view[0]);
    assert_eq!(parts[0].1, PRUNE_HEAD_CHARS, "首文本块裁为 head");
    assert_eq!(parts[2].1, PRUNE_TAIL_CHARS, "末文本块裁为 tail");
}

#[test]
fn pairing_fields_preserved() {
    let messages = vec![AgentMessage {
        role: Role::Tool,
        blocks: vec![ContentBlock::ToolResult {
            tool_call_id: "call-xyz".into(),
            is_error: true,
            content: vec![text_block(9_000)],
        }],
        source: None,
    }];
    let (view, _) = prune_tool_results(&messages);
    let blocks = &view[0].blocks;
    let ContentBlock::ToolResult { tool_call_id, is_error, content } = &blocks[0] else {
        panic!("应为工具结果块");
    };
    assert_eq!(tool_call_id, "call-xyz", "配对字段不动");
    assert!(*is_error);
    assert_eq!(content.len(), 3, "head + marker + tail 共 3 块");
}

#[test]
fn mixed_messages_only_tool_results_pruned() {
    let messages = vec![
        AgentMessage::user("hi"),
        tool_message(vec![text_block(50)]),
        tool_message(vec![text_block(9_000)]),
    ];
    let (view, pruned) = prune_tool_results(&messages);
    assert_eq!(pruned, vec![PrunedEntry { index: 2, price: pruned[0].price }]);
    assert_eq!(view[0], messages[0], "user 消息透传");
    assert_eq!(view[1], messages[1], "未超阈工具结果透传");
    let kept: usize = text_parts(&view[2])
        .into_iter()
        .filter(|(t, _)| t != PRUNE_MARKER)
        .map(|(_, n)| n)
        .sum();
    assert_eq!(kept, PRUNE_HEAD_CHARS + PRUNE_TAIL_CHARS);
}

#[test]
fn cjk_text_pruned_by_char_count() {
    // CJK 按字符数判定（code points），非字节：4000 个汉字总字符 4000 < 8192 不裁。
    let messages = vec![tool_message(vec![ContentBlock::Text { text: "汉".repeat(4_000) }])];
    let (view, pruned) = prune_tool_results(&messages);
    assert!(pruned.is_empty());
    assert_eq!(view, messages);
}

#[test]
fn entries_record_slice_index_and_repriced_price() {
    // 明细坐标 = 输入切片索引（非 view 相对）；price = 裁剪后消息估算
    //（head + marker + tail 重估），且严格小于原价。
    let messages = vec![
        AgentMessage::user("hi"),
        tool_message(vec![text_block(50)]),
        tool_message(vec![text_block(9_000)]),
        tool_message(vec![text_block(60_000)]),
    ];
    let (_, pruned) = prune_tool_results(&messages);
    assert_eq!(pruned.iter().map(|e| e.index).collect::<Vec<_>>(), vec![2, 3]);
    let repriced_small = estimate_message(&messages[2]);
    let repriced_big = estimate_message(&messages[3]);
    // 重定价 = head+marker+tail 常量形态（与原文规模无关），且严格低于原价。
    assert_eq!(pruned[0].price, pruned[1].price);
    assert!(pruned[0].price < repriced_small);
    assert!(pruned[1].price < repriced_big);
}

#[test]
fn re_prune_is_idempotent_no_new_entries() {
    // 幂等：已裁结果（head+marker+tail ≤ threshold）重跑不产生新明细。
    let messages = vec![tool_message(vec![text_block(9_000)])];
    let (view, _) = prune_tool_results(&messages);
    let (_, pruned_again) = prune_tool_results(&view);
    assert!(pruned_again.is_empty());
    assert_eq!(prune_tool_results(&view).0, view);
}

// ---- offload_images_until_fit：溢出图片卸载 ----

#[test]
fn offload_noop_when_under_limit() {
    let messages = vec![tool_message(vec![image_block()])];
    let (view, entries) = offload_images_until_fit(&messages, 0, 100_000);
    assert!(entries.is_empty());
    assert_eq!(view, messages);
}

#[test]
fn offload_without_images_reports_unchanged() {
    let messages = vec![tool_message(vec![text_block(9_000)])];
    let (view, entries) = offload_images_until_fit(&messages, 0, 100);
    assert!(entries.is_empty());
    assert_eq!(view, messages);
}

#[test]
fn offload_oldest_first_until_fit() {
    // 预算后单图固定估算 2048；limit = 3_000：两条图片消息共 4_104 tok 超限，
    // 卸载最老一条后 2_052 ≤ 3_000 收手，最新一条图片保留。
    let big_url = format!("data:image/png;base64,{}", "A".repeat(80_000));
    let messages = vec![
        tool_message(vec![ContentBlock::Image { image_url: big_url.clone() }]),
        tool_message(vec![ContentBlock::Image { image_url: big_url }]),
    ];
    let (view, entries) = offload_images_until_fit(&messages, 0, 3_000);
    assert_eq!(entries.len(), 1, "卸载明细非空即发生卸载");
    assert_eq!(entries[0].index, 0, "明细记录输入切片索引");
    let parts = text_parts(&view[0]);
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].0, IMAGE_OFFLOAD_PLACEHOLDER, "最老一条图片被卸载");
    assert_eq!(view[1], messages[1], "入窗后最新一条保留");
}

#[test]
fn offload_all_when_images_alone_exceed_limit() {
    // 卸尽仍超限 → 全部卸载（明细两条，由调用方落摘要管线兜底）。
    let big_url = format!("data:image/png;base64,{}", "A".repeat(80_000));
    let messages = vec![
        tool_message(vec![ContentBlock::Image { image_url: big_url.clone() }]),
        tool_message(vec![ContentBlock::Image { image_url: big_url }]),
    ];
    let (view, entries) = offload_images_until_fit(&messages, 0, 1_000);
    assert_eq!(entries.len(), 2);
    for m in &view {
        let parts = text_parts(m);
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].0, IMAGE_OFFLOAD_PLACEHOLDER);
    }
}

#[test]
fn offload_preserves_pairing_and_non_image_blocks() {
    let big_url = format!("data:image/png;base64,{}", "A".repeat(80_000));
    let messages = vec![AgentMessage {
        role: Role::Tool,
        blocks: vec![ContentBlock::ToolResult {
            tool_call_id: "call-img".into(),
            is_error: false,
            content: vec![text_block(10), ContentBlock::Image { image_url: big_url }, text_block(20)],
        }],
        source: None,
    }];
    let (view, entries) = offload_images_until_fit(&messages, 0, 1_000);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].price, estimate_message(&view[0]), "明细价 = 卸载后估算价");
    let blocks = &view[0].blocks;
    let ContentBlock::ToolResult { tool_call_id, is_error, content } = &blocks[0] else {
        panic!("应为工具结果块");
    };
    assert_eq!(tool_call_id, "call-img");
    assert!(!*is_error);
    assert_eq!(content.len(), 3, "块数不变，图片块原位替换");
    let texts: Vec<String> = content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        texts,
        vec!["x".repeat(10), IMAGE_OFFLOAD_PLACEHOLDER.to_string(), "x".repeat(20)]
    );
}

// ---- replay_prune：单条确定性裁剪重放（恢复路径） ----

#[test]
fn replay_under_threshold_is_none() {
    let message = tool_message(vec![text_block(100)]);
    assert!(replay_prune(&message).is_none());
    assert!(replay_prune(&AgentMessage::user("hi")).is_none(), "非工具结果恒 None");
}

#[test]
fn replay_matches_prune_shape() {
    // 重放形态与批量裁剪同一确定性规则（恢复路径的覆盖形态正确性）。
    let messages = vec![tool_message(vec![text_block(9_000)])];
    let (pruned_view, _) = prune_tool_results(&messages);
    let replayed = replay_prune(&messages[0]).expect("超阈应重放出已裁形态");
    assert_eq!(replayed, pruned_view[0]);
}

#[test]
fn replay_is_idempotent_on_pruned_shape() {
    let messages = vec![tool_message(vec![text_block(9_000)])];
    let (pruned_view, _) = prune_tool_results(&messages);
    assert!(replay_prune(&pruned_view[0]).is_none(), "已裁形态重放返回 None");
}

// ---- prune_view：组合入口 ----

#[test]
fn prune_view_combines_text_and_image_entries() {
    // 两条超阈文本 + 一条溢出图片：组合明细按 index 合并、price 以最终形态重估。
    let big_url = format!("data:image/png;base64,{}", "A".repeat(80_000));
    let messages = vec![
        tool_message(vec![text_block(9_000)]),
        tool_message(vec![ContentBlock::Image { image_url: big_url }]),
    ];
    let limit = IMAGE_TOKEN_ESTIMATE + 1; // 低限逼出 offload（2049 < 单图 2048 + 开销）
    let (view, entries) = prune_view(&messages, limit, 0);
    assert_eq!(view.len(), 2);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].index, 0);
    assert_eq!(entries[1].index, 1);
    // 文本条目 price 已被图片卸载后的最终形态覆盖（消息 0 无图片，price 保持裁剪后形态）。
    assert_eq!(entries[0].price, estimate_message(&view[0]));
    assert_eq!(entries[1].price, estimate_message(&view[1]));
    let parts = text_parts(&view[1]);
    assert_eq!(parts[0].0, IMAGE_OFFLOAD_PLACEHOLDER);
}

#[test]
fn prune_view_noop_when_under_limit() {
    let messages = vec![tool_message(vec![text_block(100)])];
    let (view, entries) = prune_view(&messages, 100_000, 0);
    assert!(entries.is_empty());
    assert_eq!(view, messages);
}

// ---- IMAGE_TOKEN_ESTIMATE / estimate_overhead ----

#[test]
fn image_estimate_is_fixed_post_budget() {
    // 预算管线（media 模块）保证入模图片 ≤1.15MP，估算按固定单图口径封顶，
    // 不再按 data URL 字符数折算（原始字节与网关实际计费无关）。
    assert_eq!(IMAGE_TOKEN_ESTIMATE, 2_048);
}

#[test]
fn estimate_overhead_sums_system_and_tools() {
    let system = "system prompt";
    let tools = vec![ToolDefinition {
        name: "echo".into(),
        description: "echo tool".into(),
        parameters: serde_json::json!({}),
    }];
    let overhead = estimate_overhead(Some(system), &tools);
    assert!(overhead > (system.chars().count() as u64) / 4);
    assert!(estimate_overhead(None, &[]) == 0);
}
