//! schedule 域逻辑：record 创建校验、全局 id、时序决策。
//!
//! - 六种 record 创建（after/at/every/daily/weekly/cron）与 `futureInstant`
//!   校验（`not_future` / `time_out_of_range`）；
//! - `at` 选择器解析（显式偏移字符串或结构化本地日历）；
//! - daily/weekly/cron 选择器规范化（IANA 时区 + ISO 工作日 + 五段 cron）；
//! - recurring 决策（`resolveEvery/Daily/Weekly/CronOccurrence`）：
//!   latest due occurrence + next future target；
//! - 全局唯一 id 分配 `new_schedule_id`（`schedule-{uuid}`）。

use std::collections::BTreeSet;

use chrono::{NaiveDate, NaiveTime};
use chrono_tz::Tz;

use super::cron::{self, ParsedCron};
use super::time;
use super::types::{
    AfterRecord, AtInput, AtRecord, CronInput, CronRecord, DailyInput, DailyRecord, EveryRecord,
    RecurringOccurrence, ScheduleDeleteResult, ScheduleError, ScheduleId, ScheduleRecord,
    ScheduleState, ScheduleUpdateResult, ScheduleView, WeeklyInput, WeeklyRecord,
    CRON_SEARCH_HORIZON_YEARS, MAX_TITLE_LENGTH, MIN_EVERY_INTERVAL_SECONDS,
};

/// 校验任务名：trim、非空、≤ `MAX_TITLE_LENGTH`。
pub fn schedule_title(title: &str) -> Result<String, ScheduleError> {
    if title.trim().is_empty() {
        return Err(ScheduleError::InvalidPrompt(
            "title is required and must be non-empty after trimming.".to_string(),
        ));
    }
    let normalized = title.trim();
    if normalized.chars().count() > MAX_TITLE_LENGTH {
        return Err(ScheduleError::InvalidPrompt(format!(
            "title must be at most {MAX_TITLE_LENGTH} characters."
        )));
    }
    Ok(normalized.to_string())
}

/// 校验提醒指令：trim、非空。
pub fn schedule_prompt(prompt: &str) -> Result<String, ScheduleError> {
    let normalized = prompt.trim();
    if normalized.is_empty() {
        return Err(ScheduleError::InvalidPrompt(
            "prompt must be non-empty after trimming.".to_string(),
        ));
    }
    Ok(normalized.to_string())
}

/// 规范化 IANA 时区为规范名（`UTC` 或 `Area/Location`）。
pub fn canonicalize_time_zone(value: &str) -> Result<String, ScheduleError> {
    let tz = time::parse_time_zone(value)?;
    Ok(tz.name().to_string())
}

/// 强制未来且可表示：范围检查 + 严格未来。
fn future_instant(epoch: i64, now: i64) -> Result<String, ScheduleError> {
    if !(time::MIN_FOUR_DIGIT_YEAR_MS..=time::MAX_FOUR_DIGIT_YEAR_MS).contains(&epoch) {
        return Err(ScheduleError::TimeOutOfRange);
    }
    if epoch <= now {
        return Err(ScheduleError::NotFuture);
    }
    Ok(time::format_utc(epoch))
}

/// 解析 `at` 选择器为 epoch 毫秒（不要求未来）。
pub fn parse_at_input(at: &AtInput) -> Result<i64, ScheduleError> {
    match at {
        AtInput::Instant(value) => time::parse_offset_instant(value),
        AtInput::Local { date, time: t, time_zone } => {
            let tz = time::parse_time_zone(time_zone)?;
            let date = time::parse_local_date(date)?;
            let time = time::parse_local_time(t)?;
            time::local_instant(date, time, tz).ok_or_else(|| {
                ScheduleError::InvalidRule(
                    "The local at time does not exist in the selected time zone.".to_string(),
                )
            })
        }
    }
}

