//! `config` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use serde_json::json;

use crate::hooks::config::{
    parse_claude_code_config, substitute_command, ParsedClaudeConfig, SubstitutionVars,
};
use crate::hooks::types::MatcherMode;

#[test]
fn parses_bare_event_map() {
    let raw = json!({
        "PreToolUse": [
            { "matcher": "write_file", "hooks": [
                { "type": "command", "command": "echo blocked", "timeout": 10 }
            ]}
        ]
    });
    let parsed = parse_claude_code_config(&raw, &SubstitutionVars::default()).expect("parses");
    let pre = parsed.config.get("PreToolUse").expect("has PreToolUse");
    assert_eq!(pre.len(), 1);
    assert_eq!(pre[0].matcher.as_deref(), Some("write_file"));
    assert_eq!(pre[0].hooks[0].command, "echo blocked");
    assert_eq!(pre[0].hooks[0].timeout_sec, Some(10));
}

#[test]
fn parses_settings_hooks_wrapper() {
    let raw = json!({ "hooks": { "Stop": [ { "hooks": [ { "type": "command", "command": "echo stop" } ] } ] } });
    let parsed = parse_claude_code_config(&raw, &SubstitutionVars::default()).expect("parses");
    assert!(parsed.config.contains_key("Stop"));
}

#[test]
fn non_command_hooks_are_skipped() {
    let raw = json!({
        "PreToolUse": [
            { "matcher": "read_file", "hooks": [
                { "type": "command", "command": "echo ok" },
                { "type": "prompt", "prompt": "hello" },
                { "type": "http", "url": "http://x" }
            ]}
        ]
    });
    let parsed = parse_claude_code_config(&raw, &SubstitutionVars::default()).expect("parses");
    let pre = parsed.config.get("PreToolUse").expect("has PreToolUse");
    // 仅 command 钩子存活
    assert_eq!(pre[0].hooks.len(), 1);
    assert_eq!(pre[0].hooks[0].command, "echo ok");
    assert_eq!(parsed.skipped.len(), 2);
    assert_eq!(parsed.skipped[0].hook_type, "prompt");
    assert_eq!(parsed.skipped[1].hook_type, "http");
}

#[test]
fn matcher_discarded_for_prompt_and_stop_events() {
    let raw = json!({
        "UserPromptSubmit": [ { "matcher": "ignored", "hooks": [ { "type": "command", "command": "echo a" } ] } ],
        "Stop": [ { "matcher": "ignored", "hooks": [ { "type": "command", "command": "echo b" } ] } ]
    });
    let parsed = parse_claude_code_config(&raw, &SubstitutionVars::default()).expect("parses");
    // matcher 字段被丢弃（无 matcher 主体）
    assert_eq!(parsed.config["UserPromptSubmit"][0].matcher, None);
    assert_eq!(parsed.config["Stop"][0].matcher, None);
}

#[test]
fn invalid_regex_matcher_rejects_config() {
    let raw = json!({
        "PreToolUse": [ { "matcher": "([unclosed", "hooks": [ { "type": "command", "command": "echo a" } ] } ]
    });
    let result = parse_claude_code_config(&raw, &SubstitutionVars::default());
    assert!(result.is_err());
}

#[test]
fn unsupported_events_are_ignored() {
    let raw = json!({
        "PreCompact": [ { "hooks": [ { "type": "command", "command": "echo a" } ] } ]
    });
    let parsed = parse_claude_code_config(&raw, &SubstitutionVars::default()).expect("parses");
    assert!(parsed.config.is_empty());
}

#[test]
fn malformed_entries_ignored() {
    let raw = json!({
        "PreToolUse": [ "not an object", { "hooks": "not array" } ]
    });
    let parsed = parse_claude_code_config(&raw, &SubstitutionVars::default()).expect("parses");
    assert!(parsed.config.is_empty());
}

#[test]
fn substitution_replaces_variables() {
    let raw = json!({
        "PreToolUse": [ { "hooks": [ { "type": "command", "command": "echo ${CLAUDE_PLUGIN_ROOT} ${CLAUDE_PROJECT_DIR} ${UNSET}" } ] } ]
    });
    let vars = SubstitutionVars {
        plugin_root: Some("/plugins".to_string()),
        project_dir: Some("/proj".to_string()),
    };
    let parsed = parse_claude_code_config(&raw, &vars).expect("parses");
    let command = &parsed.config["PreToolUse"][0].hooks[0].command;
    assert_eq!(command, "echo /plugins /proj ${UNSET}");
}

#[test]
fn substitute_command_with_unset_vars_keeps_verbatim() {
    assert_eq!(substitute_command("echo ${X}", &SubstitutionVars::default()), "echo ${X}");
}

#[test]
fn empty_or_wrong_root_returns_empty() {
    let parsed = parse_claude_code_config(&json!([]), &SubstitutionVars::default()).unwrap();
    assert!(parsed.config.is_empty());
    // 校验 ParsedClaudeConfig 可比较
    let _: ParsedClaudeConfig = parsed;
    let _ = MatcherMode::ClaudeCode;
}
