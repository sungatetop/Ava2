//! 数据模型：记忆类型与记忆头。
//!
//! 对齐 ava-harness::memory 设计蓝本：记忆被约束为封闭四型，
//! 且只保存"当前项目状态不可推导"的信息。

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// 四种记忆类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryType {
    /// 用户角色、目标、偏好、责任、知识。
    User,
    /// 用户对工作方式的指导（含 **Why** 与 **How to apply**）。
    Feedback,
    /// 进行中的工作、目标、bug、事件。
    Project,
    /// 指向外部系统的指针（Linear、Slack、Grafana）。
    Reference,
}

impl MemoryType {
    /// 类型名（小写，与 frontmatter 一致）。
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoryType::User => "user",
            MemoryType::Feedback => "feedback",
            MemoryType::Project => "project",
            MemoryType::Reference => "reference",
        }
    }
}

/// 解析记忆类型；对非法/缺失值返回 `None`（优雅降级，兼容旧文件）。
pub fn parse_memory_type(s: &str) -> Option<MemoryType> {
    match s.trim().to_ascii_lowercase().as_str() {
        "user" => Some(MemoryType::User),
        "feedback" => Some(MemoryType::Feedback),
        "project" => Some(MemoryType::Project),
        "reference" => Some(MemoryType::Reference),
        _ => None,
    }
}

/// 记忆头（扫描结果）—— 用于相关性召回。
#[derive(Debug, Clone)]
pub struct MemoryHeader {
    /// 主题文件名（如 `user_role.md`）。
    pub filename: String,
    /// 绝对路径。
    pub file_path: PathBuf,
    /// 修改时间（毫秒）。
    pub mtime_ms: u64,
    /// 单行描述（召回相关性判断关键词）。
    pub description: String,
    /// 记忆类型（缺失/非法时为 `None`）。
    pub memory_type: Option<MemoryType>,
}

impl MemoryHeader {
    /// manifest 行：`[type] filename (ISO): description`。
    pub fn manifest_line(&self) -> String {
        let ty = self.memory_type.map(|t| t.as_str()).unwrap_or("unknown");
        let when = iso_time(self.mtime_ms);
        format!("[{}] {} ({}): {}", ty, self.filename, when, self.description)
    }
}

/// 毫秒时间戳 → ISO 8601 日期时间（UTC）。
pub fn iso_time(ms: u64) -> String {
    let secs = ms.saturating_div(1000) as i64;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    format!("{y:04}-{m:02}-{d:02} {hh:02}:{mm:02}:{ss:02}Z")
}

/// 天数（自 1970-01-01）→ 公历 (y, m, d)。Howard Hinnant 算法。
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as i64;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as i64;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;
