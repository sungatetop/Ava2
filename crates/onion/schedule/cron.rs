//! cron 解析、规范化与日期匹配（Vixie 五段方言）。
//!
//! - 五段：`minute hour day-of-month month day-of-week`；
//! - 字段元素：`*`、`*/n`、`a`、`a-b`、`a-b/n`、逗号列表；
//! - 周日 0 与 7 折叠为 0（canonicalMax 6）；
//! - 规范化（`encodeCronField` / `encodeStarCronField`）保留 star 旗标；
//! - Vixie DOM/DOW AND-OR 语义：两字段都受限时 OR，否则 AND。

use std::collections::BTreeSet;

use chrono::NaiveDate;

use super::time;
use super::types::ScheduleError;

/// 字段界（求值顺序）。
struct FieldSpec {
    name: &'static str,
    min: i64,
    max: i64,
    canonical_max: i64,
}

const CRON_FIELDS: [FieldSpec; 5] = [
    FieldSpec { name: "minute", min: 0, max: 59, canonical_max: 59 },
    FieldSpec { name: "hour", min: 0, max: 23, canonical_max: 23 },
    FieldSpec { name: "day-of-month", min: 1, max: 31, canonical_max: 31 },
    FieldSpec { name: "month", min: 1, max: 12, canonical_max: 12 },
    FieldSpec { name: "day-of-week", min: 0, max: 7, canonical_max: 6 },
];

/// 解析后的 cron 表达式。
pub struct ParsedCron {
    pub expression: String,
    pub minutes: BTreeSet<i64>,
    pub hours: BTreeSet<i64>,
    pub days_of_month: BTreeSet<i64>,
    pub months: BTreeSet<i64>,
    pub days_of_week: BTreeSet<i64>,
    pub day_of_month_star: bool,
    pub day_of_week_star: bool,
}

fn invalid_field(spec: &FieldSpec, element: &str) -> ScheduleError {
    ScheduleError::InvalidRule(format!(
        "cron.expression {} field element {element:?} must be *, a value, a-b, */n, a-b/n, or a comma-separated list of those.",
        spec.name
    ))
}

fn field_value(text: &str, spec: &FieldSpec) -> Result<i64, ScheduleError> {
    let value: i64 = text
        .parse()
        .map_err(|_| ScheduleError::InvalidRule(format!("cron.expression {} field value {text} is outside {}-{}.", spec.name, spec.min, spec.max)))?;
    if value < spec.min || value > spec.max {
        return Err(ScheduleError::InvalidRule(format!(
            "cron.expression {} field value {text} is outside {}-{}.",
            spec.name, spec.min, spec.max
        )));
    }
    Ok(value)
}

fn field_step(text: &str, spec: &FieldSpec) -> Result<i64, ScheduleError> {
    let step: i64 = text
        .parse()
        .map_err(|_| ScheduleError::InvalidRule(format!("cron.expression {} field step must be a positive integer.", spec.name)))?;
    if step < 1 {
        return Err(ScheduleError::InvalidRule(format!("cron.expression {} field step must be a positive integer.", spec.name)));
    }
    Ok(step)
}

/// 展开一个逗号分隔字段为匹配值集（周日折叠）。
fn field_values(raw: &str, spec: &FieldSpec) -> Result<BTreeSet<i64>, ScheduleError> {
    let mut matched = BTreeSet::new();
    for element in raw.split(',') {
        if element.is_empty() {
            return Err(ScheduleError::InvalidRule(format!(
                "cron.expression {} field must not contain an empty list element.",
                spec.name
            )));
        }
        if element == "*" {
            for value in spec.min..=spec.max {
                matched.insert(value);
            }
            continue;
        }
        if element.starts_with('*') {
            let step_text = element.strip_prefix("*/").ok_or_else(|| invalid_field(spec, element))?;
            let step = field_step(step_text, spec)?;
            let mut value = spec.min;
            while value <= spec.max {
                matched.insert(value);
                value += step;
            }
            continue;
        }
        // `a`、`a-b`、`a-b/n`
        let (range_part, step_part) = match element.split_once('/') {
            Some((r, s)) => (r, Some(s)),
            None => (element, None),
        };
        let (start_text, end_text) = match range_part.split_once('-') {
            Some((s, e)) => (s, Some(e)),
            None => (range_part, None),
        };
        let start = field_value(start_text, spec)?;
        match end_text {
            None => {
                if step_part.is_some() {
                    return Err(invalid_field(spec, element));
                }
                matched.insert(start);
            }
            Some(end) => {
                let last = field_value(end, spec)?;
                if start > last {
                    return Err(ScheduleError::InvalidRule(format!(
                        "cron.expression {} field range {start}-{last} is inverted.",
                        spec.name
                    )));
                }
                let step = match step_part {
                    Some(s) => field_step(s, spec)?,
                    None => 1,
                };
                let mut value = start;
                while value <= last {
                    matched.insert(value);
                    value += step;
                }
            }
        }
    }
    // 周日折叠：max(7) -> min(0)。
    let mut values = BTreeSet::new();
    for value in matched {
        values.insert(if spec.canonical_max != spec.max && value == spec.max { spec.min } else { value });
    }
    Ok(values)
}

