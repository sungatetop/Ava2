//! `validation` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use crate::agent_team::validation::{member_name, required_text, write_scope, write_scopes};
use crate::agent_team::TeamError;

#[test]
fn member_name_accepts_lower_kebab() {
    assert!(member_name("reviewer").is_ok());
    assert!(member_name("code-reviewer").is_ok());
    assert!(member_name("a1-b2").is_ok());
}

#[test]
fn member_name_rejects_invalid() {
    // 大写 / 下划线 / 前导连字符 / 尾连字符 / 保留 lead。
    assert!(matches!(member_name("Reviewer"), Err(TeamError::InvalidMemberName(_))));
    assert!(matches!(member_name("reviewer_x"), Err(TeamError::InvalidMemberName(_))));
    assert!(matches!(member_name("-reviewer"), Err(TeamError::InvalidMemberName(_))));
    assert!(matches!(member_name("reviewer-"), Err(TeamError::InvalidMemberName(_))));
    assert!(matches!(member_name("lead"), Err(TeamError::InvalidMemberName(_))));
    // 超长。
    let long = "a".repeat(65);
    assert!(matches!(member_name(&long), Err(TeamError::InvalidMemberName(_))));
}

#[test]
fn required_text_trims_and_bounds() {
    assert_eq!(required_text("  hello  ", 100).unwrap(), "hello");
    assert!(matches!(required_text("   ", 100), Err(TeamError::InvalidArgument(_))));
    assert!(matches!(required_text("", 100), Err(TeamError::InvalidArgument(_))));
    let long = "x".repeat(101);
    assert!(matches!(
        required_text(&long, 100),
        Err(TeamError::InvalidArgument(_))
    ));
}

#[test]
fn write_scope_normalizes_backslashes_and_prefix() {
    assert_eq!(write_scope("src\\lib").unwrap(), "src/lib");
    assert_eq!(write_scope("./src/lib").unwrap(), "src/lib");
    assert_eq!(write_scope("src/lib/").unwrap(), "src/lib");
    assert_eq!(write_scope("src").unwrap(), "src");
}

#[test]
fn write_scope_rejects_absolute_and_dot_segments() {
    assert!(matches!(write_scope("/abs"), Err(TeamError::InvalidWriteScope(_))));
    assert!(matches!(write_scope("C:/abs"), Err(TeamError::InvalidWriteScope(_))));
    assert!(matches!(write_scope(""), Err(TeamError::InvalidWriteScope(_))));
    assert!(matches!(write_scope("a/.."), Err(TeamError::InvalidWriteScope(_))));
    assert!(matches!(write_scope("a/."), Err(TeamError::InvalidWriteScope(_))));
    assert!(matches!(write_scope("a//b"), Err(TeamError::InvalidWriteScope(_))));
}

#[test]
fn write_scopes_dedupes_preserving_order() {
    let v = vec!["src".to_string(), "src/lib".to_string(), "src".to_string()];
    let out = write_scopes(&v).unwrap();
    assert_eq!(out, vec!["src".to_string(), "src/lib".to_string()]);
}
