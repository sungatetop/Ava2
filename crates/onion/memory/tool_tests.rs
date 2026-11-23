//! `tool` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;
use crate::memory::types::MemoryType;
use crate::memory::paths::MemoryPaths;
use serde_json::{json, Value};
use std::fs;

fn store_in_tmp(tag: &str) -> MemoryStore {
    let tmp = std::env::temp_dir().join(format!("ava_tool_{tag}_{}", std::process::id()));
    let root = tmp.join("mem");
    let _ = fs::remove_dir_all(&tmp);
    MemoryStore::new(MemoryPaths { root })
}

fn call(input: Value) -> ToolCall {
    ToolCall { id: "t1".into(), name: "test".into(), input }
}

async fn run(
    tool: &dyn ToolPort,
    input: Value,
) -> Result<Vec<ContentBlock>, String> {
    tool.execute(call(input), CancellationToken::new()).await
}

#[tokio::test]
async fn save_memory_writes_file() {
    let store = store_in_tmp("save_tool");
    let tool = SaveMemory::new(store.clone());
    let out = run(
        &tool,
        json!({ "name": "User Role", "type": "user", "description": "User role", "body": "user body" }),
    )
    .await
    .unwrap();
    let text = match &out[0] {
        ContentBlock::Text { text } => text.clone(),
        _ => panic!("expected text"),
    };
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["ok"], true);
    assert!(store.root().join("user-role.md").exists());
}

#[tokio::test]
async fn save_memory_rejects_invalid_type() {
    let store = store_in_tmp("save_invalid");
    let tool = SaveMemory::new(store);
    let err = run(
        &tool,
        json!({ "name": "x", "type": "bogus", "description": "d", "body": "b" }),
    )
    .await
    .unwrap_err();
    assert!(err.contains("invalid type"));
}

#[tokio::test]
async fn recall_memory_returns_hits() {
    let store = store_in_tmp("recall_tool");
    store.save("Rust Pref", MemoryType::User, "User prefers Rust", "rust body").unwrap();
    let tool = RecallMemory::new(store);
    let out = run(&tool, json!({ "query": "rust" })).await.unwrap();
    let text = match &out[0] {
        ContentBlock::Text { text } => text.clone(),
        _ => panic!("expected text"),
    };
    let v: Value = serde_json::from_str(&text).unwrap();
    let memories = v["memories"].as_array().unwrap();
    assert_eq!(memories.len(), 1);
    assert_eq!(memories[0]["filename"], "rust-pref.md");
}

#[tokio::test]
async fn search_memories_tool_returns_snippets() {
    let store = store_in_tmp("search_tool");
    store.save("Release Process", MemoryType::Project, "release steps", "release checklist").unwrap();
    let tool = SearchMemories::new(store);
    let out = run(&tool, json!({ "query": "release" })).await.unwrap();
    let text = match &out[0] {
        ContentBlock::Text { text } => text.clone(),
        _ => panic!("expected text"),
    };
    assert!(text.contains("release-process.md"));
}

#[tokio::test]
async fn list_memories_reads_index_and_headers() {
    let store = store_in_tmp("list_tool");
    store.save("A", MemoryType::Project, "A desc", "a").unwrap();
    let tool = ListMemories::new(store);
    let out = run(&tool, json!({})).await.unwrap();
    let text = match &out[0] {
        ContentBlock::Text { text } => text.clone(),
        _ => panic!("expected text"),
    };
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["memories"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn read_memory_loads_file() {
    let store = store_in_tmp("read_tool");
    store.save("A", MemoryType::Project, "A desc", "a body").unwrap();
    let tool = ReadMemory::new(store);
    let out = run(&tool, json!({ "filename": "a.md" })).await.unwrap();
    let text = match &out[0] {
        ContentBlock::Text { text } => text.clone(),
        _ => panic!("expected text"),
    };
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["body"], "a body");
}

#[tokio::test]
async fn update_memory_partial_update() {
    let store = store_in_tmp("update_tool");
    store.save("A", MemoryType::Project, "A desc", "a body").unwrap();
    let tool = UpdateMemory::new(store.clone());
    run(&tool, json!({ "filename": "a.md", "description": "new desc" })).await.unwrap();
    let file = store.load("a.md").unwrap().unwrap();
    assert_eq!(file.description, "new desc");
    assert_eq!(file.body, "a body");
}

#[tokio::test]
async fn delete_memory_removes_file() {
    let store = store_in_tmp("delete_tool");
    store.save("A", MemoryType::Project, "A desc", "a body").unwrap();
    let tool = DeleteMemory::new(store.clone());
    run(&tool, json!({ "filename": "a.md" })).await.unwrap();
    assert!(!store.root().join("a.md").exists());
}

#[tokio::test]
async fn delete_memory_rejects_entrypoint() {
    let store = store_in_tmp("delete_ep");
    let tool = DeleteMemory::new(store);
    let err = run(&tool, json!({ "filename": "MEMORY.md" })).await.unwrap_err();
    assert!(err.contains("cannot be deleted"));
}

#[test]
fn memory_tools_registers_five() {
    let store = store_in_tmp("reg");
    let tools = memory_tools(store);
    assert_eq!(tools.len(), 5);
    let names: Vec<String> = tools.iter().map(|t| t.definition().name).collect();
    assert!(names.contains(&"SaveMemory".to_string()));
    assert!(names.contains(&"UpdateMemory".to_string()));
    assert!(names.contains(&"RecallMemory".to_string()));
    assert!(names.contains(&"ReadMemory".to_string()));
    assert!(names.contains(&"memory_search".to_string()));
}

#[test]
fn agent_memory_tools_registers_five() {
    let store = store_in_tmp("agent_reg");
    let tools = agent_memory_tools(store);
    assert_eq!(tools.len(), 5);
    let names: Vec<String> = tools.iter().map(|t| t.definition().name).collect();
    assert!(names.contains(&"ListMemories".to_string()));
    assert!(names.contains(&"ReadMemory".to_string()));
    assert!(names.contains(&"SaveMemory".to_string()));
    assert!(names.contains(&"UpdateMemory".to_string()));
    assert!(names.contains(&"DeleteMemory".to_string()));
}

#[test]
fn risk_levels_are_assigned() {
    let store = store_in_tmp("risk");
    let delete = DeleteMemory::new(store.clone());
    assert_eq!(delete.risk(), crate::approval::ToolRisk::WorkspaceWrite);
    let save = SaveMemory::new(store);
    assert_eq!(save.risk(), crate::approval::ToolRisk::ReadOnly);
}
