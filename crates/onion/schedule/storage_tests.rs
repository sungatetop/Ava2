//! `storage` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::path::PathBuf;
use std::sync::Arc;

use super::super::types::{DeliveryHistory, ScheduleRecord, ScheduleTask, TaskStatus};
use super::*;

/// 临时根目录（进程隔离，测试后清理）。
fn temp_root(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ava_onion_schedule_{}_{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// 构造一个最小任务（At 一次性，全局 id）。
fn task(id: &str, session: &str, prompt: &str) -> ScheduleTask {
    ScheduleTask {
        session_id: session.to_string(),
        record: ScheduleRecord::At(super::super::types::AtRecord {
            id: id.to_string(),
            title: "t".to_string(),
            prompt: prompt.to_string(),
            scheduled_at: "2030-01-01T00:00:00.000Z".to_string(),
        }),
        status: TaskStatus::Active,
        last_delivery: None,
        delivery_history: Some(DeliveryHistory {
            records: Vec::new(),
            earlier_records_unavailable: false,
            earlier_records_pruned: None,
        }),
        session_mode: None,
    }
}

#[test]
fn open_creates_dir_and_empty_store() {
    let root = temp_root("open_creates");
    let store = HostScheduleStore::open(&root).unwrap();
    assert!(schedule_dir(&root).is_dir());
    assert!(store.all().is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn put_get_all_delete_roundtrip() {
    let root = temp_root("roundtrip");
    let store = HostScheduleStore::open(&root).unwrap();
    let t = task("schedule-abc", "s1", "hello");
    store.put(t.clone()).unwrap();
    assert_eq!(store.get("schedule-abc"), Some(t));
    assert_eq!(store.all().len(), 1);
    assert!(store.delete("schedule-abc").unwrap());
    assert!(store.get("schedule-abc").is_none());
    assert!(store.all().is_empty());
    // 删除不存在 → false。
    assert!(!store.delete("schedule-abc").unwrap());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn flush_persists_and_reopens() {
    let root = temp_root("reopen");
    {
        let store = HostScheduleStore::open(&root).unwrap();
        store.put(task("schedule-1", "s1", "a")).unwrap();
        store.put(task("schedule-2", "s2", "b")).unwrap();
    }
    // 重新打开回放。
    let reopened = HostScheduleStore::open(&root).unwrap();
    assert_eq!(reopened.all().len(), 2);
    assert!(reopened.get("schedule-1").is_some());
    assert!(reopened.get("schedule-2").is_some());
    // 原始行非空。
    assert_eq!(reopened.raw_lines().len(), 2);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn read_tasks_file_parses_lines() {
    let root = temp_root("read_file");
    let store = HostScheduleStore::open(&root).unwrap();
    store.put(task("schedule-1", "s1", "a")).unwrap();
    let parsed = read_tasks_file(&tasks_file(&root)).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].record.id(), "schedule-1");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn corrupt_lines_are_skipped() {
    let root = temp_root("corrupt");
    let store = HostScheduleStore::open(&root).unwrap();
    store.put(task("schedule-1", "s1", "a")).unwrap();
    // 追加一行损坏 JSON，回放时跳过。
    {
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(tasks_file(&root))
            .unwrap();
        use std::io::Write;
        writeln!(f, "not valid json").unwrap();
    }
    let reopened = HostScheduleStore::open(&root).unwrap();
    assert_eq!(reopened.all().len(), 1);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn store_is_send_sync_shareable() {
    // 编译期约束：Arc<HostScheduleStore> 可跨线程共享（runtime/服务持有）。
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Arc<HostScheduleStore>>();
}
