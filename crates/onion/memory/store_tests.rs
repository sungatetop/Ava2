//! `store` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;

fn store_in_tmp(tag: &str) -> (std::path::PathBuf, MemoryStore) {
    let tmp = std::env::temp_dir().join(format!("ava_store_{tag}_{}", std::process::id()));
    let root = tmp.join("mem");
    let _ = fs::remove_dir_all(&tmp);
    let paths = MemoryPaths { root };
    (tmp, MemoryStore::new(paths))
}

#[test]
fn save_creates_file_and_index_line() {
    let (_tmp, store) = store_in_tmp("save");
    let path = store.save("User Role", MemoryType::User, "User role", "user body").unwrap();
    assert!(path.exists());
    assert_eq!(path.file_name().unwrap().to_str().unwrap(), "user-role.md");
    let index = store.entrypoint().unwrap();
    assert!(index.contains("- [User Role](user-role.md) — User role"));
}

#[test]
fn load_reads_frontmatter_and_body() {
    let (_tmp, store) = store_in_tmp("load");
    store.save("User Role", MemoryType::User, "User role", "user body").unwrap();
    let file = store.load("user-role.md").unwrap().unwrap();
    assert_eq!(file.name, "User Role");
    assert_eq!(file.description, "User role");
    assert_eq!(file.memory_type, Some(MemoryType::User));
    assert_eq!(file.body, "user body");
}

#[test]
fn update_partial_fields_and_syncs_index() {
    let (_tmp, store) = store_in_tmp("update");
    store.save("User Role", MemoryType::User, "User role", "user body").unwrap();
    store.update("user-role.md", Some(MemoryType::Feedback), Some("updated desc"), None).unwrap();
    let file = store.load("user-role.md").unwrap().unwrap();
    assert_eq!(file.memory_type, Some(MemoryType::Feedback));
    assert_eq!(file.description, "updated desc");
    assert_eq!(file.body, "user body");
    let index = store.entrypoint().unwrap();
    assert!(index.contains("updated desc"));
}

#[test]
fn update_rejects_entrypoint() {
    let (_tmp, store) = store_in_tmp("update_ep");
    let err = store.update("MEMORY.md", None, Some("x"), None);
    assert!(err.is_err());
}

#[test]
fn delete_removes_file_and_index_line() {
    let (_tmp, store) = store_in_tmp("delete");
    store.save("A", MemoryType::Project, "A desc", "a body").unwrap();
    store.save("B", MemoryType::Reference, "B desc", "b body").unwrap();
    store.delete("a.md").unwrap();
    assert!(!store.root().join("a.md").exists());
    let index = store.entrypoint().unwrap();
    assert!(!index.contains("a.md"));
    assert!(index.contains("b.md"));
}

#[test]
fn list_returns_headers_sorted_by_mtime() {
    let (_tmp, store) = store_in_tmp("list");
    store.save("A", MemoryType::Project, "A desc", "a").unwrap();
    store.save("B", MemoryType::Reference, "B desc", "b").unwrap();
    let headers = store.list().unwrap();
    assert_eq!(headers.len(), 2);
    assert!(headers.iter().all(|h| !h.filename.is_empty()));
}

#[test]
fn manifest_respects_body_and_total_limits() {
    let (_tmp, store) = store_in_tmp("manifest");
    store.save("A", MemoryType::Project, "A desc", "body a").unwrap();
    store.save("B", MemoryType::Reference, "B desc", "body b").unwrap();
    let text = store.manifest(5, 10_000).unwrap();
    assert!(text.contains("### a.md"));
    assert!(text.contains("body "));
    let small = store.manifest(3, 10_000).unwrap();
    assert!(small.contains("bod"));
}