/// 创建一次性延时提醒。
pub fn create_after_schedule_record(
    id: ScheduleId,
    prompt: &str,
    after_seconds: i64,
    now: i64,
    title: &str,
) -> Result<AfterRecord, ScheduleError> {
    let normalized_prompt = schedule_prompt(prompt)?;
    if after_seconds <= 0 {
        return Err(ScheduleError::InvalidRule(
            "after_seconds must be a positive safe integer.".to_string(),
        ));
    }
    let delay = after_seconds
        .checked_mul(1000)
        .ok_or(ScheduleError::TimeOutOfRange)?;
    let target = now.checked_add(delay).ok_or(ScheduleError::TimeOutOfRange)?;
    Ok(AfterRecord {
        id,
        title: schedule_title(title)?,
        prompt: normalized_prompt,
        after_seconds,
        scheduled_at: future_instant(target, now)?,
    })
}

/// 创建一次性绝对时刻提醒。
pub fn create_at_schedule_record(
    id: ScheduleId,
    prompt: &str,
    at: &AtInput,
    now: i64,
    title: &str,
) -> Result<AtRecord, ScheduleError> {
    Ok(AtRecord {
        id,
        title: schedule_title(title)?,
        prompt: schedule_prompt(prompt)?,
        scheduled_at: future_instant(parse_at_input(at)?, now)?,
    })
}

/// 创建固定间隔循环提醒。
pub fn create_every_schedule_record(
    id: ScheduleId,
    prompt: &str,
    every_seconds: i64,
    now: i64,
    title: &str,
) -> Result<EveryRecord, ScheduleError> {
    let normalized_prompt = schedule_prompt(prompt)?;
    if every_seconds < MIN_EVERY_INTERVAL_SECONDS {
        return Err(ScheduleError::FrequencyTooHigh);
    }
    let interval = every_seconds
        .checked_mul(1000)
        .ok_or(ScheduleError::TimeOutOfRange)?;
    let target = now.checked_add(interval).ok_or(ScheduleError::TimeOutOfRange)?;
    Ok(EveryRecord {
        id,
        title: schedule_title(title)?,
        prompt: normalized_prompt,
        every_seconds,
        scheduled_at: future_instant(target, now)?,
    })
}

/// 规范化一个 ISO 工作日集合：非空、1..=7、无重复、升序。
pub fn normalize_weekdays(weekdays: &[u32]) -> Result<Vec<u32>, ScheduleError> {
    if weekdays.is_empty() {
        return Err(ScheduleError::InvalidRule(
            "weekly.weekdays must be a non-empty array of ISO weekday numbers.".to_string(),
        ));
    }
    let mut unique = BTreeSet::new();
    for &weekday in weekdays {
        if !(1..=7).contains(&weekday) {
            return Err(ScheduleError::InvalidRule(
                "Each weekly.weekdays entry must be an integer from 1 (Monday) through 7 (Sunday)."
                    .to_string(),
            ));
        }
        if !unique.insert(weekday) {
            return Err(ScheduleError::InvalidRule(format!(
                "weekly.weekdays must not repeat weekday {weekday}."
            )));
        }
    }
    Ok(unique.into_iter().collect())
}

/// 匹配谓词：该日期命中显式 ISO 工作日集合。
fn weekday_set(weekdays: &[u32]) -> impl Fn(NaiveDate) -> bool + '_ {
    move |date| weekdays.contains(&time::iso_weekday(date))
}

/// 匹配谓词：命中所有日期（daily 按时间单独选择）。
fn every_date(_date: NaiveDate) -> bool {
    true
}

/// 从 `now`（含 `after_date` 之后）找到第一个严格未来的匹配目标。
fn next_matching_target<F>(
    time: NaiveTime,
    tz: Tz,
    now: i64,
    matches: F,
    after_date: Option<NaiveDate>,
) -> Option<String>
where
    F: Fn(NaiveDate) -> bool,
{
    let mut date = time::local_date(now, tz);
    let last_date = time::add_days(time::local_date(time::MAX_FOUR_DIGIT_YEAR_MS, Tz::UTC), 1);
    if let Some(after) = after_date {
        if date <= after {
            date = time::add_days(after, 1);
        }
    }
    while date <= last_date {
        if matches(date) {
            if let Some(target) = time::local_instant(date, time, tz) {
                if target > time::MAX_FOUR_DIGIT_YEAR_MS {
                    return None;
                }
                if target >= time::MIN_FOUR_DIGIT_YEAR_MS && target > now {
                    return Some(time::format_utc(target));
                }
            }
        }
        date = time::add_days(date, 1);
    }
    None
}

