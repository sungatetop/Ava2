//! `discover` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::path::PathBuf;

use serde_json::json;

use super::{discover_hooks, merge_hooks};

/// 独立临时目录（测试结束清理）。
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("onion_hooks_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn write(path: &std::path::Path, value: serde_json::Value) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent");
    }
    std::fs::write(path, value.to_string()).expect("write config");
}

#[test]
fn missing_files_yield_no_config() {
    let home = temp_dir("missing_home");
    let workspace = temp_dir("missing_ws");
    let (config, notes) = discover_hooks(&workspace, &home);
    assert!(config.is_none());
    assert!(notes.is_empty());
    let _ = std::fs::remove_dir_all(&home);
    let _ = std::fs::remove_dir_all(&workspace);
}

#[test]
fn merges_user_workspace_and_settings_layers_in_order() {
    let home = temp_dir("layers_home");
    let workspace = temp_dir("layers_ws");
    // 用户级 hooks.json（裸事件 map）。
    write(
        &home.join("hooks.json"),
        json!({ "PreToolUse": [{ "matcher": "read", "hooks": [{ "type": "command", "command": "user-hook" }] }] }),
    );
    // 用户级 settings.json：只认 hooks 键，其他设置不得被当作事件。
    write(
        &home.join("settings.json"),
        json!({
            "model": "some-model",
            "hooks": { "PreToolUse": [{ "matcher": "write", "hooks": [{ "type": "command", "command": "settings-hook" }] }] }
        }),
    );
    // 工作区级 hooks.json。
    write(
        &workspace.join(".onion").join("hooks.json"),
        json!({ "PreToolUse": [{ "matcher": "edit", "hooks": [{ "type": "command", "command": "workspace-hook" }] }] }),
    );

    let (config, notes) = discover_hooks(&workspace, &home);
    assert!(notes.is_empty(), "unexpected notes: {notes:?}");
    let groups = config
        .as_ref()
        .and_then(|value| value.get("hooks"))
        .and_then(|hooks| hooks.get("PreToolUse"))
        .and_then(serde_json::Value::as_array)
        .expect("PreToolUse groups");
    // 三层按发现顺序追加（用户 hooks.json → 用户 settings → 工作区）。
    let matchers: Vec<&str> =
        groups.iter().filter_map(|group| group.get("matcher").and_then(|v| v.as_str())).collect();
    assert_eq!(matchers, vec!["read", "write", "edit"]);
    // settings.json 的 model 键未被当作事件。
    let hooks = config.as_ref().and_then(|value| value.get("hooks")).unwrap();
    assert!(hooks.get("model").is_none());
    let _ = std::fs::remove_dir_all(&home);
    let _ = std::fs::remove_dir_all(&workspace);
}

#[test]
fn malformed_file_is_collected_not_fatal() {
    let home = temp_dir("malformed_home");
    let workspace = temp_dir("malformed_ws");
    std::fs::write(home.join("hooks.json"), "{ not json").expect("write malformed");
    write(
        &workspace.join(".onion").join("hooks.json"),
        json!({ "Stop": [{ "hooks": [{ "type": "command", "command": "ok-hook" }] }] }),
    );

    let (config, notes) = discover_hooks(&workspace, &home);
    assert_eq!(notes.len(), 1);
    assert!(notes[0].contains("无效"), "unexpected note: {}", notes[0]);
    // 畸形文件被跳过，其余层仍生效。
    let hooks = config.as_ref().and_then(|value| value.get("hooks")).unwrap();
    assert!(hooks.get("Stop").is_some());
    let _ = std::fs::remove_dir_all(&home);
    let _ = std::fs::remove_dir_all(&workspace);
}

#[test]
fn merge_hooks_appends_extra_after_base() {
    let base = json!({ "hooks": { "Stop": [{ "hooks": [{ "type": "command", "command": "base" }] }] } });
    let extra = json!({ "hooks": { "Stop": [{ "hooks": [{ "type": "command", "command": "extra" }] }] } });
    let merged = merge_hooks(Some(base), extra);
    let groups = merged
        .get("hooks")
        .and_then(|hooks| hooks.get("Stop"))
        .and_then(serde_json::Value::as_array)
        .expect("Stop groups");
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0]["hooks"][0]["command"], "base");
    assert_eq!(groups[1]["hooks"][0]["command"], "extra");
}

#[test]
fn merge_hooks_without_base_keeps_extra() {
    let extra = json!({ "hooks": { "Stop": [{ "hooks": [{ "type": "command", "command": "only" }] }] } });
    let merged = merge_hooks(None, extra);
    let groups = merged
        .get("hooks")
        .and_then(|hooks| hooks.get("Stop"))
        .and_then(serde_json::Value::as_array)
        .expect("Stop groups");
    assert_eq!(groups.len(), 1);
}
