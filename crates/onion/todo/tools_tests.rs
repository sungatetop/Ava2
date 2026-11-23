//! `todo/tools` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::TodoWriteTool;
use crate::session::{SessionEvent, SessionLog};
use crate::tools::ToolPort;
use crate::types::ToolCall;

fn tool(allow_parallel: bool) -> TodoWriteTool {
    TodoWriteTool::new(allow_parallel)
}

fn call(todos: serde_json::Value) -> ToolCall {
    ToolCall { id: "c1".to_string(), name: "todo_write".to_string(), input: json!({ "todos": todos }) }
}

fn text(blocks: &[crate::types::ContentBlock]) -> String {
    blocks
        .iter()
        .find_map(|b| match b {
            crate::types::ContentBlock::Text { text } => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

/// 提取 durable 事件中的 todo 清单（测试断言用）。
fn last_todo(events: &[SessionEvent]) -> Option<Vec<super::super::types::TodoItem>> {
    events.iter().rev().find_map(|e| match e {
        SessionEvent::TodoWrite { todos } => Some(todos.clone()),
        _ => None,
    })
}

#[tokio::test]
async fn todo_write_requires_owning_session() {
    let t = tool(false);
    let result = t
        .execute(
            call(json!([{ "content": "a", "status": "pending" }])),
            CancellationToken::new(),
        )
        .await;
    // 未接线 session → 拒绝（非 agent 调用方无 owning session）。
    let err = result.unwrap_err();
    assert!(err.contains("owning agent session"), "got: {err}");
}

#[tokio::test]
async fn todo_write_rejects_empty_content() {
    let t = tool(false);
    let session = SessionLog::in_memory();
    t.wire_session(std::sync::Arc::new(session));
    let result = t
        .execute(call(json!([{ "content": "  ", "status": "pending" }])), CancellationToken::new())
        .await;
    let err = result.unwrap_err();
    assert!(err.contains("non-empty"), "got: {err}");
}

#[tokio::test]
async fn todo_write_rejects_duplicate_content() {
    let t = tool(false);
    let session = SessionLog::in_memory();
    t.wire_session(std::sync::Arc::new(session));
    let result = t
        .execute(
            call(json!([
                { "content": "a", "status": "pending" },
                { "content": "a", "status": "pending" }
            ])),
            CancellationToken::new(),
        )
        .await;
    let err = result.unwrap_err();
    assert!(err.contains("duplicate"), "got: {err}");
}

#[tokio::test]
async fn todo_write_rejects_multiple_in_progress_when_single() {
    let t = tool(false);
    let session = SessionLog::in_memory();
    t.wire_session(std::sync::Arc::new(session));
    let result = t
        .execute(
            call(json!([
                { "content": "a", "status": "in_progress" },
                { "content": "b", "status": "in_progress" }
            ])),
            CancellationToken::new(),
        )
        .await;
    let err = result.unwrap_err();
    assert!(err.contains("at most one"), "got: {err}");
}

#[tokio::test]
async fn todo_write_allows_multiple_in_progress_when_parallel() {
    let t = tool(true);
    let session = SessionLog::in_memory();
    let session = std::sync::Arc::new(session);
    t.wire_session(session.clone());
    let result = t
        .execute(
            call(json!([
                { "content": "a", "status": "in_progress" },
                { "content": "b", "status": "in_progress" }
            ])),
            CancellationToken::new(),
        )
        .await;
    let result = result.unwrap();
    let text = text(&result);
    assert!(text.contains("\"inProgress\":2"), "got: {text}");
    let todos = last_todo(&session.snapshot()).unwrap();
    assert_eq!(todos.len(), 2);
    assert_eq!(todos[0].content, "a");
}

#[tokio::test]
async fn todo_write_appends_durable_event_and_returns_counts() {
    let t = tool(false);
    let session = SessionLog::in_memory();
    let session = std::sync::Arc::new(session);
    t.wire_session(session.clone());
    let result = t
        .execute(
            call(json!([
                { "content": "a", "status": "completed" },
                { "content": "b", "status": "in_progress" },
                { "content": "c", "status": "pending" }
            ])),
            CancellationToken::new(),
        )
        .await;
    let result = result.unwrap();
    let text = text(&result);
    assert!(text.contains("\"completed\":1"), "got: {text}");
    assert!(text.contains("\"inProgress\":1"), "got: {text}");
    assert!(text.contains("\"pending\":1"), "got: {text}");
    // 整表 last-write-wins：durable 事件含全部三项。
    let todos = last_todo(&session.snapshot()).unwrap();
    assert_eq!(todos.len(), 3);
    assert!(todos.iter().any(|t| t.content == "c" && matches!(t.status, super::super::types::TodoStatus::Pending)));
}
