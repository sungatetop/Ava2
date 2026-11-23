//! 会话历史查询：5 个只读工具 + 磁盘 JSONL 读取纯函数。
//!
//! 核心语义（教学子集）：
//! - `session_search`：跨会话全文搜索，返回每个会话的**最佳匹配**事件；工作区限定
//!   （只搜当前 cwd 的 `sessions/<normalized-cwd>/`），**始终排除调用会话**；
//! - `session_event_search`：单个已授权会话内全文搜索；针对当前会话时停在调用步骤
//!   之前（以调用时已记录事件数为 seq 上界）；
//! - `session_trace`：一个会话的元信息 + fork 血缘（经 `ForkSeed` 播种标记呈现）；
//! - `session_event_trace`：目标事件的位置与前后关系（onion 无压缩替换/派生关系，
//!   标注 `replaced_by: none` / `derived: none`）；
//! - `session_event_read`：一个完整未删节事件（JSON）+ 可选相邻事件摘要。
//!
//! 搜索停在部署上限（`maxSearchResults`，缺省 100）；超出部分经游标翻页（结果页以
//! `Next cursor:` 给出令牌，见 `filters`：请求指纹不符判 Invalid、语料代际不符判 Stale）。
//! 授权保守：仅允许访问与调用方 `cwd` 严格匹配的会话（经 `sessions_dir` 定位天然限定）。

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::session::SessionEvent;

pub mod filters;
pub mod tools;

/// 一条带 seq 与时间戳的 durable 事件（seq 从 1 起，按 JSONL 行序）。
#[derive(Clone, Debug, PartialEq)]
pub struct EventRecord {
    /// 事件序号（1-based）。
    pub seq: u64,
    /// 事件落盘时刻（epoch 毫秒；来自 JSONL 外层 `ts`）。
    pub ts: u64,
    /// durable 事件本体。
    pub event: SessionEvent,
}

/// 会话文件清单项：编码 id + 日志文件路径。
pub struct SessionFile {
    pub id: String,
    pub path: PathBuf,
}

/// 事件类型名（serde tag，snake_case；用于过滤与展示）。
pub fn event_type(event: &SessionEvent) -> String {
    serde_json::to_value(event)
        .ok()
        .and_then(|value| value.get("type").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

/// 读取单个会话 JSONL 为 `EventRecord` 序列（含 seq 编号与 ts）。
/// 行解析失败被跳过（容错——半写行/损坏行不拖垮整个查询）。
pub fn read_session_events(log_file: &Path) -> std::io::Result<Vec<EventRecord>> {
    let content = std::fs::read_to_string(log_file)?;
    let mut records = Vec::new();
    for (index, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parsed: Value = match serde_json::from_str(trimmed) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let ts = parsed.get("ts").and_then(Value::as_u64).unwrap_or(0);
        let event: SessionEvent = match serde_json::from_value(parsed.get("event").cloned().unwrap_or_default()) {
            Ok(value) => value,
            Err(_) => continue,
        };
        records.push(EventRecord {
            seq: (index + 1) as u64,
            ts,
            event,
        });
    }
    Ok(records)
}

/// 列出 `sessions_dir` 下全部会话（每子目录一个 `<encoded-id>/session.v1.jsonl`）。
pub fn list_session_files(sessions_dir: &Path) -> Vec<SessionFile> {
    let mut files = Vec::new();
    let Ok(entries) = std::fs::read_dir(sessions_dir) else {
        return files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().to_string();
        let file = crate::paths::session_log_file(&path);
        if file.is_file() {
            files.push(SessionFile { id, path: file });
        }
    }
    files
}

/// 事件 JSON 序列化文本（搜索与 snippet 的统一来源）。
pub fn event_json(record: &EventRecord) -> String {
    serde_json::to_string(&record.event).unwrap_or_else(|_| format!("<unserializable seq {}>", record.seq))
}

/// 截断到 `max_chars` 的 snippet（摘录；超长以 `…` 结尾）。
pub fn snippet(record: &EventRecord, max_chars: usize) -> String {
    snippet_text(&event_json(record), max_chars)
}

/// 纯文本截断（事件 JSON 摘录与搜索命中摘录共用；超长以 `…` 结尾）。
pub fn snippet_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_string()
    } else {
        let head: String = text.chars().take(max_chars).collect();
        format!("{head}…")
    }
}

/// epoch 毫秒 → ISO 8601（`Z` 时区；0 视为未知输出 `<unknown>`）。
pub fn format_time(ts: u64) -> String {
    if ts == 0 {
        return "<unknown>".to_string();
    }
    let secs = ts / 1000;
    let millis = ts % 1000;
    let days = secs / 86_400;
    let mut rem = secs % 86_400;
    let hour = rem / 3600;
    rem %= 3600;
    let minute = rem / 60;
    let second = rem % 60;
    // 自 1970-01-01 起的天数 → 年月日（民用日历，手写无 chrono）。
    let (year, month, day) = civil_from_days(days as i64);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z")
}

/// 天数 → 民用公历日期（Howard Hinnant 算法，手写无 chrono）。
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
