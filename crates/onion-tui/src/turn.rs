//! turn.rs：TUI 侧 turn 驱动（对齐 onion-cli `spawn_turn`）。
//!
//! 语义与 onion-cli `main.rs::spawn_turn` 同源，全部输入经 inbox 投递，由本驱动
//! 以空轮消费（内核 `claim_open` / `drain_next_step` 取排队消息），无任何带
//! input 直驱。
//!
//! 与 onion-cli 的唯一形态差异：反馈不进 `println!`（全屏画面被污染），统一经
//! `feedback_tx` 通道回主循环渲染；轮报告经 `done_tx` 回主循环渲染摘要。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use onion::goal::driver::GoalRoundDriver;
use onion::kernel::{TurnEndReason, TurnReport};
use onion::plan::service::PlanMode;
use onion::runtime::Runtime;
use tokio::sync::mpsc;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// TUI 侧 turn 驱动句柄。
pub struct TurnDriver {
    runtime: Arc<Runtime>,
    busy: Arc<AtomicBool>,
    goal_driver: Arc<GoalRoundDriver>,
    plan_mode: Arc<PlanMode>,
    /// 反馈行通道（驱动 → 主循环渲染）。
    feedback_tx: mpsc::UnboundedSender<String>,
    /// 轮报告通道（驱动 → 主循环渲染摘要）。
    done_tx: mpsc::UnboundedSender<TurnReport>,
    /// 当前轮起始时刻（footer running 态显示 elapsed）。
    started_at: Arc<Mutex<Option<Instant>>>,
}

impl TurnDriver {
    pub fn new(
        runtime: Arc<Runtime>,
        busy: Arc<AtomicBool>,
        goal_driver: Arc<GoalRoundDriver>,
        plan_mode: Arc<PlanMode>,
        feedback_tx: mpsc::UnboundedSender<String>,
        done_tx: mpsc::UnboundedSender<TurnReport>,
    ) -> Self {
        Self {
            runtime,
            busy,
            goal_driver,
            plan_mode,
            feedback_tx,
            done_tx,
            started_at: Arc::new(Mutex::new(None)),
        }
    }

    /// 是否有一轮在飞（`Esc` / `Ctrl+C` 据此决定"取消"还是"退出"）。
    pub fn busy(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
    }

    /// 当前轮起始时刻（无在飞轮 = None）。
    pub fn started_at(&self) -> Option<Instant> {
        *lock(&self.started_at)
    }

    /// 开轮（对齐 onion-cli `spawn_turn`）：已在飞则不重复开。
    pub fn kick(self: &Arc<Self>) {
        if self.busy.swap(true, Ordering::SeqCst) {
            return;
        }
        *lock(&self.started_at) = Some(Instant::now());
        let driver = self.clone();
        tokio::spawn(async move {
            driver.drive().await;
        });
    }

    /// 轮循环：空轮消费 inbox 排队消息 → 续轮判据 → 收敛复位 busy。
    async fn drive(self: Arc<Self>) {
        loop {
            // Ok(None) = 无轮可开（空输入且无排队消息，正常空转）；
            // Err = 状态机拒绝开轮，必须给用户可见反馈，否则驱动静默死亡、
            // busy 悬挂为 true。
            let report = match self.runtime.run(Vec::new()).await {
                Ok(Some(report)) => report,
                Ok(None) => break,
                Err(error) => {
                    self.feedback(format!("（turn 启动失败：{error}）"));
                    break;
                }
            };
            // 轮因 max-tokens 结束（模型未收敛）→ disarm goal，停止自动续轮。
            if matches!(report.reason, TurnEndReason::MaxTokens) && self.goal_driver.disarm() {
                self.feedback("（goal 因 max-tokens 结束已 disarm，停止自动续轮）".to_string());
            }
            // 每轮边界：消费 open-turn 内挂起的 plan 模式切换。
            self.plan_mode.flush_pending();
            let turn = report.turn;
            if self.done_tx.send(report).is_err() {
                break;
            }
            if self.runtime.inbox().has_next_turn() {
                self.feedback(format!("（next-turn 有排队消息，开新一轮 t{}）", turn + 1));
                continue;
            }
            // 空闲唤醒自续：Stopping 窗口后到达的 steer 残留在 next-step。
            if self.runtime.inbox().has_next_step() && self.runtime.inbox().take_wakeup() {
                self.feedback(format!("（next-step 有引导消息，开新一轮 t{}）", turn + 1));
                continue;
            }
            // goal 自动续轮：目标 active + armed + 无排队人类消息。
            if self.goal_driver.should_continue() {
                if let Some(message) = self.goal_driver.continuation_input() {
                    self.feedback(format!("（goal 自动续轮，开新一轮 t{}）", turn + 1));
                    self.goal_driver.service_enter();
                    // followup 前 CAS 预留 round；失败说明目标已被并发变更。
                    if self.goal_driver.record_round().is_err() {
                        self.goal_driver.service_exit();
                        break;
                    }
                    if self.runtime.inbox().followup(message).is_err() {
                        self.goal_driver.service_exit();
                        break;
                    }
                    continue;
                }
            }
            break;
        }
        self.goal_driver.service_exit();
        // 收敛（busy 复位）瞬间若 inbox 仍有排队消息，再触发一次开轮，
        // 避免「队列有 pending 但无人开轮」的唤醒丢失。
        if self.runtime.inbox().has_pending() {
            self.busy.store(false, Ordering::SeqCst);
            *lock(&self.started_at) = None;
            self.kick();
            return;
        }
        self.busy.store(false, Ordering::SeqCst);
        *lock(&self.started_at) = None;
    }

    /// 反馈行送回主循环（渲染到反馈区，非 durable）。
    fn feedback(&self, line: String) {
        let _ = self.feedback_tx.send(line);
    }
}
