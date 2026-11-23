//! `tools` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//!
//! 遍历 4 个 schedule 工具的执行路径：create/list/update/delete 的 happy path
//! 与错误分支（selector 校验、not-found、conflict）。经由注册表的公开
//! `execute`（模式过滤 + 超时治理）驱动，贴近真实装配。工具绑定会话 id，
//! 经 `wire_session` 注入。

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tokio_util::sync::CancellationToken;

use crate::schedule::storage::HostScheduleStore;
use crate::schedule::tools::ScheduleToolSet;
use crate::session::SessionLog;
use crate::tools::ToolRegistry;
use crate::types::{ContentBlock, ToolCall, ToolResult};

/// 构造已接线 4 工具注册表（临时 Host 存储 + 内存 Session）。
/// 每个调用使用唯一临时目录，避免并行测试互相清理。
fn wired() -> (ToolRegistry, Arc<SessionLog>) {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "ava_onion_schedule_tools_{}_{n}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let store = Arc::new(HostScheduleStore::open(&root).unwrap());
    let service = Arc::new(crate::schedule::service::ScheduleService::new(store));
    let session = Arc::new(SessionLog::in_memory());
    let toolset = ScheduleToolSet::new(service);
    toolset.wire_session(session.clone());
    let mut registry = ToolRegistry::new();
    toolset.register_all(&mut registry);
    registry.state().set_agent([
        "schedule_create".into(),
        "schedule_list".into(),
        "schedule_update".into(),
        "schedule_delete".into(),
    ]);
    (registry, session)
}

fn call(registry: &ToolRegistry, name: &str, input: serde_json::Value) -> ToolResult {
    let call = ToolCall {
        id: "call-1".to_string(),
        name: name.to_string(),
        input,
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(registry.execute(&call, &CancellationToken::new(), Duration::from_secs(5)))
}

fn text_of(result: &ToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

fn parsed(registry: &ToolRegistry, name: &str, input: serde_json::Value) -> serde_json::Value {
    let result = call(registry, name, input);
    assert!(!result.is_error, "tool {name} errored: {}", text_of(&result));
    serde_json::from_str(&text_of(&result)).unwrap()
}

#[test]
fn registers_four_tools() {
    let (registry, _) = wired();
    let names: Vec<String> = registry.definitions().into_iter().map(|d| d.name).collect();
    for expected in ["schedule_create", "schedule_list", "schedule_update", "schedule_delete"] {
        assert!(names.contains(&expected.to_string()), "missing {expected}");
    }
}

#[test]
fn create_then_list() {
    let (registry, _) = wired();
    let created = parsed(
        &registry,
        "schedule_create",
        json!({ "prompt": "drink water", "title": "hydrate", "at": "2030-01-01T00:00:00Z" }),
    );
    // 全局 uuid id。
    let id = created["id"].as_str().unwrap().to_string();
    assert!(id.starts_with("schedule-"));
    assert_eq!(created["kind"], "at");
    assert_eq!(created["state"], "scheduled");
    assert_eq!(created["delivery_mode"], "host");

    let listed = parsed(&registry, "schedule_list", json!({}));
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert_eq!(listed[0]["id"], id);
}

#[test]
fn create_rejects_multiple_selectors() {
    let (registry, _) = wired();
    // 多 selector 在 parse_create_request 阶段用 `?` 直接使工具报错（is_error=true）。
    let result = call(
        &registry,
        "schedule_create",
        json!({ "prompt": "p", "title": "t", "at": "2030-01-01T00:00:00Z", "after_seconds": 60 }),
    );
    assert!(result.is_error);
    assert!(text_of(&result).contains("exactly one"));
}

#[test]
fn create_rejects_non_future() {
    let (registry, _) = wired();
    let out = parsed(
        &registry,
        "schedule_create",
        json!({ "prompt": "p", "title": "t", "at": "2000-01-01T00:00:00Z" }),
    );
    assert_eq!(out["code"], "not_future");
}

#[test]
fn delete_existing_then_not_found() {
    let (registry, _) = wired();
    let created = parsed(
        &registry,
        "schedule_create",
        json!({ "prompt": "p", "title": "t", "at": "2030-01-01T00:00:00Z" }),
    );
    let id = created["id"].as_str().unwrap();
    let del = parsed(&registry, "schedule_delete", json!({ "id": id }));
    assert_eq!(del["deleted"], true);

    let again = parsed(&registry, "schedule_delete", json!({ "id": id }));
    assert_eq!(again["deleted"], false);
    assert_eq!(again["code"], "schedule_not_found");
}

#[test]
fn delete_empty_id_rejected() {
    let (registry, _) = wired();
    let result = call(&registry, "schedule_delete", json!({ "id": "  " }));
    let out: serde_json::Value = serde_json::from_str(&text_of(&result)).unwrap();
    assert_eq!(out["code"], "invalid_rule");
}

#[test]
fn update_title_keeps_id() {
    let (registry, _) = wired();
    let created = parsed(
        &registry,
        "schedule_create",
        json!({ "prompt": "p", "title": "t", "at": "2030-01-01T00:00:00Z" }),
    );
    let id = created["id"].as_str().unwrap();
    let out = parsed(
        &registry,
        "schedule_update",
        json!({ "id": id, "title": "renamed" }),
    );
    assert_eq!(out["id"], id);
    assert_eq!(out["title"], "renamed");
    assert_eq!(out["kind"], "at");
}

#[test]
fn update_rejects_multiple_timing() {
    let (registry, _) = wired();
    let created = parsed(
        &registry,
        "schedule_create",
        json!({ "prompt": "p", "title": "t", "at": "2030-01-01T00:00:00Z" }),
    );
    let id = created["id"].as_str().unwrap();
    let result = call(
        &registry,
        "schedule_update",
        json!({ "id": id, "at": "2030-01-01T00:00:00Z", "every_seconds": 60 }),
    );
    let out: serde_json::Value = serde_json::from_str(&text_of(&result)).unwrap();
    assert_eq!(out["code"], "invalid_selector");
}

#[test]
fn update_not_found() {
    let (registry, _) = wired();
    let result = call(&registry, "schedule_update", json!({ "id": "schedule-9", "title": "x" }));
    let out: serde_json::Value = serde_json::from_str(&text_of(&result)).unwrap();
    assert_eq!(out["code"], "schedule_not_found");
}

#[test]
fn update_timing_change_switches_kind() {
    let (registry, _) = wired();
    let created = parsed(
        &registry,
        "schedule_create",
        json!({ "prompt": "p", "title": "t", "at": "2030-01-01T00:00:00Z" }),
    );
    let id = created["id"].as_str().unwrap();
    let out = parsed(
        &registry,
        "schedule_update",
        json!({ "id": id, "every_seconds": 60 }),
    );
    assert_eq!(out["id"], id);
    assert_eq!(out["kind"], "every");
}
