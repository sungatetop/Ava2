//! `matcher` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use crate::hooks::matcher::{matcher_diagnostic, matches_matcher, select_hook_groups};
use crate::hooks::types::{CommandHook, MatcherGroup, MatcherMode};

#[test]
fn match_all_sentinels_select_everything() {
    for matcher in [None, Some(""), Some("*")] {
        assert!(matches_matcher(matcher, "anything", MatcherMode::ClaudeCode));
        assert!(matches_matcher(matcher, "anything", MatcherMode::Codex));
        assert_eq!(matcher_diagnostic(matcher, MatcherMode::ClaudeCode), None);
        assert_eq!(matcher_diagnostic(matcher, MatcherMode::Codex), None);
    }
}

#[test]
fn claude_literal_pipe_matches_exact_alternatives() {
    // 纯字母数字下划线 + | → 字面精确匹配
    assert!(matches_matcher(Some("read_file|list_dir"), "read_file", MatcherMode::ClaudeCode));
    assert!(matches_matcher(Some("read_file|list_dir"), "list_dir", MatcherMode::ClaudeCode));
    assert!(!matches_matcher(Some("read_file|list_dir"), "write_file", MatcherMode::ClaudeCode));
}

#[test]
fn claude_regex_pattern_is_unanchored() {
    // 含非词字符 → 按 unanchored regex
    assert!(matches_matcher(Some("^read_"), "read_file", MatcherMode::ClaudeCode));
    assert!(matches_matcher(Some("file$"), "read_file", MatcherMode::ClaudeCode));
    assert!(!matches_matcher(Some("^write"), "read_file", MatcherMode::ClaudeCode));
}

#[test]
fn codex_always_regex() {
    // Codex 恒为 regex：pipe 不是字面
    assert!(matches_matcher(Some("read_file|list_dir"), "read_file", MatcherMode::Codex));
    assert!(matches_matcher(Some("read."), "readX", MatcherMode::Codex));
}

#[test]
fn invalid_regex_is_non_match_and_diagnosed() {
    assert!(!matches_matcher(Some("([unclosed"), "x", MatcherMode::Codex));
    // 字面 pipe 在 claude 下有效
    assert_eq!(matcher_diagnostic(Some("read_file|list_dir"), MatcherMode::ClaudeCode), None);
    // 非法 regex 在 codex 下报诊断
    assert!(matcher_diagnostic(Some("([unclosed"), MatcherMode::Codex).is_some());
}

#[test]
fn select_hook_groups_filters_by_query() {
    let groups = vec![
        MatcherGroup {
            matcher: Some("read_file".to_string()),
            hooks: vec![CommandHook { command: "echo read".into(), timeout_sec: None }],
        },
        MatcherGroup {
            matcher: None, // match-all
            hooks: vec![CommandHook { command: "echo all".into(), timeout_sec: None }],
        },
        MatcherGroup {
            matcher: Some("write_file".to_string()),
            hooks: vec![],
        },
    ];
    let selected = select_hook_groups(Some(&groups), "read_file", MatcherMode::ClaudeCode);
    assert_eq!(selected.len(), 2);
    assert_eq!(selected[0].hooks[0].command, "echo read");
    assert_eq!(selected[1].hooks[0].command, "echo all");
}

#[test]
fn select_hook_groups_none_returns_empty() {
    assert!(select_hook_groups(None, "x", MatcherMode::ClaudeCode).is_empty());
}