/// 找到 `accepted_at` 之前（含）最新实际到期 occurrence，且不低于已提交目标。
fn latest_due_occurrence<F>(
    tz: Tz,
    matches: F,
    time: NaiveTime,
    accepted_at: i64,
    saved_target: i64,
) -> i64
where
    F: Fn(NaiveDate) -> bool,
{
    // 偏移 < 24h；日期线回滚可能使当前本地日期早于到期日，故从 UTC 决策日 +1 天往回走。
    let mut date = time::add_days(time::local_date(accepted_at, Tz::UTC), 1);
    loop {
        if matches(date) {
            if let Some(candidate) = time::local_instant(date, time, tz) {
                if candidate <= accepted_at {
                    return candidate.max(saved_target);
                }
            }
        }
        date = time::add_days(date, -1);
    }
}

/// daily 目标：按时间每天匹配。
fn next_daily_target(time: NaiveTime, tz: Tz, now: i64, after_date: Option<NaiveDate>) -> Option<String> {
    next_matching_target(time, tz, now, every_date, after_date)
}

/// weekly 目标：按时间 + 工作日集合匹配。
fn next_weekly_target(
    time: NaiveTime,
    tz: Tz,
    weekdays: &[u32],
    now: i64,
    after_date: Option<NaiveDate>,
) -> Option<String> {
    next_matching_target(time, tz, now, weekday_set(weekdays), after_date)
}

/// 规范化 daily 选择器（不计算新目标）。
pub fn parse_daily_input(daily: &DailyInput) -> Result<(String, String), ScheduleError> {
    let time = time::normalize_clock_time(&daily.time, "daily")?;
    let time_zone = canonicalize_time_zone(&daily.time_zone)?;
    Ok((time, time_zone))
}

/// 创建每日墙钟提醒。
pub fn create_daily_schedule_record(
    id: ScheduleId,
    prompt: &str,
    daily: &DailyInput,
    now: i64,
    title: &str,
) -> Result<DailyRecord, ScheduleError> {
    let normalized_prompt = schedule_prompt(prompt)?;
    let (time, time_zone) = parse_daily_input(daily)?;
    if !(time::MIN_FOUR_DIGIT_YEAR_MS..=time::MAX_FOUR_DIGIT_YEAR_MS).contains(&now) {
        return Err(ScheduleError::TimeOutOfRange);
    }
    let tz = time::parse_time_zone(&time_zone)?;
    let scheduled_at = next_daily_target(time::parse_clock_time(&time, "daily")?, tz, now, None)
        .ok_or(ScheduleError::TimeOutOfRange)?;
    Ok(DailyRecord {
        id,
        title: schedule_title(title)?,
        prompt: normalized_prompt,
        time,
        time_zone,
        scheduled_at,
    })
}

/// 规范化 weekly 选择器（不计算新目标）。
pub fn parse_weekly_input(
    weekly: &WeeklyInput,
) -> Result<(String, String, Vec<u32>), ScheduleError> {
    let time = time::normalize_clock_time(&weekly.time, "weekly")?;
    let time_zone = canonicalize_time_zone(&weekly.time_zone)?;
    let weekdays = normalize_weekdays(&weekly.weekdays)?;
    Ok((time, time_zone, weekdays))
}

