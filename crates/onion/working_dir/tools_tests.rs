//! `working_directory` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use crate::session::SessionLog;
use crate::tools::ToolPort;
use crate::types::{ContentBlock, ToolCall};

use super::WorkingDirectoryTool;

/// 构造接线好的工具（session 内存态；cwd 初始为 root）。
fn wired(root: &std::path::Path) -> (WorkingDirectoryTool, Arc<SessionLog>) {
    let tool = WorkingDirectoryTool::new(root.to_path_buf());
    let session = Arc::new(SessionLog::in_memory());
    tool.wire_session(session.clone());
    (tool, session)
}

fn call_json(tool: &WorkingDirectoryTool, input: serde_json::Value) -> Result<Vec<ContentBlock>, String> {
    let call = ToolCall {
        id: "call-1".to_string(),
        name: "working_directory".to_string(),
        input,
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(tool.execute(call, CancellationToken::new()))
}

fn text_of(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

#[test]
fn requires_owning_session() {
    // 未接线 session → 非 agent 调用方拒绝。
    let root = std::env::temp_dir().join(format!("onion_wd_owning_{}", std::process::id()));
    let tool = WorkingDirectoryTool::new(root.clone());
    let err = call_json(&tool, serde_json::json!({})).unwrap_err();
    assert!(err.contains("owning agent session"), "got: {err}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn reads_initial_cwd() {
    let root = std::env::temp_dir().join(format!("onion_wd_read_{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let (tool, _session) = wired(&root);
    let blocks = call_json(&tool, serde_json::json!({})).unwrap();
    let text = text_of(&blocks);
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    // 初始 cwd = 权限根（沙箱 root）。
    assert_eq!(value["cwd"].as_str().unwrap(), root.to_str().unwrap());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn cd_relative_resolves_from_current() {
    let root = std::env::temp_dir().join(format!("onion_wd_cd_{}", std::process::id()));
    std::fs::create_dir_all(root.join("sub")).unwrap();
    let (tool, _session) = wired(&root);
    let blocks = call_json(&tool, serde_json::json!({ "cd": "sub" })).unwrap();
    let text = text_of(&blocks);
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["cwd"].as_str().unwrap(), root.join("sub").to_str().unwrap());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn cd_chains_accumulate() {
    let root = std::env::temp_dir().join(format!("onion_wd_chain_{}", std::process::id()));
    std::fs::create_dir_all(root.join("a").join("b")).unwrap();
    let (tool, _session) = wired(&root);
    call_json(&tool, serde_json::json!({ "cd": "a" })).unwrap();
    let blocks = call_json(&tool, serde_json::json!({ "cd": "b" })).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text_of(&blocks)).unwrap();
    assert_eq!(value["cwd"].as_str().unwrap(), root.join("a").join("b").to_str().unwrap());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn cd_absolute_inside_root_allowed() {
    let root = std::env::temp_dir().join(format!("onion_wd_abs_{}", std::process::id()));
    std::fs::create_dir_all(root.join("sub")).unwrap();
    let (tool, _session) = wired(&root);
    // 根内绝对路径可直接 cd（越界判定看最终位置，不看表述形式）。
    let blocks = call_json(&tool, serde_json::json!({ "cd": root.join("sub").to_str().unwrap() })).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text_of(&blocks)).unwrap();
    assert_eq!(value["cwd"].as_str().unwrap(), root.join("sub").to_str().unwrap());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn cd_outside_root_rejected() {
    let root = std::env::temp_dir().join(format!("onion_wd_abs_out_{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let (tool, _session) = wired(&root);
    let outside = std::env::temp_dir().join("elsewhere");
    let err = call_json(&tool, serde_json::json!({ "cd": outside.to_str().unwrap() })).unwrap_err();
    assert!(err.contains("outside the permission root"), "got: {err}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn cd_escape_rejected() {
    let root = std::env::temp_dir().join(format!("onion_wd_esc_{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let (tool, _session) = wired(&root);
    let err = call_json(&tool, serde_json::json!({ "cd": ".." })).unwrap_err();
    assert!(err.contains("outside the permission root"), "got: {err}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn cd_empty_rejected() {
    let root = std::env::temp_dir().join(format!("onion_wd_empty_{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let (tool, _session) = wired(&root);
    let err = call_json(&tool, serde_json::json!({ "cd": "" })).unwrap_err();
    assert!(err.contains("non-empty"), "got: {err}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn cd_appends_durable_event() {
    let root = std::env::temp_dir().join(format!("onion_wd_durable_{}", std::process::id()));
    std::fs::create_dir_all(root.join("sub")).unwrap();
    let (tool, session) = wired(&root);
    call_json(&tool, serde_json::json!({ "cd": "sub" })).unwrap();
    let count = session
        .snapshot()
        .into_iter()
        .filter(|e| matches!(e, crate::session::SessionEvent::WorkingDir { .. }))
        .count();
    assert_eq!(count, 1);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn wire_replays_cwd_from_events() {
    let root = std::env::temp_dir().join(format!("onion_wd_replay_{}", std::process::id()));
    std::fs::create_dir_all(root.join("sub")).unwrap();
    // 先构造带持久事件的 session（模拟重放路径）。
    let session = Arc::new(SessionLog::in_memory());
    session.append(crate::session::SessionEvent::WorkingDir {
        cwd: root.join("sub").to_string_lossy().to_string(),
    });
    let tool = WorkingDirectoryTool::new(root.clone());
    tool.wire_session(session);
    let blocks = call_json(&tool, serde_json::json!({})).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text_of(&blocks)).unwrap();
    assert_eq!(value["cwd"].as_str().unwrap(), root.join("sub").to_str().unwrap());
    let _ = std::fs::remove_dir_all(&root);
}
