//! schedule 帧渲染：一次性与循环批次。
//!
//! - `render_reminder_framing`：一次性到期提醒的固定模型帧；
//! - `render_recurring_batch_framing`：循环批次（latest due occurrence）帧。
//!
//! 动态字段以规范 JSON 转义（`_json` 后缀）。

use super::types::ScheduleRecord;

/// 固定 framing 行。
const SCHEDULED_MESSAGE_FRAMING: &str = "This is a scheduled message from the user";

/// 渲染一次性到期提醒的固定模型帧。
pub fn render_reminder_framing(record: &ScheduleRecord) -> String {
    [
        "[SCHEDULE REMINDER]".to_string(),
        SCHEDULED_MESSAGE_FRAMING.to_string(),
        format!("schedule_id_json: {}", serde_json::to_string(record.id()).unwrap_or_default()),
        format!("occurrence_at: {}", record.scheduled_at()),
        format!("reminder_prompt_json: {}", serde_json::to_string(record.prompt()).unwrap_or_default()),
    ]
    .join("\n")
}

/// 渲染一个循环批次（按传入顺序；每 record 一个 latest occurrence）。
pub fn render_recurring_batch_framing(
    reminders: &[(&ScheduleRecord, String)],
) -> String {
    let payload: Vec<serde_json::Value> = reminders
        .iter()
        .map(|(record, occurrence_at)| {
            serde_json::json!({
                "schedule_id": record.id(),
                "occurrence_at": occurrence_at,
                "reminder_prompt": record.prompt(),
            })
        })
        .collect();
    [
        "[SCHEDULE REMINDER BATCH]".to_string(),
        SCHEDULED_MESSAGE_FRAMING.to_string(),
        format!("reminders_json: {}", serde_json::to_string(&payload).unwrap_or_default()),
    ]
    .join("\n")
}

#[cfg(test)]
#[path = "framing_tests.rs"]
mod tests;