/// 创建每周墙钟提醒。
pub fn create_weekly_schedule_record(
    id: ScheduleId,
    prompt: &str,
    weekly: &WeeklyInput,
    now: i64,
    title: &str,
) -> Result<WeeklyRecord, ScheduleError> {
    let normalized_prompt = schedule_prompt(prompt)?;
    let (time, time_zone, weekdays) = parse_weekly_input(weekly)?;
    if !(time::MIN_FOUR_DIGIT_YEAR_MS..=time::MAX_FOUR_DIGIT_YEAR_MS).contains(&now) {
        return Err(ScheduleError::TimeOutOfRange);
    }
    let tz = time::parse_time_zone(&time_zone)?;
    let scheduled_at =
        next_weekly_target(time::parse_clock_time(&time, "weekly")?, tz, &weekdays, now, None)
            .ok_or(ScheduleError::TimeOutOfRange)?;
    Ok(WeeklyRecord {
        id,
        title: schedule_title(title)?,
        prompt: normalized_prompt,
        time,
        time_zone,
        weekdays,
        scheduled_at,
    })
}

/// 生成一个 cron 表达式的本地时刻列表（wall-clock 升序）。
fn cron_times(parsed: &ParsedCron) -> Vec<NaiveTime> {
    let mut times = Vec::new();
    for hour in &parsed.hours {
        for minute in &parsed.minutes {
            if let Some(t) = NaiveTime::from_hms_opt(*hour as u32, *minute as u32, 0) {
                times.push(t);
            }
        }
    }
    times
}

/// 找到 `floor` 之后第一个严格未来的 cron occurrence。
fn next_cron_target(parsed: &ParsedCron, tz: Tz, floor: i64) -> Option<String> {
    let times = cron_times(parsed);
    let first_date = time::local_date(floor, tz);
    let floor_time = time::local_time_of(floor, tz);
    let ceiling = time::add_days(time::local_date(time::MAX_FOUR_DIGIT_YEAR_MS, Tz::UTC), 1);
    let horizon = time::add_years(first_date, CRON_SEARCH_HORIZON_YEARS);
    let last_date = if horizon < ceiling { horizon } else { ceiling };
    let mut date = first_date;
    while date <= last_date {
        if !parsed.months.contains(&(time::month_of(date) as i64)) {
            date = time::next_month_first(date);
            continue;
        }
        if cron::date_matches(parsed, date) {
            for t in &times {
                if date == first_date && *t < floor_time {
                    continue;
                }
                if let Some(target) = time::local_instant(date, *t, tz) {
                    if target > time::MAX_FOUR_DIGIT_YEAR_MS {
                        return None;
                    }
                    if target >= time::MIN_FOUR_DIGIT_YEAR_MS && target > floor {
                        return Some(time::format_utc(target));
                    }
                }
            }
        }
        date = time::add_days(date, 1);
    }
    None
}

/// 找到 `decision` 之前（含）最新 cron occurrence，且不低于已提交目标。
fn latest_cron_occurrence(parsed: &ParsedCron, tz: Tz, decision: i64, saved_target: i64) -> i64 {
    let times = cron_times(parsed);
    let mut date = time::add_days(time::local_date(decision, Tz::UTC), 1);
    let floor_date = time::local_date(time::MIN_FOUR_DIGIT_YEAR_MS, tz);
    let horizon = time::add_years(date, -CRON_SEARCH_HORIZON_YEARS);
    let first_date = if horizon > floor_date { horizon } else { floor_date };
    while date >= first_date {
        if !parsed.months.contains(&(time::month_of(date) as i64)) {
            date = time::add_days(time::month_first(date), -1);
            continue;
        }
        if cron::date_matches(parsed, date) {
            // 该日期最早可转换时刻若已晚于决策，则整日无候选，直接回退。
            let mut earliest: Option<i64> = None;
            for t in &times {
                if earliest.is_none() {
                    earliest = time::local_instant(date, *t, tz);
                }
            }
            if let Some(e) = earliest {
                if e > decision {
                    date = time::add_days(date, -1);
                    continue;
                }
            }
            for t in times.iter().rev() {
                if let Some(candidate) = time::local_instant(date, *t, tz) {
                    if candidate > decision || candidate < time::MIN_FOUR_DIGIT_YEAR_MS {
                        continue;
                    }
                    return candidate.max(saved_target);
                }
            }
        }
        date = time::add_days(date, -1);
    }
    saved_target
}

