//! spill 启动清扫 GC：进程启动对 spill
//! 根做一次 mtime 截止清扫——只删普通文件且 mtime 严格早于截止时间；清空后
//! 的会话目录连带修剪。工件按时间而非会话生命周期回收（fork / 子会话引用在
//! 保留期内始终可读）。

use std::path::Path;
use std::time::{Duration, SystemTime};

use super::SESSIONS_DIR;

/// 缺省保留天数（30）。
pub const DEFAULT_RETENTION_DAYS: u64 = 30;

/// 一次清扫的统计。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SweepStats {
    pub scanned_session_dirs: usize,
    pub removed_files: usize,
    pub removed_bytes: u64,
    pub pruned_dirs: usize,
}

/// 清扫截止时间：`now - retention_days`（mtime 严格早于它即过期）。
pub fn cleanup_cutoff(now: SystemTime, retention_days: u64) -> SystemTime {
    now - Duration::from_secs(retention_days.saturating_mul(86_400))
}

/// 清扫 spill 根：`<root>/sessions/*` 下逐会话目录过期文件删除 + 空目录修剪。
/// 无 `sessions/` 目录（首次启动常态）返回零统计。
pub fn sweep_spill_root(root: &Path, cutoff: SystemTime) -> std::io::Result<SweepStats> {
    let mut stats = SweepStats::default();
    let sessions = root.join(SESSIONS_DIR);
    let entries = match std::fs::read_dir(&sessions) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(stats),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        stats.scanned_session_dirs += 1;
        if sweep_session_dir(&entry.path(), cutoff, &mut stats)? {
            std::fs::remove_dir(entry.path())?;
            stats.pruned_dirs += 1;
        }
    }
    Ok(stats)
}

/// 清扫单个会话目录；全部条目清空时返回 `true`（调用方连带修剪目录）。
fn sweep_session_dir(dir: &Path, cutoff: SystemTime, stats: &mut SweepStats) -> std::io::Result<bool> {
    let mut remaining = 0usize;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        // 只删普通文件：符号链接 / 子目录一律保守保留（regular-file 约束）。
        if !entry.file_type()?.is_file() {
            remaining += 1;
            continue;
        }
        let expired = std::fs::metadata(entry.path())
            .and_then(|meta| meta.modified())
            .map(|modified| modified < cutoff)
            // mtime 不可得 → 保守保留。
            .unwrap_or(false);
        if expired {
            let size = std::fs::metadata(entry.path()).map(|meta| meta.len()).unwrap_or(0);
            std::fs::remove_file(entry.path())?;
            stats.removed_files += 1;
            stats.removed_bytes += size;
        } else {
            remaining += 1;
        }
    }
    Ok(remaining == 0)
}

#[cfg(test)]
#[path = "cleanup_tests.rs"]
mod tests;
