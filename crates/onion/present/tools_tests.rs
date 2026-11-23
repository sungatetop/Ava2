//! `present` 工具单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::fs;
use std::sync::Arc;

use serde_json::json;

use super::PresentTool;
use crate::sandbox::Sandbox;
use crate::session::SessionLog;
use crate::tools::ToolPort;
use crate::types::{ContentBlock, ToolCall};

fn temp_root(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("onion_present_{tag}_{}", std::process::id()))
}

fn session() -> Arc<SessionLog> {
    Arc::new(SessionLog::in_memory())
}

fn tool(root: std::path::PathBuf) -> PresentTool {
    PresentTool::new(Arc::new(Sandbox::new(root)), 8)
}

fn call(name: &str, input: serde_json::Value) -> ToolCall {
    ToolCall { id: "call-1".to_string(), name: name.to_string(), input }
}

async fn run_json(tool: &PresentTool, call: ToolCall) -> Result<serde_json::Value, String> {
    let def = tool.definition();
    assert_eq!(def.name, "present");
    let blocks = tool
        .execute(call, tokio_util::sync::CancellationToken::new())
        .await?;
    let mut text = String::new();
    for block in blocks {
        if let ContentBlock::Text { text: t } = block {
            text.push_str(&t);
        }
    }
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

#[tokio::test]
async fn present_requires_owning_session() {
    let root = temp_root("no_session");
    let tool = tool(root.clone());
    let result = tool
        .execute(
            call("present", json!({ "files": [{"path": "a.txt"}] })),
            tokio_util::sync::CancellationToken::new(),
        )
        .await;
    assert!(result.is_err());
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn present_missing_files_rejected() {
    let root = temp_root("no_files");
    let sess = session();
    let tool = tool(root.clone());
    tool.wire_session(sess.clone());
    let result = run_json(&tool, call("present", json!({}))).await;
    assert!(result.unwrap_err().contains("missing 'files'"));
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn present_empty_or_too_many_rejected() {
    let root = temp_root("count");
    let sess = session();
    let tool = tool(root.clone());
    tool.wire_session(sess.clone());
    // 空数组。
    let result = run_json(&tool, call("present", json!({ "files": [] }))).await;
    assert!(result.unwrap_err().contains("accepts 1 to 8 files"));
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn present_missing_file_rejected() {
    let root = temp_root("missing");
    fs::create_dir_all(&root).unwrap();
    let sess = session();
    let tool = tool(root.clone());
    tool.wire_session(sess.clone());
    let result = run_json(&tool, call("present", json!({ "files": [{"path": "nope.txt"}] }))).await;
    let err = result.unwrap_err();
    assert!(err.contains("file not found"), "{err}");
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn present_directory_rejected() {
    let root = temp_root("dir");
    fs::create_dir_all(root.join("subdir")).unwrap();
    let sess = session();
    let tool = tool(root.clone());
    tool.wire_session(sess.clone());
    let result = run_json(&tool, call("present", json!({ "files": [{"path": "subdir"}] }))).await;
    let err = result.unwrap_err();
    assert!(err.contains("not a regular file"), "{err}");
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn present_path_escape_rejected() {
    let root = temp_root("escape");
    let sess = session();
    let tool = tool(root.clone());
    tool.wire_session(sess.clone());
    let result = run_json(&tool, call("present", json!({ "files": [{"path": "../evil.txt"}] }))).await;
    let err = result.unwrap_err();
    assert!(err.contains("file not found") || err.contains("not allowed"), "{err}");
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn present_success_appends_durable_event() {
    let root = temp_root("ok");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("report.docx"), "data").unwrap();
    let sess = session();
    let tool = tool(root.clone());
    tool.wire_session(sess.clone());
    let value = run_json(
        &tool,
        call("present", json!({ "files": [{"path": "report.docx", "description": "Final report"}] })),
    )
    .await
    .unwrap();
    assert_eq!(value["turn"], 0);
    assert_eq!(value["files"][0]["path"], "report.docx");
    assert_eq!(value["files"][0]["description"], "Final report");
    // durable 事件已落盘。
    let presented = sess
        .snapshot()
        .into_iter()
        .filter(|event| matches!(event, crate::session::SessionEvent::Presented { .. }))
        .count();
    assert_eq!(presented, 1);
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn present_multiple_files() {
    let root = temp_root("multi");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.txt"), "a").unwrap();
    fs::write(root.join("b.xlsx"), "b").unwrap();
    let sess = session();
    let tool = tool(root.clone());
    tool.wire_session(sess.clone());
    let value = run_json(
        &tool,
        call(
            "present",
            json!({ "files": [{"path": "a.txt"}, {"path": "b.xlsx", "description": "Data"}] }),
        ),
    )
    .await
    .unwrap();
    assert_eq!(value["files"].as_array().unwrap().len(), 2);
    let _ = fs::remove_dir_all(root);
}
