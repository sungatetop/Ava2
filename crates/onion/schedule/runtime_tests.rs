//! `runtime` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::sync::Arc;

use crate::types::MessageSource;

use super::super::storage::HostScheduleStore;
use super::super::time;
use super::super::types::{
    DeliveryHistory, ScheduleRecord, ScheduleTask, TaskStatus,
};
use super::*;

/// 测试桩：记录被递送文本的投递目标。
struct StubDeliver {
    delivered: std::sync::Mutex<Vec<String>>,
}

impl Deliver for StubDeliver {
    fn followup(&self, text: &str) -> Result<String, String> {
        self.delivered.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(text.to_string());
        Ok(format!("msg-{}", self.delivered.lock().unwrap_or_else(std::sync::PoisonError::into_inner).len()))
    }
}

/// 临时根目录（进程隔离）。
fn temp_root(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ava_onion_schedule_runtime_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// 构造一个已到期的 At 一次性任务（scheduled_at 在过去）。
fn due_at_task(id: &str, session: &str, prompt: &str) -> ScheduleTask {
    let now = crate::session::epoch_millis() as i64;
    let past = time::format_utc(now - 60_000);
    ScheduleTask {
        session_id: session.to_string(),
        record: ScheduleRecord::At(super::super::types::AtRecord {
            id: id.to_string(),
            title: "t".to_string(),
            prompt: prompt.to_string(),
            scheduled_at: past,
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

/// 构造一个已到期的 Every 循环任务。
fn due_every_task(id: &str, session: &str, prompt: &str) -> ScheduleTask {
    let now = crate::session::epoch_millis() as i64;
    let past = time::format_utc(now - 60_000);
    ScheduleTask {
        session_id: session.to_string(),
        record: ScheduleRecord::Every(super::super::types::EveryRecord {
            id: id.to_string(),
            title: "t".to_string(),
            prompt: prompt.to_string(),
            every_seconds: 60,
            scheduled_at: past,
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

#[tokio::test]
async fn drive_once_delivers_one_shot_and_inactivates() {
    let root = temp_root("oneshot");
    let store = Arc::new(HostScheduleStore::open(&root).unwrap());
    store.put(due_at_task("schedule-1", "s1", "hello")).unwrap();

    let agent = Arc::new(StubDeliver { delivered: std::sync::Mutex::new(Vec::new()) });
    let resolve = {
        let agent = agent.clone();
        move |sid: &str| -> Option<Arc<dyn Deliver>> {
            if sid == "s1" { Some(agent.clone()) } else { None }
        }
    };

    let delivered = drive_once(&store, DeliveryRetentionBounds::default(), resolve).await;
    assert_eq!(delivered.len(), 1);
    // 帧含一次性标记。
    let texts = agent.delivered.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(texts.len(), 1);
    assert!(texts[0].contains("[SCHEDULE REMINDER]"));
    drop(texts);
    // 状态已 commit：inactive + last_delivery 与历史追加。
    let t = store.get("schedule-1").unwrap();
    assert_eq!(t.status, TaskStatus::Inactive);
    assert!(t.last_delivery.is_some());
    assert_eq!(t.delivery_history.as_ref().unwrap().records.len(), 1);
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn drive_once_skips_unresolvable_session() {
    let root = temp_root("unresolvable");
    let store = Arc::new(HostScheduleStore::open(&root).unwrap());
    store.put(due_at_task("schedule-1", "missing-session", "hello")).unwrap();
    let resolve = |_sid: &str| -> Option<Arc<dyn Deliver>> { None };
    let delivered = drive_once(&store, DeliveryRetentionBounds::default(), resolve).await;
    assert!(delivered.is_empty());
    // 未投递：任务保持 active。
    assert_eq!(store.get("schedule-1").unwrap().status, TaskStatus::Active);
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn drive_once_advances_recurring_keeps_active() {
    let root = temp_root("recurring");
    let store = Arc::new(HostScheduleStore::open(&root).unwrap());
    store.put(due_every_task("schedule-1", "s1", "tick")).unwrap();

    let agent = Arc::new(StubDeliver { delivered: std::sync::Mutex::new(Vec::new()) });
    let resolve = {
        let agent = agent.clone();
        move |sid: &str| -> Option<Arc<dyn Deliver>> {
            if sid == "s1" { Some(agent.clone()) } else { None }
        }
    };

    let delivered = drive_once(&store, DeliveryRetentionBounds::default(), resolve).await;
    assert_eq!(delivered.len(), 1);
    let texts = agent.delivered.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(texts.len(), 1);
    assert!(texts[0].contains("[SCHEDULE REMINDER BATCH]"));
    drop(texts);
    // 循环推进：保持 active，scheduled_at 前移，递送历史追加。
    let t = store.get("schedule-1").unwrap();
    assert_eq!(t.status, TaskStatus::Active);
    let old = crate::session::epoch_millis() as i64 - 60_000;
    let new_scheduled = time::parse_utc_instant(t.record.scheduled_at()).unwrap();
    assert!(new_scheduled > old);
    assert_eq!(t.delivery_history.as_ref().unwrap().records.len(), 1);
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn deliver_task_now_delivers_future_recurring_and_advances() {
    let root = temp_root("run-now-future");
    let store = Arc::new(HostScheduleStore::open(&root).unwrap());
    // 首个目标在未来的 Every 任务（未到期）。
    let now = crate::session::epoch_millis() as i64;
    let future = time::format_utc(now + 300_000);
    let task = ScheduleTask {
        session_id: "s1".to_string(),
        record: ScheduleRecord::Every(super::super::types::EveryRecord {
            id: "schedule-1".to_string(),
            title: "t".to_string(),
            prompt: "tick".to_string(),
            every_seconds: 300,
            scheduled_at: future.clone(),
        }),
        status: TaskStatus::Active,
        last_delivery: None,
        delivery_history: Some(DeliveryHistory {
            records: Vec::new(),
            earlier_records_unavailable: false,
            earlier_records_pruned: None,
        }),
        session_mode: None,
    };
    store.put(task).unwrap();

    let agent = Arc::new(StubDeliver { delivered: std::sync::Mutex::new(Vec::new()) });
    let message_id = deliver_task_now(&store, &store.get("schedule-1").unwrap(), DeliveryRetentionBounds::default(), &*agent)
        .await
        .expect("run-now 未到期循环任务应可递送");
    assert_eq!(message_id, "msg-1");
    // 本视为消费首个 occurrence：scheduled_at 推进一个间隔，保持 active。
    let t = store.get("schedule-1").unwrap();
    assert_eq!(t.status, TaskStatus::Active);
    let advanced = time::parse_utc_instant(t.record.scheduled_at()).unwrap();
    assert_eq!(advanced - time::parse_utc_instant(&future).unwrap(), 300_000);
    assert!(t.last_delivery.is_some());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn scheduled_message_sets_schedule_source() {
    let msg = scheduled_message("hello");
    assert_eq!(msg.role, crate::types::Role::User);
    assert_eq!(msg.source, Some(MessageSource::Schedule));
}

#[test]
fn format_utc_now_is_canonical() {
    let s = format_utc_now();
    assert!(s.ends_with('Z'));
    assert_eq!(s.len(), 24); // YYYY-MM-DDTHH:mm:ss.SSSZ
}
