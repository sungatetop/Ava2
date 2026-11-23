//! PlanMode 服务：plan 模式状态 + plan-review 审批。
//!
//! 进入/退出走 `plan/mode` 全值替换事件；`exit_plan_mode`
//! 发起 plan-review 审批（Approve / Keep planning）。审批经 mpsc 通道与宿主
//! （main）交互，宿主经 `/approve` / `/deny` 复决。

use std::sync::{Arc, Mutex, OnceLock};

use tokio::sync::{mpsc, oneshot};

use crate::session::SessionLog;

use super::store::{append_plan, fold_plan};

/// plan-review 审批回执（三分支）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanVerdict {
    /// 用户批准：退出 plan 模式。
    Approve,
    /// 用户拒绝：保持 plan 模式，反馈修订。
    KeepPlanning,
    /// 用户关闭请求 / 接管：保持 plan 模式并等待下一条消息
    /// （concludeTurn 结束 turn，告诉模型留下等待）。
    Dismissed,
}

/// plan-review 审批请求（listener/tool → main）。
pub struct PlanReviewRequest {
    pub plan: String,
    pub reply: oneshot::Sender<PlanVerdict>,
}

/// PlanMode 服务：持有 session 接线槽 + active 状态 + 审批通道 + pending 待定选择。
pub struct PlanMode {
    state: Mutex<bool>,
    /// 尚未生效的待定选择（open-turn 内挂起，step 边界追加）。
    pending: Mutex<Option<bool>>,
    session: Arc<OnceLock<Arc<SessionLog>>>,
    reviews: Mutex<Option<mpsc::UnboundedSender<PlanReviewRequest>>>,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl PlanMode {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(false),
            pending: Mutex::new(None),
            session: Arc::new(OnceLock::new()),
            reviews: Mutex::new(None),
        }
    }

    /// 延迟接线：从 session 重放投影 active + 注入审批通道。
    pub fn wire_session(&self, session: Arc<SessionLog>) {
        let active = fold_plan(&session.snapshot()).active;
        let _ = self.session.set(session);
        *lock(&self.state) = active;
    }

    /// 注入审批通道（main 持有 receiver）。
    pub fn set_review_channel(&self, tx: mpsc::UnboundedSender<PlanReviewRequest>) {
        *lock(&self.reviews) = Some(tx);
    }

    /// 已接线的 session（若已接线）。
    pub fn session_arc(&self) -> Option<&Arc<SessionLog>> {
        self.session.get()
    }

    pub fn active(&self) -> bool {
        *lock(&self.state)
    }

    /// 待定选择（观测用）。
    pub fn pending(&self) -> Option<bool> {
        *lock(&self.pending)
    }

    /// 设置 plan 模式（四态结果）。
    /// - 无 pending（空闲，可直接落事件）→ `committed`；
    /// - 已有 pending → `queued`（已挂起，等待边界追加）；
    /// - 已是目标状态 → `noop`。
    pub fn set(&self, active: bool) -> SetResult {
        if *lock(&self.state) == active && lock(&self.pending).is_none() {
            return SetResult::Noop;
        }
        *lock(&self.pending) = Some(active);
        SetResult::Queued
    }

    /// 在 step 边界消费 pending：若有待定选择，落一条 plan/mode 事件并更新状态。
    /// 返回是否消费了待定选择。
    pub fn flush_pending(&self) -> bool {
        let target = match lock(&self.pending).take() {
            Some(target) => target,
            None => return false,
        };
        *lock(&self.state) = target;
        if let Some(session) = self.session.get() {
            append_plan(session, target);
        }
        true
    }

    /// 立即进入 plan 模式（空闲时；落 durable 事件）。
    pub fn enter(&self) {
        *lock(&self.state) = true;
        lock(&self.pending).take();
        if let Some(session) = self.session.get() {
            append_plan(session, true);
        }
    }

    /// 立即退出（`/plan off`，不经模型；空闲时）。
    pub fn exit(&self) {
        *lock(&self.state) = false;
        lock(&self.pending).take();
        if let Some(session) = self.session.get() {
            append_plan(session, false);
        }
    }

    /// exit_plan_mode 工具：发起 plan-review 审批。
    /// - `Approve` → 退出 plan 模式，返回 Ok(true)；
    /// - `KeepPlanning` → plan 保持，返回 Err（反馈修订）；
    /// - `Dismissed` → 用户接管，plan 保持，返回 Err（告诉模型留下等待下一条消息）。
    /// - 通道关闭/取消 → 不退出，返回 Ok(false)。
    pub async fn review(&self, plan: String) -> Result<bool, String> {
        let tx = lock(&self.reviews)
            .clone()
            .ok_or_else(|| "plan review channel not wired".to_string())?;
        let (reply, receiver) = oneshot::channel();
        let request = PlanReviewRequest { plan, reply };
        tx.send(request).map_err(|_| "plan review channel closed".to_string())?;
        match receiver.await {
            Ok(PlanVerdict::Approve) => {
                self.exit();
                Ok(true)
            }
            Ok(PlanVerdict::KeepPlanning) => {
                Err("plan not approved; keep planning".to_string())
            }
            Ok(PlanVerdict::Dismissed) => {
                Err("plan review dismissed; stay in plan mode and wait for the user's next message".to_string())
            }
            // 通道关闭（宿主接管 / 会话结束）→ 视为 dismissed：plan 保持，等待下一条消息。
            Err(_) => Err(
                "plan review dismissed; stay in plan mode and wait for the user's next message"
                    .to_string(),
            ),
        }
    }
}

/// 设置 plan 模式的返回态。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetResult {
    /// 已立即生效（无 pending 被挂起）。
    Committed,
    /// 已挂起 pending，等待 step 边界追加。
    Queued,
    /// 已是目标状态，无变化。
    Noop,
}

impl Default for PlanMode {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
