//! 时间/时区基元：RFC3339 解析与格式化、IANA 时区换算、墙钟日历运算。
//!
//! Temporal 语义（教学子集）：
//! - epoch 毫秒 <-> 规范四位数年份 RFC3339 UTC 字符串；
//! - IANA 时区（chrono-tz）换算：`localInstant`（DST 重叠取 earlier，间隙拒绝）；
//! - 墙钟时间 `HH:mm:ss.SSS` 解析/规范化；
//! - 日期推进（daily/weekly/cron 搜索用）。

use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Timelike, Utc};
use chrono_tz::Tz;

use super::types::ScheduleError;

/// 四位数年份 UTC 范围边界（epoch 毫秒）。
pub const MIN_FOUR_DIGIT_YEAR_MS: i64 = -62135596800000; // 0001-01-01T00:00:00.000Z
pub const MAX_FOUR_DIGIT_YEAR_MS: i64 = 253402300799999; // 9999-12-31T23:59:59.999Z

/// 当前 epoch 毫秒。
pub fn now_ms() -> i64 {
    crate::session::epoch_millis() as i64
}

/// 把 epoch 毫秒格式化为规范四位数年份 RFC3339 UTC 字符串（`...Z`）。
/// 输入被调用方保证在四位数年份范围内；防御性 clamp 到该范围（不 panic）。
pub fn format_utc(epoch_ms: i64) -> String {
    let clamped = epoch_ms.clamp(MIN_FOUR_DIGIT_YEAR_MS, MAX_FOUR_DIGIT_YEAR_MS);
    match chrono::DateTime::<Utc>::from_timestamp_millis(clamped) {
        Some(dt) => dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
        None => "0001-01-01T00:00:00.000Z".to_string(),
    }
}

/// 解析规范 RFC3339 UTC 字符串（`...Z`）为 epoch 毫秒。
pub fn parse_utc_instant(value: &str) -> Option<i64> {
    // 支持 `YYYY-MM-DDTHH:mm:ss[.fff]Z`（.fff 为 1-3 位小数）。
    let rest = value.strip_suffix('Z')?;
    let (date_part, time_part) = rest.split_once('T')?;
    let date = NaiveDate::parse_from_str(date_part, "%Y-%m-%d").ok()?;
    let time = parse_local_time(time_part).ok()?;
    let dt: NaiveDateTime = date.and_time(time);
    Utc.from_utc_datetime(&dt).timestamp_millis().into()
}

/// 解析带偏移的 RFC3339 字符串为 epoch 毫秒。
/// 支持 `Z` 或 `±HH:MM` 显式偏移。
pub fn parse_offset_instant(value: &str) -> Result<i64, ScheduleError> {
    let (body, zone) = split_zone(value).ok_or_else(|| {
        ScheduleError::InvalidRule(
            "at must use YYYY-MM-DDTHH:mm:ss with optional 1-3 digit fractional seconds and an explicit Z or numeric offset."
                .to_string(),
        )
    })?;
    let (date_part, time_part) = body.split_once('T').ok_or_else(|| {
        ScheduleError::InvalidRule("The at value must be a real ISO calendar date and time.".to_string())
    })?;
    let date = NaiveDate::parse_from_str(date_part, "%Y-%m-%d")
        .map_err(|_| ScheduleError::InvalidRule("The at value must be a real ISO calendar date and time.".to_string()))?;
    let time = parse_local_time(time_part)?;
    let local_epoch = date.and_time(time).and_utc().timestamp_millis();
    match zone {
        Zone::Utc => Ok(local_epoch),
        Zone::Offset { sign, hour, minute } => {
            if hour > 23 || minute > 59 || (sign == -1 && hour == 0 && minute == 0) {
                return Err(ScheduleError::InvalidRule("The at numeric offset is invalid.".to_string()));
            }
            let direction = if sign == 1 { 1i64 } else { -1i64 };
            Ok(local_epoch - direction * ((hour * 60 + minute) as i64) * 60_000)
        }
    }
}

/// 解析严格本地日期字符串（`YYYY-MM-DD`）。
pub fn parse_local_date(value: &str) -> Result<NaiveDate, ScheduleError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
        ScheduleError::InvalidRule("The local at value must be a real ISO calendar date and time.".to_string())
    })
}

/// 解析本地墙钟时间字符串（`HH:mm:ss[.fff]`）。
pub fn parse_local_time(value: &str) -> Result<NaiveTime, ScheduleError> {
    if value.len() < 8 {
        return Err(ScheduleError::InvalidRule("The at value must be a real ISO calendar date and time.".to_string()));
    }
    let (hms, fraction) = match value.split_once('.') {
        Some((h, f)) => (h, f),
        None => (value, ""),
    };
    let mut parts = hms.split(':');
    let hour: u32 = parts.next().and_then(|s| s.parse().ok()).ok_or_else(|| {
        ScheduleError::InvalidRule("The at value must be a real ISO calendar date and time.".to_string())
    })?;
    let minute: u32 = parts.next().and_then(|s| s.parse().ok()).ok_or_else(|| {
        ScheduleError::InvalidRule("The at value must be a real ISO calendar date and time.".to_string())
    })?;
    let second: u32 = parts.next().and_then(|s| s.parse().ok()).ok_or_else(|| {
        ScheduleError::InvalidRule("The at value must be a real ISO calendar date and time.".to_string())
    })?;
    if hour > 23 || minute > 59 || second > 59 {
        return Err(ScheduleError::InvalidRule("The at value must be a real ISO calendar date and time.".to_string()));
    }
    let millis = if fraction.is_empty() {
        0
    } else {
        let padded = format!("{fraction:0<3}");
        padded[..3].parse::<u32>().unwrap_or(0)
    };
    NaiveTime::from_hms_milli_opt(hour, minute, second, millis)
        .ok_or_else(|| ScheduleError::InvalidRule("The at value must be a real ISO calendar date and time.".to_string()))
}

