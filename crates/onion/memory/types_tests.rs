//! `types` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;

#[test]
fn memory_type_as_str_covers_all_variants() {
    assert_eq!(MemoryType::User.as_str(), "user");
    assert_eq!(MemoryType::Feedback.as_str(), "feedback");
    assert_eq!(MemoryType::Project.as_str(), "project");
    assert_eq!(MemoryType::Reference.as_str(), "reference");
}

#[test]
fn parse_memory_type_normalizes_case_and_trims() {
    assert_eq!(parse_memory_type(" user "), Some(MemoryType::User));
    assert_eq!(parse_memory_type("FEEDBACK"), Some(MemoryType::Feedback));
    assert_eq!(parse_memory_type("Project"), Some(MemoryType::Project));
    assert_eq!(parse_memory_type("reference"), Some(MemoryType::Reference));
}

#[test]
fn parse_memory_type_rejects_invalid() {
    assert_eq!(parse_memory_type(""), None);
    assert_eq!(parse_memory_type("other"), None);
    assert_eq!(parse_memory_type("USER "), Some(MemoryType::User));
}

#[test]
fn manifest_line_formats_correctly() {
    let header = MemoryHeader {
        filename: "user_role.md".into(),
        file_path: std::path::PathBuf::from("/tmp/user_role.md"),
        mtime_ms: 0,
        description: "User role".into(),
        memory_type: Some(MemoryType::User),
    };
    assert_eq!(header.manifest_line(), "[user] user_role.md (1970-01-01 00:00:00Z): User role");
}

#[test]
fn iso_time_epoch() {
    assert_eq!(iso_time(0), "1970-01-01 00:00:00Z");
}

#[test]
fn iso_time_known_date() {
    // 2026-10-22 00:00:00 UTC = 1_792_627_200 秒
    let ms = 1_792_627_200 * 1000;
    assert_eq!(iso_time(ms), "2026-10-22 00:00:00Z");
}
