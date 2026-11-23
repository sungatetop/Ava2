//! region 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use crate::compaction::config::CompactSpec;
use crate::compaction::region::{
    assert_summary_shrinks, build_summarization_input, cut_balanced_before, select_compactable_range,
};
use crate::types::{AgentMessage, ContentBlock};

fn spec(retain_tokens: u64) -> CompactSpec {
    CompactSpec {
        context_window_tokens: 128_000,
        threshold_tokens: 60_416,
        retain_tokens,
        compact_max_retries: 1,
        max_overflow_retries: 1,
    }
}

fn user(text: &str) -> AgentMessage {
    AgentMessage::user(text)
}

fn tool_use(id: &str) -> AgentMessage {
    AgentMessage::assistant(vec![ContentBlock::ToolUse {
        id: id.to_string(),
        name: "echo".to_string(),
        input: serde_json::json!({}),
    }])
}

fn tool_result(id: &str) -> AgentMessage {
    AgentMessage::tool_result(&crate::types::ToolResult {
        tool_call_id: id.to_string(),
        name: "echo".to_string(),
        is_error: false,
        content: vec![ContentBlock::Text { text: "ok".to_string() }],
    })
}

#[test]
fn empty_messages_select_nothing() {
    assert!(select_compactable_range(&[], &spec(1)).is_none());
}

#[test]
fn all_messages_within_retention_select_nothing() {
    let messages = vec![user("hello"), user("world")];
    // retain 20 覆盖两条消息估算 → keep_from 归 0 → None。
    assert!(select_compactable_range(&messages, &spec(20)).is_none());
}

#[test]
fn retain_tail_wins_and_range_starts_at_zero() {
    // user("aa..32 chars")≈12 tok；user("bb")≈5 tok；retain=5 → 尾部累积到 idx1 即达
    // 5 ≥ 5 → 只保尾 1 条 → 区间 [0, 1) → count = 1。
    let messages = vec![user("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"), user("bb")];
    let range = select_compactable_range(&messages, &spec(5)).expect("range");
    assert_eq!(range.start, 0);
    assert_eq!(range.count, 1);
}

#[test]
fn unbalanced_cut_falls_back_to_pairing_boundary() {
    // msgs: user(0) assistant_tool_use(1) tool_result(2)。
    // retain=5 → 尾部累积含 idx2（result）→ keep_from=2；切在 2 前不平衡
    // （use 无 result 未平衡？前缀 [0,1) 只含 user → 平衡），实际检查 keep_from=2
    // 切点在 2 之前：前缀 [user, tool_use] → use 1 result 0 → 不平衡 → 退到 1；
    // 1 之前前缀 [user] → 平衡 → 落 1。
    let messages = vec![user("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"), tool_use("c1"), tool_result("c1")];
    let range = select_compactable_range(&messages, &spec(5)).expect("range");
    assert_eq!(range.count, 1);
}

#[test]
fn cut_balanced_before_boundary_cases() {
    let messages = vec![tool_use("a"), tool_result("a"), tool_use("b")];
    assert!(cut_balanced_before(&messages, 2)); // use/result 成对 → 平衡
    assert!(!cut_balanced_before(&messages, 3)); // 尾部悬空 use → 不平衡
}

#[test]
fn summarization_input_appends_instruction() {
    let messages = vec![user("hello"), user("world")];
    let input = build_summarization_input(&messages, crate::compaction::region::ShadowedRange { start: 0, count: 2 });
    assert_eq!(input.len(), 3);
    assert_eq!(input[2].text_content(), crate::compaction::summarizer::COMPACTION_INSTRUCTION);
}

#[test]
fn shrink_check_rejects_equal_or_larger() {
    assert!(assert_summary_shrinks(100, 100).is_err());
    assert!(assert_summary_shrinks(101, 100).is_err());
    assert!(assert_summary_shrinks(99, 100).is_ok());
}
