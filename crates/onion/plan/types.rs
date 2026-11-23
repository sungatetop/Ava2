//! plan 模式词汇表：状态（active + pending 待定选择）。
//!
//! 持久化用 log-only 全值替换
//! 事件（`plan/mode`，最后一条生效）；`pending` 为尚未生效的待定选择（进程内）。

/// plan 模式状态：active + 待定选择（pending）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlanModeState {
    /// 是否处于 plan 模式（durable 投影）。
    pub active: bool,
    /// 尚未生效的待定选择（进程内，不落盘）。
    /// Some(true) = 待批准退出；Some(false) = 待继续规划；None = 无待定。
    pub pending: Option<bool>,
}
