//! 会话轮次闸门：driver 唯一性 + 在飞计数 + 事件驱动等待。
//!
//! 替代裸 `AtomicBool busy`（无归属，并发 `POST /messages` 会起两个 driver，
//! 先结束者把 busy 置 false 导致 `wait=true` 提前返回）。三件套：
//! `phase.kind` 状态机（driver 唯一性）、`activityDone`（在飞活动）、
//! `whenIdle()`（事件驱动等待，无轮询）。
//!
//! 语义：
//! - `enqueue()`：外部输入入队，返回本次序号（`wait=true` 的等待目标）；
//! - `try_acquire_driver()`：CAS 抢 driver，抢不到说明已有 driver 在消费 inbox；
//! - `complete_one()`：driver 完成一轮，计数 +1 并在追平时唤醒等待者；
//! - `wait_for(seq)`：等到 `completed >= seq`（本次请求对应的轮次结束）。

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use tokio::sync::Notify;

/// 会话轮次闸门（每会话一个）。
#[derive(Default)]
pub(crate) struct TurnGate {
    /// driver 唯一性：CAS 抢占。
    driver_active: AtomicBool,
    /// 已入队轮次数。
    enqueued: AtomicU64,
    /// 已完成轮次数。
    completed: AtomicU64,
    /// 追平通知（替代 50ms 轮询）。
    idle: Notify,
}

impl TurnGate {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// 入队一次轮次请求，返回本次序号（1 起）。
    pub(crate) fn enqueue(&self) -> u64 {
        self.enqueued.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// 抢占 driver；已有 driver 时返回 false（调用方应直接返回，由现有 driver 消费 inbox）。
    pub(crate) fn try_acquire_driver(&self) -> bool {
        self.driver_active
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// 释放 driver。
    pub(crate) fn release_driver(&self) {
        self.driver_active.store(false, Ordering::SeqCst);
    }

    /// 完成一轮：计数 +1；追平已入队数时唤醒全部等待者。
    pub(crate) fn complete_one(&self) {
        let completed = self.completed.fetch_add(1, Ordering::SeqCst) + 1;
        if completed >= self.enqueued.load(Ordering::SeqCst) {
            self.idle.notify_waiters();
        }
    }

    /// driver 退出兜底：把 `completed` 拉到 `enqueued` 并唤醒等待者，防 `wait=true` 悬挂。
    pub(crate) fn drain_remaining(&self) {
        let enqueued = self.enqueued.load(Ordering::SeqCst);
        self.completed.store(enqueued, Ordering::SeqCst);
        self.idle.notify_waiters();
    }

    /// 是否有在飞或排队的轮次。
    pub(crate) fn is_busy(&self) -> bool {
        self.enqueued.load(Ordering::SeqCst) > self.completed.load(Ordering::SeqCst)
    }

    /// 序号 `seq` 对应的轮次是否已完成（供需同时轮询其他条件的调用方使用）。
    pub(crate) fn is_done(&self, seq: u64) -> bool {
        self.completed.load(Ordering::SeqCst) >= seq
    }

    /// 等到序号 `seq` 对应的轮次完成（已追平则立即返回）。
    pub(crate) async fn wait_for(&self, seq: u64) {
        loop {
            // 先注册等待再检查，避免「检查后、注册前」的完成丢失唤醒。
            let notified = self.idle.notified();
            if self.completed.load(Ordering::SeqCst) >= seq {
                return;
            }
            notified.await;
        }
    }
}

#[cfg(test)]
#[path = "turn_gate_tests.rs"]
mod tests;