/// 规范化 cron 选择器（不计算新目标）。
pub fn parse_cron_input(cron: &CronInput) -> Result<(String, String), ScheduleError> {
    let expression = cron::parse_cron_expression(&cron.expression)?.expression;
    let time_zone = canonicalize_time_zone(&cron.time_zone)?;
    Ok((expression, time_zone))
}

/// 创建五段 cron 墙钟提醒。
pub fn create_cron_schedule_record(
    id: ScheduleId,
    prompt: &str,
    cron: &CronInput,
    now: i64,
    title: &str,
) -> Result<CronRecord, ScheduleError> {
    let normalized_prompt = schedule_prompt(prompt)?;
    let (expression, time_zone) = parse_cron_input(cron)?;
    if !(time::MIN_FOUR_DIGIT_YEAR_MS..=time::MAX_FOUR_DIGIT_YEAR_MS).contains(&now) {
        return Err(ScheduleError::TimeOutOfRange);
    }
    let tz = time::parse_time_zone(&time_zone)?;
    let parsed = cron::parse_cron_expression(&expression)?;
    let scheduled_at = next_cron_target(&parsed, tz, now).ok_or(ScheduleError::TimeOutOfRange)?;
    Ok(CronRecord {
        id,
        title: schedule_title(title)?,
        prompt: normalized_prompt,
        expression,
        time_zone,
        scheduled_at,
    })
}

/// 固定间隔决策。
pub fn resolve_every_occurrence(
    record: &EveryRecord,
    accepted_at: i64,
) -> Result<RecurringOccurrence, ScheduleError> {
    let target = time::parse_utc_instant(&record.scheduled_at).ok_or(ScheduleError::Internal)?;
    let interval = record.every_seconds.checked_mul(1000).ok_or(ScheduleError::Internal)?;
    if interval <= 0 {
        return Err(ScheduleError::Internal);
    }
    if accepted_at < target {
        return Err(ScheduleError::Internal); // dispatch cannot precede the committed target
    }
    let steps = (accepted_at - target) / interval;
    let occurrence = target + steps * interval;
    let occurrence_at = time::format_utc(occurrence);
    let next = occurrence + interval;
    if next > time::MAX_FOUR_DIGIT_YEAR_MS {
        return Ok(RecurringOccurrence { occurrence_at, next_scheduled_at: None });
    }
    Ok(RecurringOccurrence {
        occurrence_at,
        next_scheduled_at: Some(time::format_utc(next)),
    })
}

/// 每日决策。
pub fn resolve_daily_occurrence(
    record: &DailyRecord,
    accepted_at: i64,
) -> Result<RecurringOccurrence, ScheduleError> {
    let saved_target = time::parse_utc_instant(&record.scheduled_at).ok_or(ScheduleError::Internal)?;
    if accepted_at < saved_target {
        return Err(ScheduleError::Internal);
    }
    let tz = time::parse_time_zone(&record.time_zone)?;
    let time = time::parse_clock_time(&record.time, "daily")?;
    let occurrence = latest_due_occurrence(tz, every_date, time, accepted_at, saved_target);
    let occurrence_at = time::format_utc(occurrence);
    let after_date = time::local_date(occurrence, tz);
    let next = next_daily_target(time, tz, accepted_at, Some(after_date));
    Ok(RecurringOccurrence { occurrence_at, next_scheduled_at: next })
}

/// 每周决策。
pub fn resolve_weekly_occurrence(
    record: &WeeklyRecord,
    accepted_at: i64,
) -> Result<RecurringOccurrence, ScheduleError> {
    let saved_target = time::parse_utc_instant(&record.scheduled_at).ok_or(ScheduleError::Internal)?;
    if accepted_at < saved_target {
        return Err(ScheduleError::Internal);
    }
    let tz = time::parse_time_zone(&record.time_zone)?;
    let time = time::parse_clock_time(&record.time, "weekly")?;
    let weekdays = normalize_weekdays(&record.weekdays)?;
    let occurrence =
        latest_due_occurrence(tz, weekday_set(&weekdays), time, accepted_at, saved_target);
    let occurrence_at = time::format_utc(occurrence);
    let after_date = time::local_date(occurrence, tz);
    let next = next_weekly_target(time, tz, &weekdays, accepted_at, Some(after_date));
    Ok(RecurringOccurrence { occurrence_at, next_scheduled_at: next })
}

