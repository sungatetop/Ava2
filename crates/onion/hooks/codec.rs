//! 解码命令钩子的进程输出（两方言共享）。
//!
//! 退出码 0 可能承载结构化 JSON 或纯 stdout；
//! 退出码 2 = 阻塞（stderr → reason）；其他退出码 = 非阻塞错误。桥决定哪些
//! 识别字段对它的钩子点适用。

use crate::hooks::types::{HookDecision, HookOutput};

/// 命令钩子用于表达「阻塞错误」的退出码（stderr → 模型）。
const BLOCKING_EXIT_CODE: i32 = 2;

/// 读字符串字段（缺失/类型不符 → None）。
fn str<'a>(obj: &'a serde_json::Map<String, serde_json::Value>, key: &str) -> Option<&'a str> {
    obj.get(key).and_then(|v| v.as_str())
}

/// 读布尔字段。
fn bool(obj: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<bool> {
    obj.get(key).and_then(serde_json::Value::as_bool)
}

/// 顶层 legacy `decision` 仅 `approve`/`block`（两参考 schema 均如此）；
/// `allow`/`deny`/`ask` 保留给 `hookSpecificOutput.permissionDecision`。
/// 越界的 `{"decision":"deny"}` 无效，忽略（不成为真实阻塞决策）。
fn top_level_decision_of(value: Option<&str>) -> Option<HookDecision> {
    match value {
        Some("approve") => Some(HookDecision::Approve),
        Some("block") => Some(HookDecision::Block),
        _ => None,
    }
}

/// `hookSpecificOutput.permissionDecision` 仅 `allow`/`deny`/`ask`。
fn permission_decision_of(value: Option<&str>) -> Option<HookDecision> {
    match value {
        Some("allow") => Some(HookDecision::Allow),
        Some("deny") => Some(HookDecision::Deny),
        Some("ask") => Some(HookDecision::Ask),
        _ => None,
    }
}

/// 把解析的结构化 stdout 对象折叠进 `output`（原地修改）。
/// `expectedEventName`（触发事件）门控 per-event `hookSpecificOutput` 块：
/// 块名不同事件或缺失 → 丢弃其事件域字段（仍记录已声明的判别符）。
fn apply_structured(
    output: &mut HookOutput,
    parsed: &serde_json::Map<String, serde_json::Value>,
    expected_event_name: Option<&str>,
) {
    if let Some(cont) = bool(parsed, "continue") {
        output.continue_ = Some(cont);
    }
    if let Some(reason) = str(parsed, "stopReason") {
        output.stop_reason = Some(reason.to_string());
    }
    if let Some(msg) = str(parsed, "systemMessage") {
        output.system_message = Some(msg.to_string());
    }
    // 顶层 legacy decision（仅 approve/block）+ reason。
    if let Some(decision) = top_level_decision_of(str(parsed, "decision")) {
        output.decision = Some(decision);
    }
    if let Some(reason) = str(parsed, "reason") {
        output.reason = Some(reason.to_string());
    }
    // hookSpecificOutput：per-event 通道，键为 hookEventName。
    // permissionDecision 覆盖顶层 decision；additionalContext / updatedInput 亦在此。
    let hso = parsed
        .get("hookSpecificOutput")
        .and_then(serde_json::Value::as_object);
    if let Some(hso) = hso {
        let event_name = str(hso, "hookEventName");
        // 始终记录判别符（日志/诊断），即使不匹配——记录该块声明的事件。
        if let Some(name) = event_name {
            output.hook_event_name = Some(name.to_string());
        }
        // 缺失或不匹配的判别符不能影响触发事件。
        if expected_event_name.is_some() && event_name != expected_event_name {
            return;
        }
        if let Some(permission) = permission_decision_of(str(hso, "permissionDecision")) {
            output.decision = Some(permission);
        }
        if let Some(reason) = str(hso, "permissionDecisionReason") {
            output.reason = Some(reason.to_string());
        }
        if let Some(ctx) = str(hso, "additionalContext") {
            output.additional_context = Some(ctx.to_string());
        }
        if let Some(updated) = hso
            .get("updatedInput")
            .and_then(serde_json::Value::as_object)
        {
            output.updated_input =
                Some(serde_json::Value::Object(updated.clone()));
        }
    }
}

/// 解码进程输出为方言无关的钩子结果。本函数全量：畸形 JSON 保持纯 stdout。
/// 设 `expected_event_name` 时，缺失/不同的 `hookSpecificOutput.hookEventName`
/// 只丢弃其事件域字段；顶层字段与已声明判别符保留。
///
/// - `exit_code` = 进程退出码；`None` = spawn 失败。
/// - `stdout`   = 捕获的 stdout（干净退出时按结构化 JSON 解析）。
/// - `stderr`   = 捕获的 stderr；退出码 2 时成为阻塞 `reason`。
pub fn parse_hook_output(
    exit_code: Option<i32>,
    stdout: &str,
    stderr: &str,
    expected_event_name: Option<&str>,
) -> HookOutput {
    let trimmed_err = stderr.trim().to_string();
    let trimmed_out = stdout.trim().to_string();
    let mut output = HookOutput {
        exit_code,
        stderr: trimmed_err.clone(),
        stdout: trimmed_out.clone(),
        ..Default::default()
    };
    // 两方言均把退出码 2 当阻塞，stderr 为其 reason。
    if exit_code == Some(BLOCKING_EXIT_CODE) {
        output.decision = Some(HookDecision::Block);
        if !trimmed_err.is_empty() {
            output.reason = Some(trimmed_err);
        }
    }
    // 结构化 stdout 仅在干净退出时有效。
    if exit_code == Some(0) {
        // 仅当 stdout 像 JSON 对象才尝试解析（对齐参考引擎：其他 stdout 当纯文本）。
        if trimmed_out.starts_with('{') {
            match serde_json::from_str::<serde_json::Value>(&trimmed_out) {
                Ok(serde_json::Value::Object(parsed)) => {
                    apply_structured(&mut output, &parsed, expected_event_name);
                }
                _ => {
                    // 干净退出的畸形 JSON = 无结构化输出（宽容）；纯 stdout 保留。
                }
            }
        }
    }
    output
}

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;
