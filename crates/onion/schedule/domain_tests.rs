//! `domain` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::super::time;
use super::super::types::*;
use super::*;

#[test]
fn schedule_title_trims_and_bounds() {
    assert_eq!(schedule_title("  done  ").unwrap(), "done");
    assert!(matches!(
        schedule_title("   "),
        Err(ScheduleError::InvalidPrompt(_))
    ));
    let long = "x".repeat(MAX_TITLE_LENGTH + 1);
    assert!(matches!(schedule_title(&long), Err(ScheduleError::InvalidPrompt(_))));
}

#[test]
fn create_after_future_and_not_future() {
    let now = 1_700_000_000_000;
    let record = create_after_schedule_record(
        "schedule-1".to_string(),
        "hello",
        60,
        now,
        "title",
    )
    .unwrap();
    assert_eq!(record.after_seconds, 60);
    assert_eq!(record.scheduled_at, time::format_utc(now + 60_000));
    // after_seconds <= 0 拒绝。
    assert!(matches!(
        create_after_schedule_record("schedule-1".to_string(), "hi", 0, now, "t"),
        Err(ScheduleError::InvalidRule(_))
    ));
}

#[test]
fn create_at_parses_instant_and_local() {
    let now = 1_700_000_000_000;
    let instant = AtInput::Instant("2030-01-01T00:00:00Z".to_string());
    let record = create_at_schedule_record("schedule-1".to_string(), "hi", &instant, now, "t").unwrap();
    // 2030 远在未来。
    assert!(time::parse_utc_instant(&record.scheduled_at).unwrap() > now);

    // 本地日历 + Asia/Shanghai。
    let local = AtInput::Local {
        date: "2030-01-01".to_string(),
        time: "08:00:00".to_string(),
        time_zone: "Asia/Shanghai".to_string(),
    };
    let rec = create_at_schedule_record("schedule-1".to_string(), "hi", &local, now, "t").unwrap();
    // 上海 2030-01-01 08:00 = UTC 2030-01-01 00:00。
    assert_eq!(rec.scheduled_at, "2030-01-01T00:00:00.000Z");
}

#[test]
fn create_every_minimum_interval() {
    let now = 1_700_000_000_000;
    let rec = create_every_schedule_record("schedule-1".to_string(), "hi", 60, now, "t").unwrap();
    assert_eq!(rec.every_seconds, 60);
    assert_eq!(rec.scheduled_at, time::format_utc(now + 60_000));
    // 低于 60 拒绝（frequency_too_high）。
    assert_eq!(
        create_every_schedule_record("schedule-1".to_string(), "hi", 30, now, "t"),
        Err(ScheduleError::FrequencyTooHigh)
    );
}

#[test]
fn normalize_weekdays_unique_ascending() {
    assert_eq!(normalize_weekdays(&[5, 1, 3]).unwrap(), vec![1, 3, 5]);
    assert!(normalize_weekdays(&[5, 1, 5]).is_err());
    assert!(normalize_weekdays(&[]).is_err());
    assert!(normalize_weekdays(&[0]).is_err());
    assert!(normalize_weekdays(&[8]).is_err());
}

#[test]
fn create_daily_weekly_cron_records() {
    let now = 1_700_000_000_000;
    let daily = DailyInput { time: "09:00:00".to_string(), time_zone: "Asia/Shanghai".to_string() };
    let d = create_daily_schedule_record("schedule-1".to_string(), "hi", &daily, now, "t").unwrap();
    assert_eq!(d.time, "09:00:00.000");
    assert_eq!(d.time_zone, "Asia/Shanghai");
    assert!(time::parse_utc_instant(&d.scheduled_at).unwrap() > now);

    let weekly = WeeklyInput {
        time: "09:00:00".to_string(),
        time_zone: "Asia/Shanghai".to_string(),
        weekdays: vec![1, 3, 5],
    };
    let w = create_weekly_schedule_record("schedule-1".to_string(), "hi", &weekly, now, "t").unwrap();
    assert_eq!(w.weekdays, vec![1, 3, 5]);
    assert!(time::parse_utc_instant(&w.scheduled_at).unwrap() > now);

    let cron = CronInput {
        expression: "0 9 * * 1-5".to_string(),
        time_zone: "Asia/Shanghai".to_string(),
    };
    let c = create_cron_schedule_record("schedule-1".to_string(), "hi", &cron, now, "t").unwrap();
    // 规范化：`1-5` 保持。
    assert_eq!(c.expression, "0 9 * * 1-5");
    assert!(time::parse_utc_instant(&c.scheduled_at).unwrap() > now);
}

#[test]
fn resolve_every_occurrence_latest_and_next() {
    let now = 1_700_000_000_000;
    let rec = create_every_schedule_record("schedule-1".to_string(), "hi", 60, now, "t").unwrap();
    // 决策在首次目标之后 125s：latest due = 首次 + 60s；next = +120s。
    let target = time::parse_utc_instant(&rec.scheduled_at).unwrap();
    let decision = target + 125_000;
    let occ = resolve_every_occurrence(&rec, decision).unwrap();
    // latest due occurrence = 首次 + 120s（最接近但不晚于 125s）；next = +180s。
    assert_eq!(
        time::parse_utc_instant(&occ.occurrence_at).unwrap(),
        target + 120_000
    );
    assert_eq!(
        time::parse_utc_instant(&occ.next_scheduled_at.unwrap()).unwrap(),
        target + 180_000
    );
}

#[test]
fn new_schedule_id_global_unique_prefixed() {
    let a = new_schedule_id();
    let b = new_schedule_id();
    assert!(a.starts_with("schedule-"));
    assert_ne!(a, b);
    // id 段是合法 UUID（36 字符 + 连字符）。
    let suffix = a.strip_prefix("schedule-").unwrap();
    assert_eq!(suffix.len(), 36);
    assert!(uuid::Uuid::parse_str(suffix).is_ok());
}

#[test]
fn schedule_view_state() {
    let now = 1_700_000_000_000;
    let rec = create_after_schedule_record("schedule-1".to_string(), "hi", 60, now, "t").unwrap();
    let v = schedule_view(&ScheduleRecord::After(rec), now);
    assert_eq!(v.state, ScheduleState::Scheduled);
    assert_eq!(v.delivery_mode, "host");

    // 到期后 → Overdue。
    let rec2 = create_after_schedule_record("schedule-1".to_string(), "hi", 60, now, "t").unwrap();
    let v2 = schedule_view(&ScheduleRecord::After(rec2), now + 120_000);
    assert_eq!(v2.state, ScheduleState::Overdue);
}
