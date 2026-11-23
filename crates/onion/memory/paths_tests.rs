//! `paths` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;

#[test]
fn entrypoint_appends_memory_md() {
    let tmp = std::env::temp_dir().join(format!("ava_paths_ep_{}", std::process::id()));
    let paths = MemoryPaths { root: tmp.clone() };
    assert_eq!(paths.entrypoint(), tmp.join("MEMORY.md"));
}

#[test]
fn resolve_without_override_uses_data_home() {
    // 无 override 时 root = data_home()/projects/<normalized>/memory/。
    let paths = MemoryPaths::resolve(std::path::Path::new("."));
    assert!(paths.root.to_string_lossy().contains("memory"));
}

#[test]
fn is_within_checks_lexical_prefix() {
    let tmp = std::env::temp_dir().join(format!("ava_paths_within_{}", std::process::id()));
    let paths = MemoryPaths { root: tmp.clone() };
    assert!(paths.is_within(&tmp.join("a.md")));
    assert!(paths.is_within(&tmp));
    assert!(!paths.is_within(tmp.parent().unwrap()));
}

#[test]
fn validate_memory_path_rejects_relative() {
    assert!(validate_memory_path(std::path::Path::new("relative.md")).is_err());
}

#[test]
fn validate_memory_path_rejects_parent_traversal() {
    assert!(validate_memory_path(std::path::Path::new("/a/../b.md")).is_err());
}

#[test]
fn validate_memory_path_rejects_nul_byte() {
    assert!(validate_memory_path(std::path::Path::new("/a\0.md")).is_err());
}

#[test]
fn validate_memory_path_accepts_absolute() {
    assert!(validate_memory_path(std::path::Path::new("/home/u/memory/a.md")).is_ok());
}

#[test]
fn slug_filename_converts_spaces_and_case() {
    assert_eq!(slug_filename("Ava 图片分析最佳实践"), "ava-");
    assert_eq!(slug_filename("My Note"), "my-note");
    assert_eq!(slug_filename("---"), "memory");
    assert_eq!(slug_filename("a_b-c"), "a_b-c");
}
