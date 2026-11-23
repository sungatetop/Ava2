//! `index` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;

#[test]
fn format_index_line_matches_blueprint() {
    let line = format_index_line("User role", "user_role.md", "User role: avoid cargo fmt");
    assert_eq!(line, "- [User role](user_role.md) — User role: avoid cargo fmt");
}

#[test]
fn format_index_line_truncates_overlong_fields() {
    let title = "x".repeat(200);
    let line = format_index_line(&title, "f.md", "hook");
    assert!(line.chars().count() <= MAX_INDEX_LINE_CHARS * 2 + 10);
}

#[test]
fn truncate_entrypoint_limits_lines_and_bytes() {
    let long = (0..300).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
    let truncated = truncate_entrypoint(&long);
    assert!(truncated.lines().count() <= MAX_ENTRYPOINT_LINES);
    assert!(truncated.contains("<warning>"));
}

#[test]
fn truncate_entrypoint_limits_bytes() {
    let long = "x".repeat(MAX_ENTRYPOINT_BYTES + 5_000);
    let truncated = truncate_entrypoint(&long);
    assert!(truncated.len() <= MAX_ENTRYPOINT_BYTES + 100);
    assert!(truncated.contains("<warning>"));
}

#[test]
fn read_entrypoint_missing_returns_empty() {
    let tmp = std::env::temp_dir().join(format!("ava_index_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let root = tmp.join("missing");
    let out = read_entrypoint(&root).unwrap();
    assert_eq!(out, "");
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn upsert_index_line_appends_then_replaces() {
    let tmp = std::env::temp_dir().join(format!("ava_index_upsert_{}", std::process::id()));
    let root = tmp.join("mem");
    let _ = std::fs::remove_dir_all(&tmp);
    upsert_index_line(&root, "a.md", "A", "first").unwrap();
    upsert_index_line(&root, "b.md", "B", "second").unwrap();
    let content = read_entrypoint(&root).unwrap();
    assert!(content.contains("- [A](a.md) — first"));
    assert!(content.contains("- [B](b.md) — second"));

    // 同名替换。
    upsert_index_line(&root, "a.md", "A2", "updated").unwrap();
    let content = read_entrypoint(&root).unwrap();
    assert!(content.contains("- [A2](a.md) — updated"));
    assert!(!content.contains("first"));
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn remove_index_line_removes_only_target() {
    let tmp = std::env::temp_dir().join(format!("ava_index_remove_{}", std::process::id()));
    let root = tmp.join("mem");
    let _ = std::fs::remove_dir_all(&tmp);
    upsert_index_line(&root, "a.md", "A", "keep?").unwrap();
    upsert_index_line(&root, "b.md", "B", "hook").unwrap();
    remove_index_line(&root, "a.md").unwrap();
    let content = read_entrypoint(&root).unwrap();
    assert!(!content.contains("a.md"));
    assert!(content.contains("b.md"));
    let _ = std::fs::remove_dir_all(&tmp);
}
