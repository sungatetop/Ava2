//! 运行时准入截止 + 有界结算。
//!
//! `close()` 置 disposed（准入操作报 `TEAM_DISPOSED`）并唤醒 waiter；dispose 时
//! 有界等待子成员 drain，超时 `TEAM_DISPOSAL_TIMEOUT`。

use std::time::Duration;

use tokio::time::timeout;

use crate::agent_team::TeamError;

/// 生命周期状态。
#[derive(Debug)]
pub struct Lifecycle {
    disposed: std::sync::atomic::AtomicBool,
    config: crate::agent_team::config::TeamConfig,
}

impl Lifecycle {
    pub fn new(config: crate::agent_team::config::TeamConfig) -> Self {
        Self {
            disposed: std::sync::atomic::AtomicBool::new(false),
            config,
        }
    }

    /// 是否已 disposed（准入操作应报 `TEAM_DISPOSED`）。
    pub fn is_disposed(&self) -> bool {
        self.disposed.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// 标记已 disposed 并返回是否首次。
    pub fn close(&self) -> bool {
        !self.disposed.swap(true, std::sync::atomic::Ordering::SeqCst)
    }

    /// 有界等待一个结算 future，超时抛 `TEAM_DISPOSAL_TIMEOUT`。
    pub async fn with_timeout<F, T>(&self, fut: F) -> Result<T, TeamError>
    where
        F: std::future::Future<Output = Result<T, TeamError>>,
    {
        let dur = Duration::from_millis(self.config.disposal_timeout_ms);
        match timeout(dur, fut).await {
            Ok(Ok(t)) => Ok(t),
            Ok(Err(e)) => Err(e),
            Err(_elapsed) => Err(TeamError::DisposalTimeout),
        }
    }
}
