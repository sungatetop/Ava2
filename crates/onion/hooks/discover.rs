//! hooks 配置来源发现：用户级 + 工作区级分层。
//!
//! `configPath` 语义为「一个 `hooks.json` 或一个含 `hooks` 键的 settings
//! 文件」，按层发现并合并：
//! 1. 用户级：`<ONION_HOME>/hooks.json`（裸事件 map 或 `{hooks:{…}}`）；
//! 2. 用户级 settings：`<ONION_HOME>/settings.json` 的 `hooks` 键（与 /model 等
//!    其他设置共存于同一文件）；
//! 3. 工作区级：`<workspace>/.onion/hooks.json`（显式提交物，对齐 `.onion/mcp.json`）。
//!
//! 合并语义与 App contributes 一致：同名事件的 matcher 组按发现顺序追加（低优先级
//! 在前），全部命中钩子都会运行。文件不存在 → 静默跳过（自动发现正常态）；
//! 格式错误 → 错误进 notes 呈现（装配层不 panic）。

use std::path::Path;

use serde_json::{Map, Value};

/// 发现并合并用户级 / 工作区级 hooks 配置。
///
/// 返回 `(合并后的配置, 说明性 notes)`；无任何配置时配置为 `None`。
pub fn discover_hooks(root: &Path, data_home: &Path) -> (Option<Value>, Vec<String>) {
    let mut notes = Vec::new();
    let mut merged: Map<String, Value> = Map::new();
    // (路径, 是否允许裸事件 map)：settings.json 只认 `hooks` 键，避免把其他设置
    // 误当事件名。
    let candidates = [
        (data_home.join("hooks.json"), true),
        (data_home.join("settings.json"), false),
        (root.join(".onion").join("hooks.json"), true),
    ];
    for (path, bare_allowed) in candidates {
        if !path.exists() {
            continue;
        }
        let raw = match std::fs::read_to_string(&path)
            .map_err(|error| error.to_string())
            .and_then(|text| serde_json::from_str::<Value>(&text).map_err(|error| error.to_string()))
        {
            Ok(raw) => raw,
            Err(error) => {
                notes.push(format!("hooks 配置 {} 无效: {error}", path.display()));
                continue;
            }
        };
        let Some(hooks) = extract_hooks(&raw, bare_allowed) else {
            continue;
        };
        merge_event_map(&mut merged, &hooks);
    }
    if merged.is_empty() {
        return (None, notes);
    }
    (Some(serde_json::json!({ "hooks": merged })), notes)
}

/// 合并两份 hooks 配置（`base` 在前、`extra` 在后；同名事件数组追加）。
///
/// 供装配侧把发现结果与 App contributes 合并为单份配置。
pub fn merge_hooks(base: Option<Value>, extra: Value) -> Value {
    let mut merged: Map<String, Value> = Map::new();
    if let Some(base) = base.as_ref().and_then(|value| extract_hooks(value, true)) {
        merge_event_map(&mut merged, &base);
    }
    if let Some(extra) = extract_hooks(&extra, true) {
        merge_event_map(&mut merged, &extra);
    }
    serde_json::json!({ "hooks": merged })
}

/// 读取显式指定的 hooks 配置文件（`ONION_HOOKS` 入口）。
///
/// 读取或解析失败即 panic——显式配置是用户明确意图，静默吞错会让钩子悄悄失效。
pub fn read_hooks_file(path: &str) -> Value {
    std::fs::read_to_string(path)
        .map_err(|error| error.to_string())
        .and_then(|text| serde_json::from_str(&text).map_err(|error| error.to_string()))
        .unwrap_or_else(|error| panic!("ONION_HOOKS 解析失败 {path}: {error}"))
}

/// 取配置中的事件 map：优先 `hooks` 键；`bare_allowed` 时回退整份对象。
fn extract_hooks(value: &Value, bare_allowed: bool) -> Option<Map<String, Value>> {
    if let Some(map) = value.get("hooks").and_then(Value::as_object) {
        return Some(map.clone());
    }
    if bare_allowed {
        return value.as_object().cloned();
    }
    None
}

/// 把 `source` 的事件组追加进 `merged`（同名事件数组拼接）。
fn merge_event_map(merged: &mut Map<String, Value>, source: &Map<String, Value>) {
    for (event, groups) in source {
        let slot = merged.entry(event.clone()).or_insert_with(|| Value::Array(Vec::new()));
        if let (Some(slot), Some(groups)) = (slot.as_array_mut(), groups.as_array()) {
            slot.extend(groups.iter().cloned());
        }
    }
}

#[cfg(test)]
#[path = "discover_tests.rs"]
mod tests;
