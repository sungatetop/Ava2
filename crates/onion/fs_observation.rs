//! read-before-write 观测策略。
//!
//! 设计原则：
//!
//! 1. **事件门禁而非服务**——本模块只提供状态表与判定，纯函数化，不触碰文件系统；
//!    工具侧在读成功后 [`FsObservation::record`]，在写/改前 [`FsObservation::guard`]
//!    取得可选版本凭证。这样移除策略不影响工具可用性（退回无条件改写）。
//! 2. **已观察状态是先前观察记录**——三态逻辑：未见 / 确认缺失 / 存在于某版本。
//!    缺失也被记录（读到不存在即记 `Absent`），因此随后的 `write` 可走创建路径
//!    重新创建，而不会覆盖并发创建者。
//!
//! 版本凭证用「内容摘要」而非 mtime（内容寻址：
//! 无平台依赖，且天然检测「自读取以来被改动」）。非 UTF-8 文件按字节摘要处理。
//!
//! 会话恢复不携带观测状态（dispose 即丢弃，绝不跨会话持久化）——
//! 因此 `/resume` 后必须重新读取才能改写。

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

/// 内容摘要（FNV-1a 64bit，十六进制；仅用于新鲜度比对，非安全哈希）。
fn digest(data: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in data {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// 单个目标的观测记录。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Observed {
    /// 未见（本会话无任何读记录）。
    Unseen,
    /// 已确认缺失（读到不存在也是有效观察，故允许后续创建）。
    Absent,
    /// 存在于该内容摘要版本。
    Present { version: String },
}

/// 会话级观测表（键为沙箱 root 内的相对路径，统一分隔符）。
pub struct FsObservation {
    entries: Mutex<HashMap<String, Observed>>,
}

impl FsObservation {
    pub fn new() -> Self {
        Self { entries: Mutex::new(HashMap::new()) }
    }

    /// 记录一次观察（读成功记 `Present(version)`，读失败记 `Absent`）。
    pub fn record(&self, relative: &str, observed: Observed) {
        lock(&self.entries).insert(normalize_key(relative), observed);
    }

    /// 读取当前观察状态（默认 `Unseen`）。
    pub fn get(&self, relative: &str) -> Observed {
        lock(&self.entries).get(&normalize_key(relative)).cloned().unwrap_or(Observed::Unseen)
    }

    /// 写/改前的观测门禁：返回 `Ok(())` 放行（可含过期凭证），或 `Err` 拒绝。
    ///
    /// - `Unseen` → 拒绝（`FS_NOT_OBSERVED`：必须先读）；
    /// - `Absent` + 当前已存在 → 拒绝（`FS_NOT_FOUND`：观测为缺失却已出现，视为并发创建，
    ///   须重新读取以取得新事实）；
    /// - `Present(version)` + 当前摘要不同 → 放行但返回过期凭证，由调用方决定；
    ///   本实现中 `Sandbox::edit_text` 以过期凭证失败（`FS_STALE_VERSION`）：
    ///   自读取以来变化（含缺失）的文件一律以 FS_STALE_VERSION 失败。
    pub fn guard(&self, relative: &str, current: Option<&str>) -> Result<Option<String>, String> {
        match self.get(relative) {
            Observed::Unseen => Err(format!(
                "cannot modify \"{relative}\": file has not been read — read the file, then retry \
                 (FS_NOT_OBSERVED)"
            )),
            Observed::Absent => match current {
                // 确认缺失且当前仍缺失：放行（创建路径）。
                None => Ok(None),
                Some(_) => Err(format!(
                    "cannot modify \"{relative}\": not found (FS_NOT_FOUND) — it was observed missing; \
                     re-read before modifying"
                )),
            },
            Observed::Present { version } => match current {
                Some(actual) if actual != version => Ok(Some(version)),
                Some(_) => Ok(None),
                // 观测存在但现已消失：过期凭证（FS_STALE_VERSION）。
                None => Ok(Some(version)),
            },
        }
    }

    /// 落地一次成功的改动：刷新为新版本观察（写/改后本会话已持有新事实）。
    pub fn refresh(&self, relative: &str, version: String) {
        self.record(relative, Observed::Present { version });
    }

    /// 清除全部观测（会话结束 / 测试隔离用）。
    pub fn clear(&self) {
        lock(&self.entries).clear();
    }
}

impl Default for FsObservation {
    fn default() -> Self {
        Self::new()
    }
}

/// 计算内容版本凭证（供 `record` / `refresh` 与 `guard` 比对）。
pub fn version_of(data: &[u8]) -> String {
    digest(data)
}

/// 路径键归一（统一分隔符、去前导 `./`，大小写保持原样）。
fn normalize_key(relative: &str) -> String {
    let mut key = relative.replace('\\', "/");
    while let Some(stripped) = key.strip_prefix("./") {
        key = stripped.to_string();
    }
    key
}

/// 便捷构造：`Present` 观测。
pub fn observed_present(version: String) -> Observed {
    Observed::Present { version }
}

/// 便捷构造：`Absent` 观测。
pub fn observed_absent() -> Observed {
    Observed::Absent
}

/// 以 `&Path` 归一为观测表键（工具层统一走此处，避免各处拼键规则不一致）。
pub fn key_of(path: &Path) -> String {
    normalize_key(&path.to_string_lossy())
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
#[path = "fs_observation_tests.rs"]
mod tests;