/// cron 决策。
pub fn resolve_cron_occurrence(
    record: &CronRecord,
    accepted_at: i64,
) -> Result<RecurringOccurrence, ScheduleError> {
    let saved_target = time::parse_utc_instant(&record.scheduled_at).ok_or(ScheduleError::Internal)?;
    if accepted_at < saved_target {
        return Err(ScheduleError::Internal);
    }
    let tz = time::parse_time_zone(&record.time_zone)?;
    let parsed = cron::parse_cron_expression(&record.expression)?;
    let occurrence = latest_cron_occurrence(&parsed, tz, accepted_at, saved_target);
    let occurrence_at = time::format_utc(occurrence);
    let next = next_cron_target(&parsed, tz, accepted_at);
    Ok(RecurringOccurrence { occurrence_at, next_scheduled_at: next })
}

/// 是否为循环 record（排除 after/at 一次性变体）。
pub fn is_recurring(record: &ScheduleRecord) -> bool {
    matches!(
        record,
        ScheduleRecord::Every(_)
            | ScheduleRecord::Daily(_)
            | ScheduleRecord::Weekly(_)
            | ScheduleRecord::Cron(_)
    )
}

/// 解析一个到期循环 record 的规则特定时序。
pub fn resolve_recurring_occurrence(
    record: &ScheduleRecord,
    accepted_at: i64,
) -> Result<RecurringOccurrence, ScheduleError> {
    match record {
        ScheduleRecord::Every(r) => resolve_every_occurrence(r, accepted_at),
        ScheduleRecord::Daily(r) => resolve_daily_occurrence(r, accepted_at),
        ScheduleRecord::Weekly(r) => resolve_weekly_occurrence(r, accepted_at),
        ScheduleRecord::Cron(r) => resolve_cron_occurrence(r, accepted_at),
        _ => Err(ScheduleError::Internal),
    }
}

/// 生成全局唯一提醒身份（`schedule-{uuid}`）。
pub fn new_schedule_id() -> ScheduleId {
    format!("schedule-{}", uuid::Uuid::new_v4())
}

/// 构造执行本地管理视图（state + delivery_mode）。
pub fn schedule_view(record: &ScheduleRecord, now: i64) -> ScheduleView {
    let state = if now >= time::parse_utc_instant(record.scheduled_at()).unwrap_or(i64::MAX) {
        ScheduleState::Overdue
    } else {
        ScheduleState::Scheduled
    };
    ScheduleView {
        record: record.clone(),
        state,
        delivery_mode: "host".to_string(),
    }
}

// 以下为 update 语义辅助，供 service 组装结果使用。

/// 校验一个时序变更选择器（kind 与其匹配的属性键）。
pub fn timing_selector(kind: &str) -> Option<&'static str> {
    match kind {
        "at" => Some("at"),
        "every" => Some("every_seconds"),
        "daily" => Some("daily"),
        "weekly" => Some("weekly"),
        "cron" => Some("cron"),
        _ => None,
    }
}

/// 规范化 weekly 工作日集合的比较封装（供 update 判断等价）。
pub fn weekdays_equal(left: &[u32], right: &[u32]) -> bool {
    left == right
}

/// 删除结果：统一处缺失为 not-found（非变更）。
pub fn delete_result(id: ScheduleId, deleted: bool) -> ScheduleDeleteResult {
    if deleted {
        ScheduleDeleteResult::Deleted { id }
    } else {
        ScheduleDeleteResult::NotFound { id }
    }
}

/// 更新结果：把 `changed_record` 的产物映射为稳定结果。
pub fn updated_result(id: ScheduleId, record: Option<ScheduleRecord>) -> ScheduleUpdateResult {
    match record {
        Some(record) => ScheduleUpdateResult::Updated { record },
        None => ScheduleUpdateResult::Ended { id },
    }
}

#[cfg(test)]
#[path = "domain_tests.rs"]
mod tests;
