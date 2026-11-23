//! session_fork 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::{build_fork_seed, write_forked_log, ForkError, FORKED_TURN_REASON};
use crate::session::SessionEvent;
use crate::session_query::{read_session_events, EventRecord};
use crate::types::{ToolCall, ToolResult};

fn record(seq: u64, ts: u64, event: SessionEvent) -> EventRecord {
    EventRecord { seq, ts, event }
}

fn call(id: &str) -> ToolCall {
    ToolCall { id: id.to_string(), name: "demo".to_string(), input: serde_json::json!({}) }
}

/// 构造一段已闭合的单轮会话（TurnStart → … → TurnEnd）。
fn closed_turn_records(turn: u32, ts: u64) -> Vec<EventRecord> {
    let demo = call("call-a");
    vec![
        record(1, ts, SessionEvent::TurnStart { turn }),
        record(2, ts, SessionEvent::StepStart { turn, step: 1 }),
        record(3, ts, SessionEvent::ToolCall { turn, step: 1, call: demo.clone() }),
        record(4, ts, SessionEvent::ToolResult { turn, step: 1, result: ToolResult::text(&demo, "ok") }),
        record(5, ts, SessionEvent::StepEnd { turn, step: 1 }),
        record(6, ts, SessionEvent::TurnEnd { turn, reason: "complete".to_string() }),
    ]
}

#[test]
fn empty_source_without_boundary_yields_empty_child() {
    let forked = build_fork_seed(&[], None, "parent-1").unwrap();
    assert_eq!(forked.records, Vec::new());
    assert_eq!(forked.inherited_events, 0);
}

#[test]
fn empty_source_with_explicit_boundary_rejected() {
    assert_eq!(
        build_fork_seed(&[], Some(1), "parent-1"),
        Err(ForkError::InvalidBoundary { boundary: 1, last_seq: None })
    );
}

#[test]
fn boundary_defaults_to_last_seq() {
    let source = closed_turn_records(1, 1_000);
    let forked = build_fork_seed(&source, None, "parent-1").unwrap();
    // 前缀 6 条 + ForkSeed，全部已闭合 → 无合成关闭。
    assert_eq!(forked.inherited_events, 6);
    assert_eq!(forked.records.len(), 7);
    let seed = forked.records.last().unwrap();
    assert_eq!(
        seed.event,
        SessionEvent::ForkSeed {
            parent_session_id: "parent-1".to_string(),
            parent_boundary_seq: 6,
            inherited_events: 6,
        }
    );
    assert_eq!(seed.ts, 1_000);
}

#[test]
fn explicit_boundary_out_of_range_rejected() {
    let source = closed_turn_records(1, 1_000);
    assert_eq!(
        build_fork_seed(&source, Some(0), "parent-1"),
        Err(ForkError::InvalidBoundary { boundary: 0, last_seq: Some(6) })
    );
    assert_eq!(
        build_fork_seed(&source, Some(7), "parent-1"),
        Err(ForkError::InvalidBoundary { boundary: 7, last_seq: Some(6) })
    );
    assert_eq!(
        build_fork_seed(&source, Some(7), "parent-1").unwrap_err().to_string(),
        "fork boundary 7 out of range 1..=6"
    );
}

#[test]
fn prefix_copied_with_renumbered_seq_and_original_ts() {
    // 源 seq 含空洞（容错跳行形态）：1、2、4；边界 2 → 子会话复制两条并重排。
    let source = vec![
        record(1, 100, SessionEvent::TurnStart { turn: 1 }),
        record(2, 200, SessionEvent::StepStart { turn: 1, step: 1 }),
        record(4, 400, SessionEvent::StepEnd { turn: 1, step: 1 }),
    ];
    let forked = build_fork_seed(&source, Some(2), "parent-1").unwrap();
    assert_eq!(forked.inherited_events, 2);
    assert_eq!(forked.records[0].seq, 1);
    assert_eq!(forked.records[0].ts, 100);
    assert_eq!(forked.records[1].seq, 2);
    assert_eq!(forked.records[1].ts, 200);
    // 播种标记 seq 3，ts 沿用边界事件（seq 2，ts 200）。
    assert_eq!(forked.records[2].seq, 3);
    assert_eq!(forked.records[2].ts, 200);
    // 边界截在 step 内 → 补 StepEnd + TurnEnd（无未配对工具调用）。
    assert_eq!(forked.records.len(), 5);
    assert_eq!(forked.records[3].event, SessionEvent::StepEnd { turn: 1, step: 1 });
    assert_eq!(
        forked.records[4].event,
        SessionEvent::TurnEnd { turn: 1, reason: FORKED_TURN_REASON.to_string() }
    );
}

