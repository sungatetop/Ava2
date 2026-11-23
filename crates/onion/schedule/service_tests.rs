//! `service` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//!
//! 使用临时目录下的 Host 存储，验证 Host 独立存储 CRUD（create/list/update/
//! delete/catalog/history）；id 为全局 `schedule-{uuid}`。

use std::path::PathBuf;
use std::sync::Arc;

use super::super::storage::HostScheduleStore;
use super::super::types::*;
use super::*;

/// 临时根目录（进程隔离，测试后清理）。
fn temp_root(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ava_onion_schedule_svc_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// 构造绑定到临时 Host 存储的 service。
fn service(name: &str) -> (ScheduleService, PathBuf) {
    let root = temp_root(name);
    let store = Arc::new(HostScheduleStore::open(&root).unwrap());
    (ScheduleService::new(store), root)
}

fn create_request(prompt: &str, title: &str) -> ScheduleCreateRequest {
    ScheduleCreateRequest {
        prompt: prompt.to_string(),
        title: title.to_string(),
        after_seconds: None,
        at: Some(AtInput::Instant("2030-01-01T00:00:00Z".to_string())),
        every_seconds: None,
        daily: None,
        weekly: None,
        cron: None,
    }
}

#[test]
fn create_assigns_global_uuid_id_and_persists() {
    let (svc, root) = service("create_id");
    let record = svc.create("s1", create_request("hi", "t")).unwrap();
    // 全局 uuid id，非会话内递增。
    assert!(record.id().starts_with("schedule-"));
    assert_ne!(record.id(), "schedule-1");
    assert_eq!(svc.list("s1").len(), 1);
    // Host 存储落盘。
    let reopened = HostScheduleStore::open(&root).unwrap();
    assert_eq!(reopened.all().len(), 1);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn create_rejects_multiple_selectors() {
    let (svc, root) = service("multi_sel");
    let mut req = create_request("hi", "t");
    req.after_seconds = Some(60);
    assert!(matches!(svc.create("s1", req), Err(ScheduleError::InvalidSelector(_))));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn create_rejects_non_future_at() {
    let (svc, root) = service("non_future");
    let mut req = create_request("hi", "t");
    req.at = Some(AtInput::Instant("2000-01-01T00:00:00Z".to_string()));
    assert_eq!(svc.create("s1", req), Err(ScheduleError::NotFuture));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn list_filters_by_session_and_active() {
    let (svc, root) = service("list_filter");
    let r1 = svc.create("s1", create_request("a", "ta")).unwrap();
    let r2 = svc.create("s1", create_request("b", "tb")).unwrap();
    svc.create("s2", create_request("c", "tc")).unwrap();
    // s1 只有 2 条。
    assert_eq!(svc.list("s1").len(), 2);
    assert_eq!(svc.list("s2").len(), 1);
    // 删除 s1 的一条 → 只剩 1 条 active。
    svc.delete("s1", r1.id());
    assert_eq!(svc.list("s1").len(), 1);
    assert_eq!(svc.list("s1")[0].id(), r2.id());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn delete_owned_but_not_foreign() {
    let (svc, root) = service("delete_owned");
    let r = svc.create("s1", create_request("a", "ta")).unwrap();
    let id = r.id().to_string();
    // 非属主 session 删除 → NotFound。
    assert_eq!(svc.delete("s2", &id), ScheduleDeleteResult::NotFound { id: id.clone() });
    // 属主删除 → Deleted。
    assert_eq!(svc.delete("s1", &id), ScheduleDeleteResult::Deleted { id: id.clone() });
    // 再删 → NotFound。
    assert_eq!(svc.delete("s1", &id), ScheduleDeleteResult::NotFound { id });
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn update_content_keeps_id() {
    let (svc, root) = service("update_content");
    let r = svc.create("s1", create_request("hi", "t")).unwrap();
    let id = r.id().to_string();
    let current = svc.find(&id).unwrap();
    let res = svc.update(
        "s1",
        &id,
        &current,
        None,
        ScheduleUpdateContent { title: Some("new title".to_string()), prompt: None },
        InactivePolicy::Reject,
    );
    match res {
        ScheduleUpdateResult::Updated { record } => {
            assert_eq!(record.id(), &id);
            assert_eq!(record.title(), "new title");
            assert_eq!(record.prompt(), "hi");
        }
        other => panic!("expected Updated, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn update_conflict_detected() {
    let (svc, root) = service("update_conflict");
    let r = svc.create("s1", create_request("hi", "t")).unwrap();
    let id = r.id().to_string();
    // 用错误的 expected（伪造不同 title）触发 Conflict。
    let mut wrong = svc.find(&id).unwrap();
    wrong = match wrong {
        ScheduleRecord::At(mut x) => { x.title = "other".to_string(); ScheduleRecord::At(x) }
        other => other,
    };
    let res = svc.update("s1", &id, &wrong, None, ScheduleUpdateContent::default(), InactivePolicy::Reject);
    assert!(matches!(res, ScheduleUpdateResult::Conflict { .. }));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn update_not_found_and_foreign() {
    let (svc, root) = service("update_notfound");
    // 不存在的 id → NotFound。
    let res = svc.update("s1", "schedule-zzz", &schedule_record_placeholder(), None, ScheduleUpdateContent::default(), InactivePolicy::Reject);
    assert!(matches!(res, ScheduleUpdateResult::NotFound { .. }));
    // 存在但属主不符 → NotFound。
    let r = svc.create("s1", create_request("hi", "t")).unwrap();
    let id = r.id().to_string();
    let current = svc.find(&id).unwrap();
    let res = svc.update("s2", &id, &current, None, ScheduleUpdateContent::default(), InactivePolicy::Reject);
    assert!(matches!(res, ScheduleUpdateResult::NotFound { .. }));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn update_inactive_task_respects_policy() {
    let root = temp_root("update_inactive");
    let store = Arc::new(HostScheduleStore::open(&root).unwrap());
    let svc = ScheduleService::new(store.clone());
    let r = svc.create("s1", create_request("hi", "t")).unwrap();
    let id = r.id().to_string();
    let task = store.get(&id).unwrap();
    store.put(ScheduleTask { status: TaskStatus::Inactive, ..task }).unwrap();
    let current = svc.find(&id).unwrap();

    // Reject（模型侧工具）：停用视为已结束。
    let res = svc.update(
        "s1",
        &id,
        &current,
        None,
        ScheduleUpdateContent { title: Some("x".to_string()), prompt: None },
        InactivePolicy::Reject,
    );
    assert!(matches!(res, ScheduleUpdateResult::Ended { .. }));

    // Allow（产品侧编辑）：可改内容，停用状态保持不变。
    let res = svc.update(
        "s1",
        &id,
        &current,
        None,
        ScheduleUpdateContent { title: Some("新标题".to_string()), prompt: None },
        InactivePolicy::Allow,
    );
    match res {
        ScheduleUpdateResult::Updated { record } => assert_eq!(record.title(), "新标题"),
        other => panic!("expected Updated, got {other:?}"),
    }
    assert_eq!(store.get(&id).unwrap().status, TaskStatus::Inactive);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn update_timing_change_recomputes_target() {
    let (svc, root) = service("update_timing");
    let r = svc.create("s1", create_request("hi", "t")).unwrap();
    let id = r.id().to_string();
    let current = svc.find(&id).unwrap();
    // 改成 every（循环），id 稳定。
    let res = svc.update(
        "s1",
        &id,
        &current,
        Some(ScheduleTimingChange::Every { every_seconds: 60 }),
        ScheduleUpdateContent::default(),
        InactivePolicy::Reject,
    );
    match res {
        ScheduleUpdateResult::Updated { record } => {
            assert_eq!(record.id(), &id);
            assert!(matches!(record, ScheduleRecord::Every(_)));
        }
        other => panic!("expected Updated, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn catalog_returns_all_tasks_with_status() {
    let (svc, root) = service("catalog");
    let r = svc.create("s1", create_request("hi", "t")).unwrap();
    let id = r.id().to_string();
    assert_eq!(svc.catalog().len(), 1);
    let task = svc.task(&id).unwrap();
    assert_eq!(task.session_id, "s1");
    assert_eq!(task.status, TaskStatus::Active);
    // 新任务带空递送历史。
    assert!(task.delivery_history.is_some());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn history_validates_limit_and_ownership() {
    let (svc, root) = service("history");
    let r = svc.create("s1", create_request("hi", "t")).unwrap();
    let id = r.id().to_string();
    // limit 越界。
    assert_eq!(svc.history("s1", &id, 0, None), Err(DeliveryHistoryError::InvalidLimit));
    assert_eq!(svc.history("s1", &id, 101, None), Err(DeliveryHistoryError::InvalidLimit));
    // 属主不符 → NotFound。
    assert_eq!(svc.history("s2", &id, 10, None), Err(DeliveryHistoryError::NotFound));
    // 正常空历史页。
    let page = svc.history("s1", &id, 10, None).unwrap();
    assert!(page.records.is_empty());
    assert_eq!(page.id, id);
    let _ = std::fs::remove_dir_all(&root);
}

/// 占位 record：仅用于 update not-found 的 expected 参数。
fn schedule_record_placeholder() -> ScheduleRecord {
    ScheduleRecord::At(AtRecord {
        id: "schedule-x".to_string(),
        title: "t".to_string(),
        prompt: "p".to_string(),
        scheduled_at: "2030-01-01T00:00:00.000Z".to_string(),
    })
}
