//! 递送历史：收据追加剪枝 + 游标分页。
//!
//! - `append_delivery`：在唯一写路径（runtime 递送确认）上追加收据并剪枝到
//!   配置窗口与记录数上限；`lastDelivery` 恒保留；
//! - `delivery_history_of`：缺失历史时仅暴露实际 legacy 收据；
//! - `delivery_history_page`：最新优先、按 append 序分页，独立于墙钟排序。

use super::types::{
    DeliveryCursorNotFound, DeliveryHistory, DeliveryHistoryPage, DeliveryReceipt, DeliveryRecord,
    DeliveryRetentionBounds, ScheduleTask,
};

/// 一天毫秒数（保留窗口以天为单位）。
const DAY_MS: i64 = 86_400_000;

/// 读取保留历史；缺失时暴露仅有的实际 legacy 收据。
pub fn delivery_history_of(task: &ScheduleTask) -> DeliveryHistory {
    match &task.delivery_history {
        Some(history) => history.clone(),
        None => match &task.last_delivery {
            Some(receipt) => DeliveryHistory {
                records: vec![DeliveryRecord { receipt: receipt.clone(), prompt: None }],
                earlier_records_unavailable: true,
                earlier_records_pruned: None,
            },
            None => DeliveryHistory {
                records: Vec::new(),
                earlier_records_unavailable: true,
                earlier_records_pruned: None,
            },
        },
    }
}

/// 为同一任务写追加一次真实 inbox 确认，剪枝到窗口与记录数上限。
///
/// 返回收据与保留历史；调用方仅在任务持久化后发布。
pub fn append_delivery(
    task: &ScheduleTask,
    receipt: DeliveryReceipt,
    bounds: DeliveryRetentionBounds,
) -> (DeliveryReceipt, DeliveryHistory) {
    let mut history = delivery_history_of(task);
    let prompt = Some(task.record.prompt().to_string());
    history.records.push(DeliveryRecord { receipt: receipt.clone(), prompt });
    let floor = parse_ms(&receipt.delivered_at) - bounds.days * DAY_MS;
    let appended_len = history.records.len();
    history.records.retain(|record| parse_ms(&record.receipt.delivered_at) >= floor);
    let truncated = history.records.len() != appended_len;
    if history.records.len() > bounds.records {
        let keep = history.records.split_off(history.records.len() - bounds.records);
        history.records = keep;
    }
    let pruned = truncated || history.records.len() != appended_len;
    history.earlier_records_unavailable = history.earlier_records_unavailable || pruned;
    history.earlier_records_pruned = Some(history.earlier_records_pruned == Some(true) || pruned);
    (receipt, history)
}

/// 解析规范 UTC instant 为毫秒（收据字段恒为规范四位数年份）。
fn parse_ms(instant: &str) -> i64 {
    crate::schedule::time::parse_utc_instant(instant).unwrap_or(i64::MAX)
}

/// 读取一页最新优先（append 序）的递送记录。
///
/// `before` 为上一页最旧条目的消息身份（排除该条）；`limit` 为页大小。
/// 返回游标未找到结果（`delivery_cursor_not_found`）。
pub fn delivery_history_page(
    task: &ScheduleTask,
    id: &str,
    limit: usize,
    before: Option<&str>,
    retention: DeliveryRetentionBounds,
) -> Result<DeliveryHistoryPage, DeliveryCursorNotFound> {
    let history = delivery_history_of(task);
    let end = match before {
        None => history.records.len(),
        Some(cursor) => history
            .records
            .iter()
            .position(|record| record.receipt.message_id == cursor)
            .ok_or(DeliveryCursorNotFound { id: id.to_string() })?,
    };
    let start = end.saturating_sub(limit);
    let records: Vec<DeliveryRecord> = history.records[start..end].iter().rev().cloned().collect();
    let oldest = records.last();
    let next_before = if start > 0 { oldest.map(|r| r.receipt.message_id.clone()) } else { None };
    Ok(DeliveryHistoryPage {
        id: id.to_string(),
        records,
        earlier_records_unavailable: history.earlier_records_unavailable,
        earlier_records_pruned: history.earlier_records_pruned == Some(true),
        retention,
        next_before,
    })
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