#[test]
fn unpaired_tool_call_gets_error_result_then_closers() {
    let demo_a = call("call-a");
    let demo_b = call("call-b");
    let source = vec![
        record(1, 100, SessionEvent::TurnStart { turn: 1 }),
        record(2, 100, SessionEvent::StepStart { turn: 1, step: 1 }),
        record(3, 100, SessionEvent::ToolCall { turn: 1, step: 1, call: demo_a.clone() }),
        record(4, 100, SessionEvent::ToolResult { turn: 1, step: 1, result: ToolResult::text(&demo_a, "ok") }),
        record(5, 100, SessionEvent::ToolCall { turn: 1, step: 1, call: demo_b.clone() }),
    ];
    let forked = build_fork_seed(&source, None, "parent-1").unwrap();
    assert_eq!(forked.inherited_events, 5);
    // 顺序：前缀 → ForkSeed → 合成关闭（call-b 错误结果 → StepEnd → TurnEnd）。
    assert_eq!(forked.records.len(), 9);
    assert_eq!(
        forked.records[5].event,
        SessionEvent::ForkSeed {
            parent_session_id: "parent-1".to_string(),
            parent_boundary_seq: 5,
            inherited_events: 5,
        }
    );
    assert_eq!(
        forked.records[6].event,
        SessionEvent::ToolResult {
            turn: 1,
            step: 1,
            result: ToolResult::error(&demo_b, "forked: tool did not complete"),
        }
    );
    assert_eq!(forked.records[7].event, SessionEvent::StepEnd { turn: 1, step: 1 });
    assert_eq!(
        forked.records[8].event,
        SessionEvent::TurnEnd { turn: 1, reason: FORKED_TURN_REASON.to_string() }
    );
}

#[test]
fn fully_closed_turns_produce_no_closers() {
    let source = closed_turn_records(1, 1_000);
    let forked = build_fork_seed(&source, None, "parent-1").unwrap();
    assert_eq!(forked.records.len(), source.len() + 1);
}

#[test]
fn second_open_turn_only_gets_its_own_closers() {
    let source = closed_turn_records(1, 1_000);
    let demo = call("call-c");
    let mut source = source;
    source.push(record(7, 2_000, SessionEvent::TurnStart { turn: 2 }));
    source.push(record(8, 2_000, SessionEvent::StepStart { turn: 2, step: 1 }));
    source.push(record(9, 2_000, SessionEvent::ToolCall { turn: 2, step: 1, call: demo.clone() }));
    let forked = build_fork_seed(&source, None, "parent-1").unwrap();
    assert_eq!(forked.inherited_events, 9);
    assert_eq!(forked.records.len(), 13);
    // ForkSeed 在 index 9；仅第二轮的开放状态被补关闭（第一轮已闭合，不受影响）。
    assert_eq!(
        forked.records[10].event,
        SessionEvent::ToolResult { turn: 2, step: 1, result: ToolResult::error(&demo, "forked: tool did not complete") }
    );
    assert_eq!(forked.records[11].event, SessionEvent::StepEnd { turn: 2, step: 1 });
    assert_eq!(
        forked.records[12].event,
        SessionEvent::TurnEnd { turn: 2, reason: FORKED_TURN_REASON.to_string() }
    );
}

#[test]
fn write_forked_log_roundtrips_seq_ts_and_events() {
    let root = std::env::temp_dir().join(format!("ava_session_fork_{}", std::process::id()));
    let child_dir = root.join("child-1");
    let _ = std::fs::remove_dir_all(&root);
    let source = closed_turn_records(1, 1_000);
    let forked = build_fork_seed(&source, None, "parent-1").unwrap();
    let write = write_forked_log(&child_dir, &forked.records);
    assert!(write.is_ok(), "write_forked_log failed: {write:?}");
    let log_file = crate::paths::session_log_file(&child_dir);
    let records = read_session_events(&log_file).unwrap();
    // seq 由 1-based 行号推导，应与构造时编号一致；ts/event 往返保持。
    assert_eq!(records.len(), forked.records.len());
    for (read, written) in records.iter().zip(forked.records.iter()) {
        assert_eq!(read.seq, written.seq);
        assert_eq!(read.ts, written.ts);
        assert_eq!(read.event, written.event);
    }
    let _ = std::fs::remove_dir_all(&root);
}
