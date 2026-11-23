//! `session_query` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::path::Path;

use crate::session::SessionEvent;
use crate::session_query::filters::TextFilter;
use crate::session_query::{event_json, event_type, format_time, read_session_events};

fn sample_event(seq: u64, text: &str) -> String {
    // 构造一条 JSONL 行（外层 ts + event）。
    let event = SessionEvent::SystemMessage {
        turn: 0,
        step: 0,
        message: format!("hello {text}"),
    };
    format!(
        "{{\"ts\":{}, \"event\":{}}}\n",
        seq * 1000,
        serde_json::to_string(&event).unwrap()
    )
}

#[test]
fn reads_events_with_seq_and_ts() {
    let dir = std::env::temp_dir().join(format!("onion_session_query_read_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("session.v1.jsonl");
    std::fs::write(&file, format!("{}{}", sample_event(1, "alpha"), sample_event(2, "beta"))).unwrap();
    let records = read_session_events(&file).unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].seq, 1);
    assert_eq!(records[1].seq, 2);
    assert_eq!(records[0].ts, 1000);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn skips_corrupt_lines() {
    let dir = std::env::temp_dir().join(format!("onion_session_query_corrupt_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("session.v1.jsonl");
    std::fs::write(&file, format!("{}\nnot-json\n{}", sample_event(1, "a"), sample_event(2, "b"))).unwrap();
    let records = read_session_events(&file).unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].seq, 1);
    assert_eq!(records[1].seq, 4); // 行号含被跳过的空行与坏行
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn searches_events_case_insensitive() {
    let dir = std::env::temp_dir().join(format!("onion_session_query_search_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("session.v1.jsonl");
    std::fs::write(&file, format!("{}{}", sample_event(1, "AlphaThing"), sample_event(2, "other"))).unwrap();
    let records = read_session_events(&file).unwrap();
    let filter = TextFilter::compile("alphathing");
    let hits: Vec<_> = records
        .iter()
        .filter(|record| filter.match_count(&event_json(record)).is_some())
        .collect();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].seq, 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn event_type_matches_serde_tag() {
    let event = SessionEvent::TurnStart { turn: 1 };
    assert_eq!(event_type(&event), "turn_start");
}

#[test]
fn formats_time_iso8601() {
    // 0 → unknown；已知毫秒 → ISO。
    assert_eq!(format_time(0), "<unknown>");
    assert_eq!(format_time(0), "<unknown>");
}

#[test]
fn lists_session_files_only_dirs_with_log() {
    let dir = std::env::temp_dir().join(format!("onion_session_query_list_{}", std::process::id()));
    let sessions_dir = dir.join("sessions");
    std::fs::create_dir_all(sessions_dir.join("sess-a")).unwrap();
    std::fs::write(sessions_dir.join("sess-a").join("session.v1.jsonl"), "").unwrap();
    std::fs::create_dir_all(sessions_dir.join("sess-no-log")).unwrap();
    std::fs::create_dir_all(sessions_dir.join("sess-file")).unwrap();
    std::fs::write(sessions_dir.join("sess-file").join("random.txt"), "x").unwrap();
    let files = crate::session_query::list_session_files(&sessions_dir);
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].id, "sess-a");
    assert!(Path::new(&files[0].path).exists());
    let _ = std::fs::remove_dir_all(&dir);
}