/// 编码一个匹配值集为最短等价逗号分隔字段（不以 `*` 开头）。
fn encode_field(values: &[i64]) -> String {
    if values.len() == 1 {
        return values[0].to_string();
    }
    let first = values[0];
    let last = values[values.len() - 1];
    let step = values[1] - first;
    let uniform = (2..values.len()).all(|i| values[i] - values[i - 1] == step);
    if uniform {
        return if step == 1 { format!("{first}-{last}") } else { format!("{first}-{last}/{step}") };
    }
    let mut parts: Vec<String> = Vec::new();
    let mut run_start = first;
    let mut prev = first;
    for current in values.iter().skip(1).copied() {
        if current == prev + 1 {
            prev = current;
            continue;
        }
        parts.push(if run_start == prev { run_start.to_string() } else { format!("{run_start}-{prev}") });
        run_start = current;
        prev = current;
    }
    parts.push(if run_start == prev { run_start.to_string() } else { format!("{run_start}-{prev}") });
    parts.join(",")
}

/// 从最小值以固定步长均匀行走。
fn star_walk(spec: &FieldSpec, step: i64) -> BTreeSet<i64> {
    let mut walked = BTreeSet::new();
    let mut value = spec.min;
    while value <= spec.max {
        walked.insert(if spec.canonical_max != spec.max && value == spec.max { spec.min } else { value });
        value += step;
    }
    walked
}

/// 编码一个以 `*` 开头的字段（保留 star 旗标）。
fn encode_star_field(values: &[i64], spec: &FieldSpec) -> String {
    if values.len() as i64 == spec.canonical_max - spec.min + 1 {
        return "*".to_string();
    }
    let present: BTreeSet<i64> = values.iter().copied().collect();
    let mut best_step: Option<i64> = None;
    let mut best_walk: Vec<i64> = Vec::new();
    for step in 1..=(spec.max - spec.min + 1) {
        let walk: Vec<i64> = star_walk(spec, step).into_iter().collect();
        if walk.len() > best_walk.len() && walk.iter().all(|v| present.contains(v)) {
            best_step = Some(step);
            best_walk = walk;
        }
    }
    let step = best_step.unwrap_or_else(|| panic!("{} cannot keep a leading `*`.", spec.name));
    let walked: BTreeSet<i64> = best_walk.into_iter().collect();
    let remaining: Vec<i64> = values.iter().copied().filter(|v| !walked.contains(v)).collect();
    let walk_text = format!("*/{step}");
    if remaining.is_empty() {
        walk_text
    } else {
        format!("{walk_text},{}", encode_field(&remaining))
    }
}

/// 解析一个字段：匹配值 + 规范拼写 + star 旗标。
fn parse_field(raw: &str, spec: &FieldSpec) -> Result<(BTreeSet<i64>, String, bool), ScheduleError> {
    let values = field_values(raw, spec)?;
    let star = raw.starts_with('*');
    let canonical = if star { encode_star_field(&values.iter().copied().collect::<Vec<_>>(), spec) } else { encode_field(&values.iter().copied().collect::<Vec<_>>()) };
    Ok((values, canonical, star))
}

/// 解析并规范化五段 cron 表达式。
pub fn parse_cron_expression(expression: &str) -> Result<ParsedCron, ScheduleError> {
    if expression.trim() != expression || expression.is_empty() {
        return Err(ScheduleError::InvalidRule("cron.expression must be a non-empty trimmed string.".to_string()));
    }
    let fields: Vec<&str> = expression.split_whitespace().collect();
    if fields.len() != 5 {
        return Err(ScheduleError::InvalidRule(
            "cron.expression must contain exactly five whitespace-separated fields: minute hour day-of-month month day-of-week."
                .to_string(),
        ));
    }
    let mut parsed_fields = Vec::with_capacity(5);
    let mut canonicals = Vec::with_capacity(5);
    for (index, field) in fields.iter().enumerate() {
        let spec = &CRON_FIELDS[index];
        let (values, canonical, star) = parse_field(field, spec)?;
        parsed_fields.push((values, star));
        canonicals.push(canonical);
    }
    let minutes = parsed_fields[0].0.clone();
    let hours = parsed_fields[1].0.clone();
    let days_of_month = parsed_fields[2].0.clone();
    let months = parsed_fields[3].0.clone();
    let days_of_week = parsed_fields[4].0.clone();
    Ok(ParsedCron {
        expression: canonicals.join(" "),
        minutes,
        hours,
        days_of_month,
        months,
        days_of_week,
        day_of_month_star: parsed_fields[2].1,
        day_of_week_star: parsed_fields[4].1,
    })
}

/// 日期匹配谓词（Vixie DOM/DOW AND-OR 语义）。
pub fn date_matches(parsed: &ParsedCron, date: NaiveDate) -> bool {
    if !parsed.months.contains(&(time::month_of(date) as i64)) {
        return false;
    }
    let dom = parsed.days_of_month.contains(&(time::day_of(date) as i64));
    let cron_dow = (time::iso_weekday(date) % 7) as i64; // 周日(7) -> 0
    let dow = parsed.days_of_week.contains(&cron_dow);
    let dom_restricted = !parsed.day_of_month_star;
    let dow_restricted = !parsed.day_of_week_star;
    if dom_restricted && dow_restricted {
        dom || dow
    } else {
        dom && dow
    }
}
