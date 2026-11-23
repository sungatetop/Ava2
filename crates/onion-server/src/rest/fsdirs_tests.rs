//! `fsdirs` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::list_dirs;

/// 建一个临时目录：`b-dir/`、`a-dir/`、`note.md`。
fn sample_root(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("ava_onion_dirs_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("b-dir")).unwrap();
    std::fs::create_dir_all(root.join("a-dir")).unwrap();
    std::fs::write(root.join("note.md"), "hello").unwrap();
    root
}

#[test]
fn lists_subdirectories_only_in_name_order() {
    let root = sample_root("list");
    let listing = list_dirs(Some(&root.to_string_lossy()), false).unwrap();
    let names: Vec<&str> = listing.entries.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(names, vec!["a-dir", "b-dir"]);
    assert!(listing.parent.is_some());
    assert!(listing.files.is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn files_opt_in_lists_regular_files() {
    let root = sample_root("files");
    let listing = list_dirs(Some(&root.to_string_lossy()), true).unwrap();
    let names: Vec<&str> = listing.files.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(names, vec!["note.md"]);
    // 目录不进 files，文件不进 entries
    assert_eq!(listing.entries.len(), 2);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn rejects_relative_path() {
    assert!(list_dirs(Some("relative/path"), false).is_err());
}

#[test]
fn missing_directory_is_not_found() {
    let missing = std::env::temp_dir().join(format!("ava_onion_dirs_missing_{}", std::process::id()));
    assert!(list_dirs(Some(&missing.to_string_lossy()), false).is_err());
}
