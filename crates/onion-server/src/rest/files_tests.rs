//! `files` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::path::Path;

use super::{content_type, resolve_workspace_file};

/// 建一个临时工作区：`nested/a.md` 与 `pic.png`。
fn workspace_root(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("ava_onion_files_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("nested")).unwrap();
    std::fs::write(root.join("nested/a.md"), "hello").unwrap();
    std::fs::write(root.join("pic.png"), [0x89, 0x50, 0x4e, 0x47]).unwrap();
    root
}

#[test]
fn resolves_relative_file_inside_workspace() {
    let root = workspace_root("ok");
    let resolved = resolve_workspace_file(&root, "nested/a.md").unwrap();
    // 解析结果是规范化路径（Windows 上带 `\\?\` 前缀），须与同为规范的根比较。
    assert!(resolved.starts_with(root.canonicalize().unwrap()));
    assert_eq!(std::fs::read_to_string(&resolved).unwrap(), "hello");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn accepts_absolute_path_inside_workspace() {
    let root = workspace_root("absolute");
    let absolute = root.join("nested/a.md");
    let resolved = resolve_workspace_file(&root, absolute.to_str().unwrap()).unwrap();
    assert_eq!(std::fs::read_to_string(&resolved).unwrap(), "hello");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn rejects_paths_outside_workspace() {
    let root = workspace_root("reject");
    assert!(resolve_workspace_file(&root, "../outside.md").is_err());
    assert!(resolve_workspace_file(&root, "nested/../../outside.md").is_err());
    // 权限根之外的绝对路径同样拒绝
    let outside = std::env::temp_dir().join("ava_onion_files_outside.md");
    std::fs::write(&outside, "nope").unwrap();
    assert!(resolve_workspace_file(&root, outside.to_str().unwrap()).is_err());
    let _ = std::fs::remove_file(&outside);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn rejects_control_characters_and_empty_path() {
    let root = workspace_root("control");
    assert!(resolve_workspace_file(&root, "  ").is_err());
    assert!(resolve_workspace_file(&root, "nested/a\0.md").is_err());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn missing_file_is_an_error() {
    let root = workspace_root("missing");
    assert!(resolve_workspace_file(&root, "nested/none.md").is_err());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn content_type_maps_previewable_kinds() {
    assert_eq!(content_type(Path::new("a.png")), "image/png");
    assert_eq!(content_type(Path::new("a.JPEG")), "image/jpeg");
    assert_eq!(content_type(Path::new("a.md")), "text/markdown; charset=utf-8");
    assert_eq!(content_type(Path::new("a.json")), "application/json; charset=utf-8");
    assert_eq!(content_type(Path::new("a.svg")), "text/plain; charset=utf-8");
    assert_eq!(content_type(Path::new("LICENSE")), "text/plain; charset=utf-8");
}
