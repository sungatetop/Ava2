//! `codec` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use crate::hooks::codec::parse_hook_output;
use crate::hooks::types::HookDecision;

#[test]
fn exit_two_blocks_with_stderr_as_reason() {
    let out = parse_hook_output(Some(2), "", "blocked by policy", None);
    assert_eq!(out.decision, Some(HookDecision::Block));
    assert_eq!(out.reason.as_deref(), Some("blocked by policy"));
}

#[test]
fn exit_two_without_stderr_blocks_without_reason() {
    let out = parse_hook_output(Some(2), "", "", None);
    assert_eq!(out.decision, Some(HookDecision::Block));
    assert_eq!(out.reason, None);
}

#[test]
fn exit_zero_plain_stdout_is_kept() {
    let out = parse_hook_output(Some(0), "hello world", "", None);
    assert_eq!(out.stdout, "hello world");
    assert_eq!(out.decision, None);
}

#[test]
fn exit_zero_json_parses_top_level_decision() {
    let out = parse_hook_output(
        Some(0),
        r#"{"decision":"block","reason":"no"}"#,
        "",
        None,
    );
    assert_eq!(out.decision, Some(HookDecision::Block));
    assert_eq!(out.reason.as_deref(), Some("no"));
}

#[test]
fn top_level_allow_invalid_and_ignored() {
    // 顶层 decision 仅 approve/block；allow/deny/ask 在顶层无效并忽略
    let out = parse_hook_output(Some(0), r#"{"decision":"deny"}"#, "", None);
    assert_eq!(out.decision, None);
}

#[test]
fn permission_decision_overrides_top_level() {
    let out = parse_hook_output(
        Some(0),
        r#"{"decision":"approve","hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"nope"}}"#,
        "",
        Some("PreToolUse"),
    );
    assert_eq!(out.decision, Some(HookDecision::Deny));
    assert_eq!(out.reason.as_deref(), Some("nope"));
}

#[test]
fn mismatched_event_name_discards_event_fields() {
    let out = parse_hook_output(
        Some(0),
        r#"{"hookSpecificOutput":{"hookEventName":"Stop","permissionDecision":"deny"}}"#,
        "",
        Some("PreToolUse"),
    );
    // 判别符保留，但事件域字段被丢弃
    assert_eq!(out.hook_event_name.as_deref(), Some("Stop"));
    assert_eq!(out.decision, None);
}

#[test]
fn exit_code_other_is_non_blocking() {
    let out = parse_hook_output(Some(3), "", "boom", None);
    assert_eq!(out.decision, None);
    assert_eq!(out.stderr, "boom");
}

#[test]
fn no_exit_code_spawn_failure_is_non_blocking() {
    let out = parse_hook_output(None, "", "spawn failed", None);
    assert_eq!(out.exit_code, None);
    assert_eq!(out.decision, None);
}

#[test]
fn continue_false_and_additional_context_parsed() {
    let out = parse_hook_output(
        Some(0),
        r#"{"continue":false,"stopReason":"halt","hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"extra"}}"#,
        "",
        Some("UserPromptSubmit"),
    );
    assert_eq!(out.continue_, Some(false));
    assert_eq!(out.stop_reason.as_deref(), Some("halt"));
    assert_eq!(out.additional_context.as_deref(), Some("extra"));
}

#[test]
fn malformed_json_on_clean_exit_stays_plain() {
    let out = parse_hook_output(Some(0), "{not json", "", None);
    assert_eq!(out.stdout, "{not json");
    assert_eq!(out.decision, None);
}
