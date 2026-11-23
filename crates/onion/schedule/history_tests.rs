//! `history` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::super::types::{
    DeliveryHistory, DeliveryReceipt, DeliveryRetentionBounds, ScheduleRecord, ScheduleTask,
    TaskStatus,
};
use super::*;

/// 构造一个 At 一次性任务（带空历史）。
fn task(id: &str, prompt: &str) -> ScheduleTask {
    ScheduleTask {
        session_id: "s1".to_string(),
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

fn receipt(message_id: &str, delivered_at: &str) -> DeliveryReceipt {
    DeliveryReceipt {
        scheduled_at: "2030-01-01T00:00:00.000Z".to_string(),
        delivered_at: delivered_at.to_string(),
        message_id: message_id.to_string(),
    }
}

#[test]
fn delivery_history_of_missing_exposes_legacy_receipt() {
    // 无 delivery_history，仅 last_delivery。
    let mut t = task("s-1", "p");
    t.delivery_history = None;
    t.last_delivery = Some(receipt("m1", "2030-01-02T00:00:00.000Z"));
    let h = delivery_history_of(&t);
    assert_eq!(h.records.len(), 1);
    assert_eq!(h.records[0].receipt.message_id, "m1");
    assert!(h.earlier_records_unavailable);
}

#[test]
fn append_delivery_adds_and_sets_flags() {
    let t = task("s-1", "p");
    let (receipt, history) = append_delivery(
        &t,
        receipt("m1", "2030-01-02T00:00:00.000Z"),
        DeliveryRetentionBounds { days: 30, records: 200 },
    );
    assert_eq!(history.records.len(), 1);
    assert_eq!(history.records[0].receipt.message_id, "m1");
    assert_eq!(history.records[0].prompt.as_deref(), Some("p"));
    assert_eq!(receipt.message_id, "m1");
    // 未剪枝：earlier_unavailable 保持 false。
    assert!(!history.earlier_records_unavailable);
}

#[test]
fn append_delivery_prunes_by_window() {
    let bounds = DeliveryRetentionBounds { days: 30, records: 200 };
    // 旧收据（2020 年，超过 30 天窗口）。
    let old = receipt("old", "2020-01-01T00:00:00.000Z");
    // 先造一个带 old 的历史。
    let mut base = task("s-1", "p");
    base.delivery_history = Some(DeliveryHistory {
        records: vec![super::super::types::DeliveryRecord { receipt: old, prompt: None }],
        earlier_records_unavailable: false,
        earlier_records_pruned: None,
    });
    // 追加 new（2030 年）→ old 超出窗口被剪除。
    let new = receipt("new", "2030-01-02T00:00:00.000Z");
    let (_, history) = append_delivery(&base, new, bounds);
    // old 超出窗口被剪除，仅剩 new；earlier_unavailable 置 true。
    assert_eq!(history.records.len(), 1);
    assert_eq!(history.records[0].receipt.message_id, "new");
    assert!(history.earlier_records_unavailable);
    assert_eq!(history.earlier_records_pruned, Some(true));
}

#[test]
fn append_delivery_prunes_by_record_count() {
    let t = task("s-1", "p");
    let bounds = DeliveryRetentionBounds { days: 30, records: 2 };
    let (_, h1) = append_delivery(&t, receipt("m1", "2030-01-01T00:00:00.000Z"), bounds);
    let mut t2 = t.clone();
    t2.delivery_history = Some(h1);
    let (_, h2) = append_delivery(&t2, receipt("m2", "2030-01-02T00:00:00.000Z"), bounds);
    let mut t3 = t;
    t3.delivery_history = Some(h2);
    let (_, h3) = append_delivery(&t3, receipt("m3", "2030-01-03T00:00:00.000Z"), bounds);
    // 保留最新 2 条：m3, m2。
    assert_eq!(h3.records.len(), 2);
    assert_eq!(h3.records[0].receipt.message_id, "m2");
    assert_eq!(h3.records[1].receipt.message_id, "m3");
    assert!(h3.earlier_records_unavailable);
}

#[test]
fn delivery_page_latest_first() {
    let mut t = task("s-1", "p");
    // 构造 3 条递送历史（append 序：m1, m2, m3）。
    let bounds = DeliveryRetentionBounds { days: 30, records: 200 };
    let (_, h1) = append_delivery(&t, receipt("m1", "2030-01-01T00:00:00.000Z"), bounds);
    t.delivery_history = Some(h1);
    let (_, h2) = append_delivery(&t, receipt("m2", "2030-01-02T00:00:00.000Z"), bounds);
    t.delivery_history = Some(h2);
    let (_, h3) = append_delivery(&t, receipt("m3", "2030-01-03T00:00:00.000Z"), bounds);
    t.delivery_history = Some(h3);

    // 首页 limit=2：最新优先 [m3, m2]，next_before = m2。
    let page = delivery_history_page(&t, "s-1", 2, None, bounds).unwrap();
    assert_eq!(page.records.len(), 2);
    assert_eq!(page.records[0].receipt.message_id, "m3");
    assert_eq!(page.records[1].receipt.message_id, "m2");
    assert_eq!(page.next_before.as_deref(), Some("m2"));

    // 第二页 before=m2：仅剩 [m1]。
    let page2 = delivery_history_page(&t, "s-1", 2, Some("m2"), bounds).unwrap();
    assert_eq!(page2.records.len(), 1);
    assert_eq!(page2.records[0].receipt.message_id, "m1");
    assert_eq!(page2.next_before, None);
}

#[test]
fn delivery_page_cursor_not_found() {
    let t = task("s-1", "p");
    let bounds = DeliveryRetentionBounds::default();
    let err = delivery_history_page(&t, "s-1", 10, Some("missing"), bounds).unwrap_err();
    assert_eq!(err, DeliveryCursorNotFound { id: "s-1".to_string() });
}
