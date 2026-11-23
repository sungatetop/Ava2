//! `framing` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::super::types::{AtRecord, ScheduleRecord};
use super::*;

fn at_record(id: &str, prompt: &str, scheduled_at: &str) -> ScheduleRecord {
    ScheduleRecord::At(AtRecord {
        id: id.to_string(),
        title: "t".to_string(),
        prompt: prompt.to_string(),
        scheduled_at: scheduled_at.to_string(),
    })
}

#[test]
fn reminder_framing_fixed_lines() {
    let record = at_record("schedule-abc", "drink water", "2030-01-01T00:00:00.000Z");
    let frame = render_reminder_framing(&record);
    let lines: Vec<&str> = frame.lines().collect();
    assert_eq!(lines[0], "[SCHEDULE REMINDER]");
    assert_eq!(lines[1], "This is a scheduled message from the user");
    assert_eq!(lines[2], "schedule_id_json: \"schedule-abc\"");
    assert_eq!(lines[3], "occurrence_at: 2030-01-01T00:00:00.000Z");
    // prompt 以规范 JSON 转义。
    assert_eq!(lines[4], "reminder_prompt_json: \"drink water\"");
}

#[test]
fn reminder_framing_escapes_prompt_json() {
    let record = at_record("schedule-1", "say \"hi\" & more", "2030-01-01T00:00:00.000Z");
    let frame = render_reminder_framing(&record);
    assert!(frame.contains("reminder_prompt_json: \"say \\\"hi\\\" & more\""));
}

#[test]
fn recurring_batch_framing_lists_reminders() {
    let r1 = at_record("schedule-1", "a", "2030-01-01T00:00:00.000Z");
    let r2 = at_record("schedule-2", "b", "2030-01-02T00:00:00.000Z");
    let batch = render_recurring_batch_framing(&[(&r1, "2030-01-01T00:00:00.000Z".to_string()), (&r2, "2030-01-02T00:00:00.000Z".to_string())]);
    let lines: Vec<&str> = batch.lines().collect();
    assert_eq!(lines[0], "[SCHEDULE REMINDER BATCH]");
    assert_eq!(lines[1], "This is a scheduled message from the user");
    assert!(lines[2].starts_with("reminders_json: "));
    let payload: serde_json::Value = serde_json::from_str(lines[2].strip_prefix("reminders_json: ").unwrap()).unwrap();
    let arr = payload.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["schedule_id"], "schedule-1");
    assert_eq!(arr[0]["occurrence_at"], "2030-01-01T00:00:00.000Z");
    assert_eq!(arr[0]["reminder_prompt"], "a");
    assert_eq!(arr[1]["schedule_id"], "schedule-2");
}
