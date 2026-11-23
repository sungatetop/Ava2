//! `session_query::tools` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::path::PathBuf;

use serde_json::json;

use crate::session::SessionEvent;
use crate::session_query::filters::{decode_cursor, CURSOR_VERSION, SearchCursor};
use crate::session_query::tools::{
    CallerContext, SearchFilter, execute_event_read, execute_event_search, execute_event_trace,
    execute_session_search, execute_session_trace, normalize_query, target_id,
};
use crate::session_query::{EventRecord, read_session_events};
use crate::types::ToolCall;

fn caller(id: &str, recorded_events: u64) -> CallerContext {
    CallerContext { id: id.to_string(), recorded_events }
}

fn call(input: serde_json::Value) -> ToolCall {
    ToolCall { id: "c1".into(), name: "t".into(), input }
}

fn write_session(dir: &std::path::Path, id: &str, events: &[&str]) -> PathBuf {
    let session_dir = crate::paths::session_dir(dir, id);
    std::fs::create_dir_all(&session_dir).unwrap();
    let file = crate::paths::session_log_file(&session_dir);
    let mut content = String::new();
    for (i, text) in events.iter().enumerate() {
        let event = SessionEvent::SystemMessage { turn: 0, step: 0, message: text.to_string() };
        content.push_str(&format!(
            "{{\"ts\":{}, \"event\":{}}}\n",
            (i as u64 + 1) * 1000,
            serde_json::to_string(&event).unwrap()
        ));
    }
    std::fs::write(&file, content).unwrap();
    file
}

/// 带 `SessionStarted` 的会话（用于 created_at 过滤测试）：先写 SessionStarted
/// （ts=`started_ts`），再逐条 SystemMessage（ts 依次 +1s）。
fn write_session_started(dir: &std::path::Path, id: &str, started_ts: u64, messages: &[&str]) -> PathBuf {
    let session_dir = crate::paths::session_dir(dir, id);
    std::fs::create_dir_all(&session_dir).unwrap();
    let file = crate::paths::session_log_file(&session_dir);
    let mut content = String::new();
    let started = SessionEvent::SessionStarted {
        session_id: id.to_string(),
        model: "test-model".to_string(),
    };
    content.push_str(&format!(
        "{{\"ts\":{}, \"event\":{}}}\n",
        started_ts,
        serde_json::to_string(&started).unwrap()
    ));
    for (i, text) in messages.iter().enumerate() {
        let event = SessionEvent::SystemMessage { turn: 0, step: 0, message: text.to_string() };
        content.push_str(&format!(
            "{{\"ts\":{}, \"event\":{}}}\n",
            started_ts + (i as u64 + 1) * 1000,
            serde_json::to_string(&event).unwrap()
        ));
    }
    std::fs::write(&file, content).unwrap();
    file
}

/// 从结果页提取 `Next cursor:` 令牌并解码为 `SearchCursor`。
fn extract_cursor(page: &str) -> SearchCursor {
    let token = page.split("Next cursor: ").nth(1).unwrap().trim().to_string();
    decode_cursor(&token).unwrap()
}

#[test]
fn normalize_query_trims_and_rejects_empty() {
    assert_eq!(normalize_query("  a   b  ").unwrap(), "a b");
    assert!(normalize_query("   ").is_err());
    assert!(normalize_query("\0").is_err());
}

#[test]
fn target_id_defaults_to_caller() {
    let c = caller("caller-1", 0);
    assert_eq!(target_id(&call(json!({})), &c), "caller-1");
    assert_eq!(target_id(&call(json!({ "session_id": "other" })), &c), "other");
}

