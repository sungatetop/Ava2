//! 一次性变化等待器。
//!
//! `wait_for_change` 等待下一次团队变化（roster/task/消息/状态边），支持超时、
//! 取消（abort）、关闭（disposal 释放）。超时范围 [10s, 1h]，否则
//! `TEAM_INVALID_TIMEOUT`。

use std::time::Duration;

use tokio::time::timeout;

use crate::agent_team::journal::Notify;
use crate::agent_team::TeamError;

/// 超时范围下限（毫秒，10s）。
pub const MIN_TIMEOUT_MS: u64 = 10_000;
/// 超时范围上限（毫秒，1h）。
pub const MAX_TIMEOUT_MS: u64 = 3_600_000;

/// 等待结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaitResult {
    pub timed_out: bool,
    /// 是否因团队关闭而返回（`{timedOut:false}`）。
    pub closed: bool,
}

/// 等待一次团队变化。
///
/// - `notify`：版本号广播源；
/// - `timeout_ms`：超时（须在 [MIN, MAX] 内）；
/// - `closed`：团队是否已关闭（关闭则立即返回 `{timedOut:false}`）。
pub async fn wait_for_change(
    notify: &Notify,
    timeout_ms: u64,
    closed: bool,
) -> Result<WaitResult, TeamError> {
    if !(MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(&timeout_ms) {
        return Err(TeamError::InvalidTimeout);
    }
    Ok(wait_impl(notify, timeout_ms, closed).await)
}

/// 核心等待逻辑（公开 API 做超时区间校验后委托；测试可直接调用任意超时）。
pub async fn wait_impl(notify: &Notify, timeout_ms: u64, closed: bool) -> WaitResult {
    if closed {
        return WaitResult { timed_out: false, closed: true };
    }
    let (current, mut rx) = notify.subscribe();
    // 若已有未消费的版本变化，立即返回（不等待）。
    if *rx.borrow() != current {
        return WaitResult { timed_out: false, closed: false };
    }
    let fut = async move {
        match rx.changed().await {
            Ok(()) => WaitResult { timed_out: false, closed: false },
            Err(_) => WaitResult { timed_out: false, closed: true },
        }
    };
    let dur = Duration::from_millis(timeout_ms);
    match timeout(dur, fut).await {
        Ok(result) => result,
        Err(_elapsed) => WaitResult { timed_out: true, closed: false },
    }
}

#[cfg(test)]
#[path = "activity_tests.rs"]
mod tests;
