//! session 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）：`list_sessions`。

use super::*;
use crate::paths::{session_log_file, sessions_dir};
use crate::types::{AgentMessage, ContentBlock};

/// 临时会话目录（测试结束清理，不污染真实数据目录）。
fn temp_sessions_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ava_session_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("建临时会话目录");
    dir
}

/// 写一个会话日志（信封格式 `{"ts":..,"event":{..}}`，与回放路径同构）。
fn write_session(sessions: &std::path::Path, id: &str, events: Vec<serde_json::Value>) {
    let session = sessions.join(crate::paths::encode_session_id(id));
    std::fs::create_dir_all(&session).expect("建会话目录");
    let mut content = String::new();
    for (index, event) in events.into_iter().enumerate() {
        content.push_str(&serde_json::to_string(&serde_json::json!({
            "ts": 1_700_000_000_000u64 + index as u64,
            "event": event,
        })).expect("序列化信封"));
        content.push('\n');
    }
    std::fs::write(session_log_file(&session), content).expect("写会话日志");
}

/// 构造一条 user_message durable 事件（信封内的 event 体）。
fn user_message(text: &str) -> serde_json::Value {
    serde_json::to_value(SessionEvent::UserMessage {
        turn: 1,
        step: 1,
        message: AgentMessage::user(text),
    })
    .expect("序列化 UserMessage")
}