/// 解析严格的本地时钟时间（`HH:mm:ss[.fff]`，用于 daily/weekly 选择器）。
pub fn parse_clock_time(value: &str, selector: &str) -> Result<NaiveTime, ScheduleError> {
    parse_local_time(value).map_err(|_| {
        ScheduleError::InvalidRule(format!(
            "{selector}.time must use HH:mm:ss with optional 1-3 fractional digits, without leap seconds or 24:00."
        ))
    })
}

/// 规范化本地时钟时间为 `HH:mm:ss.SSS`（3 位小数秒）。
pub fn normalize_clock_time(value: &str, selector: &str) -> Result<String, ScheduleError> {
    let time = parse_clock_time(value, selector)?;
    Ok(format!(
        "{:02}:{:02}:{:02}.{:03}",
        time.hour(),
        time.minute(),
        time.second(),
        time.nanosecond() / 1_000_000
    ))
}

/// 解析 IANA 时区字符串为 `Tz`。
pub fn parse_time_zone(value: &str) -> Result<Tz, ScheduleError> {
    if value == "UTC" {
        return Ok(Tz::UTC);
    }
    value
        .parse::<Tz>()
        .map_err(|_| ScheduleError::InvalidTimeZone("time_zone must be UTC or a valid IANA Area/Location name.".to_string()))
}

/// 本地日历时刻 -> epoch 毫秒：DST 重叠取 earlier（`disambiguation: 'earlier'`）；间隙（不存在）返回 None。
pub fn local_instant(date: NaiveDate, time: NaiveTime, tz: Tz) -> Option<i64> {
    let local = date.and_time(time);
    tz.from_local_datetime(&local)
        .earliest()
        .map(|dt| dt.timestamp_millis())
}

/// epoch 毫秒 -> 指定时区的本地日期。
pub fn local_date(epoch_ms: i64, tz: Tz) -> NaiveDate {
    let clamped = epoch_ms.clamp(MIN_FOUR_DIGIT_YEAR_MS, MAX_FOUR_DIGIT_YEAR_MS);
    tz.timestamp_millis_opt(clamped).single().map(|dt| dt.date_naive()).unwrap_or(NaiveDate::MIN)
}

/// epoch 毫秒 -> 指定时区的本地时刻（用于 cron 的 floorTime 比较）。
pub fn local_time_of(epoch_ms: i64, tz: Tz) -> NaiveTime {
    let clamped = epoch_ms.clamp(MIN_FOUR_DIGIT_YEAR_MS, MAX_FOUR_DIGIT_YEAR_MS);
    tz.timestamp_millis_opt(clamped).single().map(|dt| dt.time()).unwrap_or(NaiveTime::MIN)
}

/// ISO 工作日（周一 1 .. 周日 7，Temporal ISO 口径）。
pub fn iso_weekday(date: NaiveDate) -> u32 {
    date.weekday().number_from_monday()
}

/// 月份（1-12）。
pub fn month_of(date: NaiveDate) -> u32 {
    date.month()
}

/// 日（1-31）。
pub fn day_of(date: NaiveDate) -> u32 {
    date.day()
}

/// 天数推进。
pub fn add_days(date: NaiveDate, days: i64) -> NaiveDate {
    date + chrono::Duration::days(days)
}

/// 月份推进到次月 1 日。
#[allow(clippy::expect_used)] // 由 Y/M 构造的 1 日恒合法。
pub fn next_month_first(date: NaiveDate) -> NaiveDate {
    let (y, m) = if date.month() == 12 { (date.year() + 1, 1) } else { (date.year(), date.month() + 1) };
    NaiveDate::from_ymd_opt(y, m, 1).expect("month first always valid")
}

/// 回退到当月 1 日。
#[allow(clippy::expect_used)] // 由 Y/M 构造的 1 日恒合法。
pub fn month_first(date: NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd_opt(date.year(), date.month(), 1).expect("month first always valid")
}

/// 年份推进。
#[allow(clippy::expect_used)] // 闰日回退到 2 月 28 日恒合法。
pub fn add_years(date: NaiveDate, years: i64) -> NaiveDate {
    NaiveDate::from_ymd_opt(date.year() + years as i32, date.month(), date.day())
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(date.year() + years as i32, 2, 28).expect("leap day rolls back to Feb 28"))
}

enum Zone {
    Utc,
    Offset { sign: i32, hour: i32, minute: i32 },
}

/// 分割 RFC3339 的日期时间体与时区后缀。
fn split_zone(value: &str) -> Option<(&str, Zone)> {
    if let Some(body) = value.strip_suffix('Z') {
        return Some((body, Zone::Utc));
    }
    // 找最后一个 `+` 或 `-`（日期时间体里的 `-` 出现在 `T` 前；偏移在 `T` 后）。
    let t_index = value.find('T')?;
    let after = &value[t_index + 1..];
    let sign_index = after.rfind(['+', '-'])?;
    let sign_char = after.as_bytes()[sign_index] as char;
    let sign = if sign_char == '+' { 1 } else { -1 };
    let offset = &after[sign_index + 1..];
    let (oh, om) = offset.split_once(':')?;
    let hour: i32 = oh.parse().ok()?;
    let minute: i32 = om.parse().ok()?;
    let body = &value[..t_index + 1 + sign_index];
    Some((body, Zone::Offset { sign, hour, minute }))
}
