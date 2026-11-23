//! `spill::store` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};

use super::{encode_segment, SpillStore};

fn temp_root(tag: &str) -> std::path::PathBuf {
    // 同进程内多个测试共用根时按计数器分目录，避免并行冲突。
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("onion_spill_store_{tag}_{}_{}", std::process::id(), seq))
}

#[test]
fn encode_segment_keeps_safe_and_escapes_rest() {
    assert_eq!(encode_segment("report.txt"), "report.txt");
    assert_eq!(encode_segment("a b/c"), "a~20b~2Fc");
    assert_eq!(encode_segment(""), "_");
    // `~` 自身必被转义，保证编码无歧义（可逆）。
    assert_eq!(encode_segment("a~b"), "a~7Eb");
    // 多字节字符按 UTF-8 字节逐个转义。
    assert_eq!(encode_segment("中"), "~E4~B8~AD");
}

#[test]
fn encode_segment_truncates_long_names() {
    let encoded = encode_segment(&"x".repeat(200));
    assert_eq!(encoded.chars().count(), 48);
}

#[test]
fn save_text_roundtrip_yields_readable_ref() {
    let root = temp_root("roundtrip");
    let store = SpillStore::new(root.clone(), 1);
    let saved = store.save_text("onion-1", "report.txt", "hello spill").unwrap();
    assert_eq!(saved.bytes, "hello spill".len() as u64);
    assert!(saved.locator.contains("report.txt"));
    assert!(saved.retrieval_hint.contains(&saved.locator));
    let content = fs::read_to_string(&saved.locator).unwrap();
    assert_eq!(content, "hello spill");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn save_text_is_exclusive_and_namespaced_per_session() {
    let root = temp_root("exclusive");
    let store = SpillStore::new(root.clone(), 1);
    // 同建议名两次保存 → 两个文件（随机前缀），不互相覆盖。
    let a = store.save_text("session-a", "out.txt", "first").unwrap();
    let b = store.save_text("session-a", "out.txt", "second").unwrap();
    assert_ne!(a.locator, b.locator);
    assert_eq!(fs::read_to_string(&a.locator).unwrap(), "first");
    assert_eq!(fs::read_to_string(&b.locator).unwrap(), "second");
    // 不同 owner 会话 → 不同子目录（互不污染）。
    let c = store.save_text("session-b", "out.txt", "other").unwrap();
    assert_ne!(
        Path::new(&a.locator).parent().unwrap(),
        Path::new(&c.locator).parent().unwrap()
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn save_text_layout_matches_sessions_convention() {
    let root = temp_root("layout");
    let store = SpillStore::new(root.clone(), 1);
    let saved = store.save_text("onion-7", "grep.txt", "match").unwrap();
    let session_dir = root.join("sessions").join("onion-7");
    assert_eq!(Path::new(&saved.locator).parent().unwrap(), session_dir);
    let _ = fs::remove_dir_all(root);
}
