//! `scan` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;
use crate::memory::types::MemoryType;
use std::path::PathBuf;

fn make_file(root: &Path, name: &str, content: &str) {
    fs::create_dir_all(root).unwrap();
    fs::write(root.join(name), content).unwrap();
}

#[test]
fn scan_memory_files_skips_entrypoint_and_non_md() {
    let tmp = std::env::temp_dir().join(format!("ava_scan_skip_{}", std::process::id()));
    let root = tmp.join("mem");
    let _ = fs::remove_dir_all(&tmp);
    make_file(&root, ENTRYPOINT_NAME, "# index\n");
    make_file(&root, "user_role.md", "---\nname: user_role\ndescription: User role\ntype: user\n---\n\nbody\n");
    make_file(&root, "notes.txt", "not a memory\n");
    let headers = scan_memory_files(&root).unwrap();
    assert_eq!(headers.len(), 1);
    assert_eq!(headers[0].filename, "user_role.md");
    assert_eq!(headers[0].description, "User role");
    assert_eq!(headers[0].memory_type, Some(MemoryType::User));
    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn scan_memory_files_missing_root_returns_empty() {
    let tmp = std::env::temp_dir().join(format!("ava_scan_missing_{}", std::process::id()));
    let root = tmp.join("no_such_dir");
    let headers = scan_memory_files(&root).unwrap();
    assert!(headers.is_empty());
}

#[test]
fn scan_memory_files_sorts_newest_first() {
    let tmp = std::env::temp_dir().join(format!("ava_scan_order_{}", std::process::id()));
    let root = tmp.join("mem");
    let _ = fs::remove_dir_all(&tmp);
    make_file(&root, "a.md", "---\nname: a\ndescription: A\n---\n\nx\n");
    // 留出 mtime 间隔，保证 b.md 时间戳严格更新。
    std::thread::sleep(std::time::Duration::from_millis(50));
    make_file(&root, "b.md", "---\nname: b\ndescription: B\n---\n\ny\n");
    let headers = scan_memory_files(&root).unwrap();
    assert_eq!(headers[0].filename, "b.md");
    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn format_memory_manifest_empty_and_nonempty() {
    assert_eq!(format_memory_manifest(&[]), "(no memories)");
    let header = MemoryHeader {
        filename: "user_role.md".into(),
        file_path: PathBuf::from("/tmp/user_role.md"),
        mtime_ms: 0,
        description: "User role".into(),
        memory_type: Some(MemoryType::User),
    };
    let text = format_memory_manifest(&[header]);
    assert_eq!(text, "[user] user_role.md (1970-01-01 00:00:00Z): User role");
}

#[test]
fn snapshot_and_diff_files() {
    let tmp = std::env::temp_dir().join(format!("ava_scan_snap_{}", std::process::id()));
    let root = tmp.join("mem");
    let _ = fs::remove_dir_all(&tmp);
    make_file(&root, "a.md", "v1");
    make_file(&root, "b.md", "old");
    let before = snapshot_files(&root).unwrap();
    make_file(&root, "b.md", "new");
    make_file(&root, "c.md", "added");
    fs::remove_file(root.join("a.md")).unwrap();
    let after = snapshot_files(&root).unwrap();
    let (changed, deleted) = diff_files(&before, &after);
    assert_eq!(changed, 2); // b.md 修改 + c.md 新增
    assert_eq!(deleted, 1); // a.md 删除
    let _ = fs::remove_dir_all(&tmp);
}