#[test]
fn unknown_event_without_ignorable_refuses_rebuild() {
    let dir = temp_sessions_dir("unknown_strict");
    write_session(
        &dir,
        "strict",
        vec![
            user_message("hi"),
            serde_json::json!({ "type": "future_event", "payload": 1 }),
        ],
    );
    let result = SessionLog::open_with_replay(&dir, "strict");
    assert!(result.is_err(), "未知事件未标 ignorable 时必须拒绝重建");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unknown_event_with_ignorable_is_skipped() {
    let dir = temp_sessions_dir("unknown_ignorable");
    write_session(
        &dir,
        "keeper",
        vec![
            user_message("hi"),
            serde_json::json!({ "type": "future_event", "ignorable": true, "payload": 1 }),
        ],
    );
    let log = SessionLog::open_with_replay(&dir, "keeper").expect("标了 ignorable 应可重建");
    let texts: Vec<String> = log
        .snapshot()
        .iter()
        .filter_map(|event| match event {
            SessionEvent::UserMessage { message, .. } => Some(message.text_content()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, vec!["hi".to_string()]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn known_event_with_shape_drift_is_skipped() {
    let dir = temp_sessions_dir("shape_drift");
    write_session(
        &dir,
        "drift",
        vec![
            user_message("hi"),
            // 已知类型但缺少必填字段（历史字段漂移）→ 跳过而非拒绝重建
            serde_json::json!({ "type": "turn_start" }),
        ],
    );
    let log = SessionLog::open_with_replay(&dir, "drift").expect("已知类型字段漂移应可重建");
    assert_eq!(log.snapshot().len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_dir_yields_empty_list() {
    let dir = std::env::temp_dir().join("ava_session_absent_never_created");
    assert_eq!(list_sessions(&dir), Vec::new());
}

#[test]
fn lists_sessions_with_id_event_count_and_title() {
    let dir = temp_sessions_dir("basic");
    write_session(&dir, "s-1", vec![user_message("修复登录崩溃"), user_message("再补个测试")]);

    let summaries = list_sessions(&dir);
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, "s-1");
    assert_eq!(summaries[0].events, 2);
    assert_eq!(summaries[0].title, "修复登录崩溃");
    assert_eq!(summaries[0].goal_phase, "none");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn injected_messages_are_skipped_when_picking_title() {
    let dir = temp_sessions_dir("inject");
    write_session(
        &dir,
        "s-2",
        vec![
            user_message("<system-reminder>runtime ctx</system-reminder>"),
            user_message("<environment>cwd</environment>"),
            user_message("Current runtime context. now"),
            user_message("真实提问"),
        ],
    );

    let summaries = list_sessions(&dir);
    // 注入消息都计入 events，但不作为标题。
    assert_eq!(summaries[0].events, 4);
    assert_eq!(summaries[0].title, "真实提问");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn long_title_is_truncated_with_ellipsis() {
    let dir = temp_sessions_dir("long");
    write_session(&dir, "s-3", vec![user_message(&"x".repeat(80))]);

    let summaries = list_sessions(&dir);
    assert_eq!(summaries[0].title.chars().count(), 49);
    assert!(summaries[0].title.ends_with('…'));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn corrupt_lines_are_skipped_without_hiding_the_session() {
    let dir = temp_sessions_dir("corrupt");
    let session = dir.join("s-4");
    std::fs::create_dir_all(&session).expect("建会话目录");
    let good = serde_json::to_string(&serde_json::json!({
        "ts": 1,
        "event": user_message("有效行"),
    }))
    .expect("序列化");
    std::fs::write(
        session_log_file(&session),
        format!("{good}\n{{ 不是合法 JSON\n也 不是一个事件\n"),
    )
    .expect("写日志");

    let summaries = list_sessions(&dir);
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].events, 1);
    assert_eq!(summaries[0].title, "有效行");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sessions_without_log_file_are_omitted() {
    let dir = temp_sessions_dir("nolog");
    // 只有目录、没有 session.v1.jsonl → 无 mtime 可取 → 跳过。
    std::fs::create_dir_all(dir.join("ghost")).expect("建空会话目录");
    write_session(&dir, "real", vec![user_message("x")]);

    let summaries = list_sessions(&dir);
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, "real");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn plain_files_in_sessions_dir_are_ignored() {
    let dir = temp_sessions_dir("plainfile");
    std::fs::write(dir.join("stray.txt"), "not a session").expect("写散落文件");
    write_session(&dir, "s-5", vec![user_message("y")]);

    let summaries = list_sessions(&dir);
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, "s-5");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn session_id_is_the_encoded_dir_name_and_paths_helper_resolves_it() {
    let dir = temp_sessions_dir("enc");
    write_session(&dir, "a.b/c", vec![user_message("z")]);

    let summaries = list_sessions(&dir);
    // 目录名是编码后的 id（含 `.` 与 `/` 被替换）。
    assert_eq!(summaries[0].id, "a_b_c");
    assert!(session_log_file(&sessions_dir(&dir, &dir).join("nope")).ends_with("session.v1.jsonl"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn title_uses_first_real_user_message_only() {
    let dir = temp_sessions_dir("firstmsg");
    // 非文本块不参与标题拼接。
    let mut message = AgentMessage::user("标题");
    message.blocks = vec![
        ContentBlock::Text { text: "标题".to_string() },
        ContentBlock::Image { image_url: "data:image/png;base64,AA".to_string() },
    ];
    let event = serde_json::to_value(SessionEvent::UserMessage { turn: 1, step: 1, message })
        .expect("序列化");
    write_session(&dir, "s-6", vec![event]);

    let summaries = list_sessions(&dir);
    assert_eq!(summaries[0].title, "标题");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn explicit_session_title_overrides_first_user_message() {
    let dir = temp_sessions_dir("explicit");
    let title = |text: &str| {
        serde_json::to_value(SessionEvent::SessionTitle { title: text.to_string() })
            .expect("序列化 SessionTitle")
    };
    // 多次命名：最后一条非空生效；空白标题被忽略（仍沿用前一条）。
    write_session(
        &dir,
        "s-7",
        vec![user_message("帮我说明一下 Rust 中 Arc 和 Rc 的区别"), title("Arc 与 Rc"), title("  ")],
    );

    let summaries = list_sessions(&dir);
    assert_eq!(summaries[0].title, "Arc 与 Rc");

    let _ = std::fs::remove_dir_all(&dir);
}

// ========== #30：surface 事件白名单 ==========

/// 只有 4 类消息事件是 surface 事件；其余（含压缩摘要、命令、审计）都不是。
#[test]
fn surface_whitelist_matches_reference() {
    use crate::types::{AgentMessage, ToolResult, Usage};

    let surface = [
        SessionEvent::SystemMessage { turn: 1, step: 1, message: "sys".to_string() },
        SessionEvent::UserMessage {
            turn: 1,
            step: 1,
            message: AgentMessage::user("hi".to_string()),
        },
        SessionEvent::AssistantMessage {
            turn: 1,
            step: 1,
            message: AgentMessage::assistant(vec![]),
            usage: Usage::default(),
        },
        SessionEvent::ToolResult {
            turn: 1,
            step: 1,
            result: ToolResult {
                tool_call_id: "c1".to_string(),
                name: "read".to_string(),
                content: vec![],
                is_error: false,
            },
        },
    ];
    for event in &surface {
        assert!(event.is_surface(), "应为 surface 事件：{event:?}");
    }

    // 压缩摘要改写已有区间，不新增 surface 节点（等价 surfaceOp: replace 动作）。
    let non_surface = [
        SessionEvent::TurnStart { turn: 1 },
        SessionEvent::StepStart { turn: 1, step: 1 },
        SessionEvent::CompactionSummary {
            turn: 1,
            compaction_id: "cmp-1".to_string(),
            summary: "s".to_string(),
            shadowed_count: 1,
            shadowed_tokens: 10,
            usage: Usage::default(),
        },
        SessionEvent::CommandRun {
            command_id: "cmd-1".to_string(),
            name: "compact".to_string(),
            args: None,
        },
        SessionEvent::ApprovalAudited {
            tool: "write".to_string(),
            decision: "allowed-once".to_string(),
            via: "sandbox".to_string(),
        },
        SessionEvent::FeedbackRecord { text: Some("x".to_string()), category: None },
    ];
    for event in &non_surface {
        assert!(!event.is_surface(), "不应为 surface 事件：{event:?}");
    }
}

// ========== #11：崩溃恢复与写完整性（torn-tail / 写租约） ==========

/// 在既有日志末尾追加崩溃撕裂尾（半行 JSON，无换行）。
fn append_torn_fragment(path: &std::path::Path, marker: &str) {
    let mut content = std::fs::read_to_string(path).expect("读日志");
    content.push_str(&format!("{{\"ts\":{marker},\"event\":{{\"typ"));
    std::fs::write(path, content).expect("写撕裂尾");
}

#[test]
fn torn_tail_is_truncated_on_next_append() {
    let dir = temp_sessions_dir("torn_tail");
    write_session(&dir, "t1", vec![user_message("第一"), user_message("第二")]);
    let path = session_log_file(&dir.join(crate::paths::encode_session_id("t1")));
    append_torn_fragment(&path, "99999999");

    let log = SessionLog::open_with_replay(&dir, "t1").expect("打开会话");
    assert_eq!(log.snapshot().len(), 2, "撕裂尾不进入内存投影");
    log.append(SessionEvent::UserMessage {
        turn: 2,
        step: 1,
        message: AgentMessage::user("第三"),
    });
    log.close();
    drop(log);

    let after = std::fs::read_to_string(&path).expect("读回");
    assert!(!after.contains("99999999"), "撕裂尾必须被首个追加动作截断");
    assert_eq!(after.lines().count(), 3, "两条旧事件 + 一条新事件");
    let replayed = SessionLog::open_with_replay(&dir, "t1").expect("重开回放");
    assert_eq!(replayed.snapshot().len(), 3, "重开应完整回放");
    drop(replayed);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn corrupt_final_line_is_healed_on_append() {
    let dir = temp_sessions_dir("heal");
    write_session(&dir, "h1", vec![user_message("好的")]);
    let path = session_log_file(&dir.join(crate::paths::encode_session_id("h1")));
    let mut content = std::fs::read_to_string(&path).expect("读日志");
    content.push_str("{ 不是合法 JSON\n");
    std::fs::write(&path, content).expect("写末尾坏行");

    let log = SessionLog::open_with_replay(&dir, "h1").expect("打开会话");
    assert_eq!(log.snapshot().len(), 1, "坏行不进入投影");
    log.append(SessionEvent::UserMessage {
        turn: 1,
        step: 2,
        message: AgentMessage::user("新"),
    });
    log.close();
    drop(log);

    let after = std::fs::read_to_string(&path).expect("读回");
    assert!(!after.contains("不是合法 JSON"), "末尾损坏行应被截断修复");
    assert_eq!(after.lines().count(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mid_file_corrupt_line_is_retained_but_skipped() {
    let dir = temp_sessions_dir("mid_corrupt");
    let session = dir.join(crate::paths::encode_session_id("m1"));
    std::fs::create_dir_all(&session).expect("建会话目录");
    let good = |text: &str| {
        serde_json::to_string(&serde_json::json!({ "ts": 1, "event": user_message(text) }))
            .expect("序列化")
    };
    std::fs::write(
        session_log_file(&session),
        format!("{}\n{{ 中段坏行\n{}\n", good("前"), good("后")),
    )
    .expect("写日志");

    let log = SessionLog::open_with_replay(&dir, "m1").expect("打开会话");
    assert_eq!(log.snapshot().len(), 2, "中段坏行被跳过，前后事件均回放");
    log.append(SessionEvent::UserMessage {
        turn: 1,
        step: 3,
        message: AgentMessage::user("新"),
    });
    log.close();
    drop(log);

    let after = std::fs::read_to_string(session_log_file(&session)).expect("读回");
    assert!(after.contains("中段坏行"), "中段坏行位于已提交区间内，保留不截断");
    assert_eq!(after.lines().count(), 4, "两旧 + 一坏 + 一新");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn replay_only_reads_without_repairing_or_taking_lease() {
    let dir = temp_sessions_dir("replay_only");
    write_session(&dir, "r1", vec![user_message("仅有")]);
    let path = session_log_file(&dir.join(crate::paths::encode_session_id("r1")));
    append_torn_fragment(&path, "99999999");

    // 写者先持有租约，只读回放不受阻（非写者不取租约）。
    let writer = SessionLog::open_with_replay(&dir, "r1").expect("写者打开");
    let log = SessionLog::replay_only(&dir, "r1").expect("只读回放");
    assert_eq!(log.snapshot().len(), 1, "撕裂尾不进入只读投影");
    drop(log);
    writer.close();
    drop(writer);

    let after = std::fs::read_to_string(&path).expect("读回");
    assert!(after.contains("99999999"), "只读回放不得改写落盘");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_lease_blocks_second_writer_until_close() {
    let dir = temp_sessions_dir("lease");
    let first = SessionLog::open_with_replay(&dir, "writer-a").expect("首个写者");
    let other = SessionLog::open_with_replay(&dir, "writer-b").expect("不同目录不受影响");

    let err = SessionLog::open_with_replay(&dir, "writer-a").expect_err("同 id 第二写者应被拒绝");
    assert!(err.to_string().contains("写租约"), "错误应说明租约冲突：{err}");

    first.close();
    drop(first);
    SessionLog::open_with_replay(&dir, "writer-a").expect("close 释放租约后可重开");
    other.close();
    drop(other);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn fresh_session_ids_do_not_collide_within_same_millisecond() {
    let first = crate::session::fresh_session_id();
    let second = crate::session::fresh_session_id();
    assert_ne!(first, second, "同毫秒连续生成必须不同");
    assert!(first.starts_with("onion-"));
}
