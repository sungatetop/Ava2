//! status 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;
use onion::session::SessionEvent;
use onion::types::{AgentMessage, ContentBlock, ToolCall, ToolResult, Usage};

/// 构造含完整生命周期的事件序列。
fn sample_events() -> Vec<SessionEvent> {
    let call = ToolCall {
        id: "c1".to_string(),
        name: "read_file".to_string(),
        input: serde_json::json!({"path": "a.txt"}),
    };
    let result = ToolResult::text(&call, "hello");
    vec![
        SessionEvent::SessionStarted {
            session_id: "s-abc".to_string(),
            model: "test-model".to_string(),
        },
        SessionEvent::TurnStart { turn: 1 },
        SessionEvent::StepStart { turn: 1, step: 1 },
        SessionEvent::AssistantMessage {
            turn: 1,
            step: 1,
            message: AgentMessage::assistant(vec![ContentBlock::Text {
                text: "working".to_string(),
            }]),
            usage: Usage {
                input_tokens: 10,
                output_tokens: 5,
                cache_read_tokens: 20,
                cache_write_tokens: 2,
            },
        },
        SessionEvent::ToolCall {
            turn: 1,
            step: 1,
            call,
        },
        SessionEvent::ToolResult {
            turn: 1,
            step: 1,
            result,
        },
        SessionEvent::StepEnd { turn: 1, step: 1 },
        SessionEvent::TurnEnd { turn: 1, reason: "end_turn".to_string() },
        SessionEvent::CompactionStart {
            turn: 1,
            compaction_id: "k1".to_string(),
            trigger: "manual".to_string(),
        },
        SessionEvent::CompactionEnd {
            turn: 1,
            compaction_id: "k1".to_string(),
            error: None,
        },
    ]
}

#[test]
fn from_events_counts_lifecycle_and_usage() {
    let events = sample_events();
    let snap = StatusSnapshot::from_events(&events, 128_000);
    assert_eq!(snap.session_id, "s-abc");
    assert_eq!(snap.model, "test-model");
    assert_eq!(snap.turns, 1);
    assert_eq!(snap.steps, 1);
    assert_eq!(snap.tool_calls, 1);
    assert_eq!(snap.compactions, 1);
    assert_eq!(snap.usage.input_tokens, 10);
    assert_eq!(snap.usage.output_tokens, 5);
    assert_eq!(snap.usage.cache_read_tokens, 20);
    assert_eq!(snap.usage.cache_write_tokens, 2);
    assert_eq!(snap.context_window_tokens, 128_000);
}

#[test]
fn kv_hit_rate_shows_dash_when_no_denominator() {
    let snap = StatusSnapshot::from_events(&[], 128_000);
    let lines = snap.render();
    let hit = lines.iter().find(|l| l.contains("KV")).expect("KV line");
    assert!(hit.contains("—"), "expected dash: {hit}");
}

#[test]
fn kv_hit_rate_computed_from_cache_read_over_denominator() {
    let snap = StatusSnapshot::from_events(&sample_events(), 128_000);
    let lines = snap.render();
    let hit = lines.iter().find(|l| l.contains("KV")).expect("KV line");
    // cache_read 20 / (input 10 + cache_read 20) = 66.7%
    assert!(hit.contains("66.7%"), "expected 66.7%: {hit}");
}

#[test]
fn render_includes_context_ratio_line() {
    let snap = StatusSnapshot::from_events(&sample_events(), 128_000);
    let lines = snap.render();
    let ctx = lines.iter().find(|l| l.contains("上下文")).expect("context line");
    assert!(ctx.contains("128000"), "missing budget: {ctx}");
}

#[test]
fn estimate_events_counts_message_text_and_tool_payloads() {
    let estimated = estimate_events(&sample_events());
    // SystemMessage 无；UserMessage 无；AssistantMessage "working" 7 chars；
    // ToolCall "c1" + "read_file" + input json；ToolResult "hello" 5 chars。
    // 全部 4 chars/token。粗验 > 0 即可（精确值随 json 序列化格式浮动）。
    assert!(estimated > 0);
}

#[test]
fn estimate_events_counts_system_and_user_messages() {
    let events = vec![
        SessionEvent::SystemMessage {
            turn: 1,
            step: 0,
            message: "12345678".to_string(), // 8 chars
        },
        SessionEvent::UserMessage {
            turn: 1,
            step: 1,
            message: AgentMessage::user("1234"), // 4 chars
        },
    ];
    assert_eq!(estimate_events(&events), 3); // 12 chars / 4 = 3
}
