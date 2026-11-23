//! plan 模式投影：log-only 全值替换事件折叠。
//!
//! 四轨投影：
//! - `active`：当前是否处于 plan 模式（`plan/mode` 事件，last-write-wins）；
//! - `wanted`：宿主期望的 plan 状态（`plan/wanted` 事件，last-write-wins）；
//! - `running`：是否有 plan 回合正在运行（`plan/run-done` 生命周期，见 commands.rs）；
//! - `pending`：可重放推导——`wanted != active` 即有待生效的切换（无需进程内状态）。
//!
//! resume/fork 折叠日志即可恢复全部四轨。

use std::sync::Arc;

use crate::session::SessionLog;

/// plan 模式投影快照。
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct PlanProjection {
    /// 当前 plan 模式（durable，`plan/mode` 最后一条生效）。
    pub active: bool,
    /// 宿主期望的 plan 状态（durable，`plan/wanted` 最后一条生效）。
    pub wanted: bool,
    /// 是否有 plan 回合正在运行（`plan/run-done` 折叠）。
    pub running: bool,
}

impl PlanProjection {
    /// pending = 期望与当前不一致（有待生效的切换）。
    pub fn pending(&self) -> bool {
        self.wanted != self.active
    }
}

/// 从 durable 事件流折叠出四轨投影（last-write-wins）。
pub fn fold_plan(events: &[crate::session::SessionEvent]) -> PlanProjection {
    let mut projection = PlanProjection::default();
    for event in events {
        match event {
            crate::session::SessionEvent::PlanMode { active } => projection.active = *active,
            crate::session::SessionEvent::PlanWanted { active } => projection.wanted = *active,
            crate::session::SessionEvent::PlanRunDone { .. } => projection.running = false,
            _ => {}
        }
    }
    projection
}

/// 追加一条 plan/mode 事件（生效状态）。
pub fn append_plan(session: &SessionLog, active: bool) {
    session.append(crate::session::SessionEvent::PlanMode { active });
}

/// 追加一条 plan/wanted 事件（宿主期望状态）。
pub fn append_plan_wanted(session: &SessionLog, active: bool) {
    session.append(crate::session::SessionEvent::PlanWanted { active });
}

/// 追加一条 plan/run-done 事件（plan 回合结束，running 置 false）。
pub fn append_plan_run_done(session: &SessionLog) {
    session.append(crate::session::SessionEvent::PlanRunDone {});
}

/// 接线辅助：构造时从 session 投影初始状态。
pub fn initial_projection(session: &Arc<SessionLog>) -> PlanProjection {
    fold_plan(&session.snapshot())
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
