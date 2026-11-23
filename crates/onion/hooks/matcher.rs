//! matcher 匹配与命令组选择（两方言共享）。
//!
//! Claude 把「纯字母数字下划线 + `|`」
//! 模式当字面 pipe 精确匹配，其余当 unanchored regex；Codex 恒为 regex。
//! 缺失 / 空 / `*` = 匹配全部。运行时把 invalid regex 当非匹配（不抛），
//! config parser 用 `matcher_diagnostic` 提前拒绝。

use crate::hooks::types::{MatcherGroup, MatcherMode};

/// Claude 字面模式：纯词字符 + `|`（regex-vs-literal 判别符）。
const CLAUDE_LITERAL: &str = r"^[A-Za-z0-9_|]+$";

/// 编译 unanchored matcher regex；无效模式返回 `None`。
fn compile_regex(pattern: &str) -> Option<regex::Regex> {
    regex::Regex::new(pattern).ok()
}

/// `pattern` 是否形如 Claude 字面模式（纯词字符 + `|`）。
fn is_claude_literal(pattern: &str) -> bool {
    regex::Regex::new(CLAUDE_LITERAL)
        .ok()
        .is_some_and(|re| re.is_match(pattern))
}

/// 校验一个 matcher（桥接受其配置组之前）。
/// 返回 `None` = 有效；否则为稳定诊断串。
pub fn matcher_diagnostic(matcher: Option<&str>, mode: MatcherMode) -> Option<String> {
    let pattern = matcher?;
    if pattern.is_empty() || pattern == "*" {
        return None;
    }
    if mode == MatcherMode::ClaudeCode && is_claude_literal(pattern) {
        return None;
    }
    match compile_regex(pattern) {
        Some(_) => None,
        None => Some(format!("invalid {mode:?} regex matcher {pattern:?}")),
    }
}

/// `matcher` 是否在指定方言下选中 `query`。
/// Claude 字面模式 pipe 分隔精确匹配；其余为 unanchored regex。
/// invalid regex 返回 `false`（不抛）。
pub fn matches_matcher(matcher: Option<&str>, query: &str, mode: MatcherMode) -> bool {
    let Some(pattern) = matcher else {
        return true;
    };
    if pattern.is_empty() || pattern == "*" {
        return true;
    }
    if mode == MatcherMode::ClaudeCode && is_claude_literal(pattern) {
        return pattern.split('|').any(|alt| alt == query);
    }
    compile_regex(pattern).is_some_and(|re| re.is_match(query))
}

/// 选择有命令钩子且匹配事件主体的组（按配置顺序，保留原引用）。
pub fn select_hook_groups(
    groups: Option<&[MatcherGroup]>,
    query: &str,
    mode: MatcherMode,
) -> Vec<MatcherGroup> {
    groups
        .map(|gs| {
            gs.iter()
                .filter(|g| !g.hooks.is_empty() && matches_matcher(g.matcher.as_deref(), query, mode))
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "matcher_tests.rs"]
mod tests;