#[test]
fn session_search_excludes_caller_and_finds_others() {
    let dir = std::env::temp_dir().join(format!("onion_sq_search_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    write_session(&dir, "caller-id", &["hello alpha", "world"]);
    write_session(&dir, "other-1", &["alpha thing here", "x"]);
    write_session(&dir, "other-2", &["no match"]);
    let c = caller("caller-id", 2);
    let text = execute_session_search(&dir, &c, "alpha", 100, &SearchFilter::default(), None).unwrap();
    // 排除调用会话：只有 other-1 命中。
    assert!(text.contains("Session search results (1):"));
    assert!(text.contains("other-1"));
    assert!(!text.contains("caller-id"));
    assert!(text.contains("alpha"));
    assert!(text.contains("matches 1"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn session_search_no_match() {
    let dir = std::env::temp_dir().join(format!("onion_sq_nomatch_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    write_session(&dir, "other-1", &["nothing here"]);
    let c = caller("caller-id", 0);
    assert_eq!(
        execute_session_search(&dir, &c, "zzz", 100, &SearchFilter::default(), None).unwrap(),
        "No prior session matches found."
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn session_search_caps_results() {
    let dir = std::env::temp_dir().join(format!("onion_sq_cap_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for i in 0..3 {
        write_session(&dir, &format!("s{i}"), &[&format!("match {i}")]);
    }
    let c = caller("caller-id", 0);
    let text = execute_session_search(&dir, &c, "match", 2, &SearchFilter::default(), None).unwrap();
    assert!(text.contains("Session search results (2):"));
    assert!(text.contains("Result cap reached"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn session_search_ranks_by_match_count() {
    let dir = std::env::temp_dir().join(format!("onion_sq_rank_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    write_session(&dir, "weak-1", &["alpha tail"]);
    write_session(&dir, "strong-1", &["alpha alpha alpha tail"]);
    let c = caller("caller-id", 0);
    let text = execute_session_search(&dir, &c, "alpha", 100, &SearchFilter::default(), None).unwrap();
    // 每会话最强命中按 match_count 降序：strong-1（3 次）排在 weak-1（1 次）前。
    let strong_pos = text.find("strong-1").unwrap();
    let weak_pos = text.find("weak-1").unwrap();
    assert!(strong_pos < weak_pos);
    assert!(text.contains("matches 3"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn session_search_paginates_with_cursor() {
    let dir = std::env::temp_dir().join(format!("onion_sq_cursor_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for i in 0..3 {
        write_session(&dir, &format!("s{i}"), &[&format!("hit {i}")]);
    }
    let c = caller("caller-id", 0);
    // 页 1：2 条命中 + 游标。
    let page1 = execute_session_search(&dir, &c, "hit", 2, &SearchFilter::default(), None).unwrap();
    assert!(page1.contains("Session search results (2):"));
    assert!(page1.contains("Next cursor: "));
    let cursor = extract_cursor(&page1);
    assert_eq!(cursor.o, 2);
    // 页 2：剩余 1 条，无后续游标。
    let page2 = execute_session_search(&dir, &c, "hit", 2, &SearchFilter::default(), Some(&cursor)).unwrap();
    assert!(page2.contains("Session search results (1):"));
    assert!(!page2.contains("Next cursor: "));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn session_search_created_at_bounds() {
    let dir = std::env::temp_dir().join(format!("onion_sq_created_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    write_session_started(&dir, "old", 1000, &["alpha old"]);
    write_session_started(&dir, "new", 9_000_000, &["alpha new"]);
    let c = caller("caller-id", 0);
    // created_at 下界：只保留 new。
    let filter_from = SearchFilter { created_at_from: Some(5_000_000), ..Default::default() };
    let text = execute_session_search(&dir, &c, "alpha", 100, &filter_from, None).unwrap();
    assert!(text.contains("new"));
    assert!(!text.contains("old"));
    // created_at 上界：只保留 old。
    let filter_to = SearchFilter { created_at_to: Some(5_000), ..Default::default() };
    let text = execute_session_search(&dir, &c, "alpha", 100, &filter_to, None).unwrap();
    assert!(text.contains("old"));
    assert!(!text.contains("new"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cursor_rejects_mismatched_fingerprint_and_stale_corpus() {
    let dir = std::env::temp_dir().join(format!("onion_sq_stale_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for i in 0..2 {
        write_session(&dir, &format!("s{i}"), &[&format!("match {i}")]);
    }
    let c = caller("caller-id", 0);
    // 手造指纹不符 → Invalid cursor。
    let bogus = SearchCursor { v: CURSOR_VERSION, f: "x".into(), g: "y".into(), o: 0 };
    let err = execute_session_search(&dir, &c, "match", 1, &SearchFilter::default(), Some(&bogus)).unwrap_err();
    assert!(err.contains("Invalid cursor"));
    // 正确页 1 → 语料追加新会话 → 代际不符 → Stale cursor。
    let page1 = execute_session_search(&dir, &c, "match", 1, &SearchFilter::default(), None).unwrap();
    let cursor = extract_cursor(&page1);
    write_session(&dir, "s-new", &["match added"]);
    let err = execute_session_search(&dir, &c, "match", 1, &SearchFilter::default(), Some(&cursor)).unwrap_err();
    assert!(err.contains("Stale cursor"));
    // 查询参数变化（query 不同）→ Invalid cursor。
    let err = execute_session_search(&dir, &c, "other", 1, &SearchFilter::default(), Some(&cursor)).unwrap_err();
    assert!(err.contains("Invalid cursor"));
    // 坏 token（非 base64url）。
    assert!(decode_cursor("!!!not-base64!!!").is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn event_search_current_session_stops_before_step() {
    let dir = std::env::temp_dir().join(format!("onion_sq_evsearch_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    write_session(&dir, "cur", &["alpha 1", "beta", "alpha 3"]);
    // 调用时已记录 2 个事件：只应命中 seq <= 2 的 alpha。
    let c = caller("cur", 2);
    let text = execute_event_search(&dir, &c, &call(json!({ "query": "alpha" })), "alpha", 100, &SearchFilter::default(), None).unwrap();
    assert!(text.contains("event search results (1):"));
    assert!(text.contains("seq 1"));
    assert!(!text.contains("seq 3"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn event_search_other_session_sees_all() {
    let dir = std::env::temp_dir().join(format!("onion_sq_evsearch2_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    write_session(&dir, "other", &["alpha 1", "beta", "alpha 3"]);
    let c = caller("cur", 0);
    let text = execute_event_search(&dir, &c, &call(json!({ "session_id": "other", "query": "alpha" })), "alpha", 100, &SearchFilter::default(), None).unwrap();
    assert!(text.contains("event search results (2):"));
    assert!(text.contains("seq 1"));
    assert!(text.contains("seq 3"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn event_search_paginates_with_cursor() {
    let dir = std::env::temp_dir().join(format!("onion_sq_evcursor_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    write_session(&dir, "other", &["alpha", "alpha alpha", "alpha alpha alpha", "alpha x"]);
    let c = caller("cur", 0);
    // 页 1：最强两个命中（count 3 / 2）+ 游标。
    let page1 = execute_event_search(&dir, &c, &call(json!({ "session_id": "other", "query": "alpha" })), "alpha", 2, &SearchFilter::default(), None).unwrap();
    assert!(page1.contains("event search results (2):"));
    assert!(page1.contains("matches 3"));
    assert!(page1.contains("Next cursor: "));
    let cursor = extract_cursor(&page1);
    assert_eq!(cursor.o, 2);
    // 页 2：剩余两个 count=1 命中，无后续游标。
    let page2 = execute_event_search(&dir, &c, &call(json!({ "session_id": "other", "query": "alpha" })), "alpha", 2, &SearchFilter::default(), Some(&cursor)).unwrap();
    assert!(page2.contains("event search results (2):"));
    assert!(!page2.contains("Next cursor: "));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn session_trace_summarizes() {
    let dir = std::env::temp_dir().join(format!("onion_sq_trace_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    write_session(&dir, "cur", &["a", "b"]);
    let c = caller("cur", 2);
    let text = execute_session_trace(&dir, &c, &call(json!({}))).unwrap();
    assert!(text.contains("Session cur"));
    assert!(text.contains("Events: 2"));
    assert!(text.contains("Ancestors: none"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn event_trace_reports_target_and_none_relations() {
    let dir = std::env::temp_dir().join(format!("onion_sq_evtrace_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    write_session(&dir, "cur", &["a", "b"]);
    let c = caller("cur", 2);
    let text = execute_event_trace(&dir, &c, &call(json!({ "seq": 2 }))).unwrap();
    assert!(text.contains("Target: seq 2"));
    assert!(text.contains("Replaced by: none"));
    assert!(text.contains("Direct derived events: none"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn event_trace_missing_seq_errors() {
    let dir = std::env::temp_dir().join(format!("onion_sq_evtrace2_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    write_session(&dir, "cur", &["a"]);
    let c = caller("cur", 1);
    assert!(execute_event_trace(&dir, &c, &call(json!({ "seq": 99 }))).is_err());
    assert!(execute_event_trace(&dir, &c, &call(json!({}))).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn event_read_emits_json_and_neighbors() {
    let dir = std::env::temp_dir().join(format!("onion_sq_evread_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    write_session(&dir, "cur", &["first", "second", "third"]);
    let c = caller("cur", 3);
    let text = execute_event_read(&dir, &c, &call(json!({ "seq": 2, "before": 1, "after": 1 }))).unwrap();
    assert!(text.contains("Target event seq 2:"));
    assert!(text.contains("```json"));
    assert!(text.contains("second"));
    assert!(text.contains("Before:"));
    assert!(text.contains("first"));
    assert!(text.contains("After:"));
    assert!(text.contains("third"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn event_read_no_neighbors_when_omitted() {
    let dir = std::env::temp_dir().join(format!("onion_sq_evread2_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    write_session(&dir, "cur", &["first", "second"]);
    let c = caller("cur", 2);
    let text = execute_event_read(&dir, &c, &call(json!({ "seq": 1 }))).unwrap();
    assert!(!text.contains("Before:"));
    assert!(!text.contains("After:"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn read_session_events_used_by_tools() {
    let dir = std::env::temp_dir().join(format!("onion_sq_read_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    write_session(&dir, "cur", &["a", "b"]);
    let session_dir = crate::paths::session_dir(&dir, "cur");
    let file = crate::paths::session_log_file(&session_dir);
    let records: Vec<EventRecord> = read_session_events(&file).unwrap();
    assert_eq!(records.len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

/// 子代理会话与父会话并列（`<root>/logs/subagents/<id>/`，不按 cwd 分区）：
/// 按 id 读取必须回落该根，否则读面报「not readable」，模型会当成会话不存在
/// 而反复换关键词重试（每次重试都是一整轮模型往返）。
#[test]
fn load_session_falls_back_to_subagent_log_root() {
    let root = std::env::temp_dir().join(format!("onion_sq_subagent_{}", std::process::id()));
    let sessions = root.join("sessions").join("C--ws");
    let subagents = crate::paths::subagent_logs_dir(&root);
    std::fs::create_dir_all(&sessions).unwrap();
    std::fs::create_dir_all(&subagents).unwrap();
    write_session(&subagents, "onion-child-1", &["child says ok"]);

    let records = super::load_session_at(&sessions, &subagents, "onion-child-1").unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(crate::session_query::event_type(&records[0].event), "system_message");

    // 两处都不存在时仍报错（不能静默返回空，否则「读不到」与「没有事件」混淆）。
    assert!(super::load_session_at(&sessions, &subagents, "onion-missing").is_err());
    let _ = std::fs::remove_dir_all(&root);
}
