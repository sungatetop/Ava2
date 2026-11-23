//! GoalService：事件溯源投影 + CAS 变更 + rounds 推进 + 权威判定。
//!
//! - durable 相位从 `GoalChange` 事件流重放投影（严格重放，非法推进 fail-loud）；
//! - 激活（process-local）单独维护，永不落盘；冷恢复一律 Disarmed；
//! - CAS revision：每次变更递增，调用方带 revision 防并发覆盖；
//! - 每次变更 append 一条 `GoalChange` 事件（「model-visible means logged」）。

use std::sync::{Arc, Mutex, OnceLock};

use crate::session::SessionLog;

use super::state::{replay_goal, GoalState};
use super::types::{
    GoalActivation, GoalBlockReason, GoalError, GoalPhase, GoalRef, GoalSnapshot, GoalView,
};

/// 会话句柄接线槽：GoalService 构造早于父会话 Runtime 装配（SessionLog 装配后
/// 才存在），经 OnceLock 延迟接线（`wire_session`，宿主装配完成后调用）。
pub struct GoalService {
    state: Mutex<GoalState>,
    session: Arc<OnceLock<Arc<SessionLog>>>,
    /// 进程内「当前是否 goal 自动续轮」标记（driver 驱动续轮时置位，轮后复位；
    /// 不落盘）。工具面据此区分 goal-round 权威。
    in_goal_round: Mutex<bool>,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl GoalService {
    /// 构造：初始无目标（durable 相位由 wire 后重放恢复）。
    pub fn new() -> Self {
        Self {
            state: Mutex::new(GoalState::default()),
            session: Arc::new(OnceLock::new()),
            in_goal_round: Mutex::new(false),
        }
    }

    /// 标记进入 goal 自动续轮（driver 构造 continuation_input 时调用）。
    pub fn enter_goal_round(&self) {
        *lock(&self.in_goal_round) = true;
    }

    /// 标记退出 goal 自动续轮（driver 轮结束后调用）。
    pub fn exit_goal_round(&self) {
        *lock(&self.in_goal_round) = false;
    }

    /// 当前是否处于 goal 自动续轮（工具面权威判定用）。
    pub fn in_goal_round(&self) -> bool {
        *lock(&self.in_goal_round)
    }

    /// 延迟接线：从 SessionLog 重放 GoalChange 事件恢复 durable 相位。
    /// 激活一律 Disarmed（冷恢复不 armed，需人工 resume）。
    /// 重放严格校验（id 复用 / 字段 fail-loud）失败时 panic（编程/日志损坏错误）。
    pub fn wire_session(&self, session: Arc<SessionLog>) {
        let _ = self.session.set(session.clone());
        let events = session.snapshot();
        let state = replay_goal(&events, GoalActivation::Disarmed)
            .unwrap_or_else(|error| panic!("goal 重放校验失败: {error}"));
        *lock(&self.state) = state;
    }

    /// 当前视图（快照 + 派生值 + 进程内激活）。
    pub fn view(&self) -> Option<GoalView> {
        let state = lock(&self.state);
        let snapshot = state.snapshot.clone()?;
        Some(GoalView {
            snapshot,
            rounds_started: state.rounds_started,
            created_at_ms: state.created_at_ms,
            updated_at_ms: state.updated_at_ms,
            activation: state.activation,
        })
    }

    /// 目标是否可被自动续轮驱动（active + armed）。
    ///
    /// **不含预算判据**——预算耗尽由 driver 在开轮前
    /// 判定并落 `round-limit` block（若在此处提前返回 false，driver 永远走不到
    /// block 分支，目标会永久停在 active+armed 且不再续轮）。
    pub fn should_continue(&self) -> bool {
        let state = lock(&self.state);
        matches!(
            (&state.snapshot, state.activation),
            (Some(goal), GoalActivation::Armed) if goal.phase == GoalPhase::Active
        )
    }

    /// 当前激活（工具面观测；永不作为重放依据）。
    pub fn activation(&self) -> GoalActivation {
        lock(&self.state).activation
    }

    /// 当前轮号（下一个待续轮 = rounds_started + 1）。
    pub fn next_round(&self) -> Option<u32> {
        let state = lock(&self.state);
        state.snapshot.as_ref().map(|_| state.rounds_started + 1)
    }

    /// 构造下一轮输入消息的 source 锚点（GoalRound { id, revision, round }）。
    pub fn round_source(&self) -> Option<crate::types::MessageSource> {
        let state = lock(&self.state);
        let goal = state.snapshot.as_ref()?;
        Some(crate::types::MessageSource::GoalRound {
            goal_id: goal.id.clone(),
            revision: goal.revision,
            round: state.rounds_started + 1,
        })
    }

    /// 记录下一轮已真实进入历史（driver 驱动 run 返回后调用）。
    /// 经 durable 事件推进 round（严格校验）。
    pub fn record_round_entered(&self) -> Result<(), GoalError> {
        let source = self
            .round_source()
            .ok_or(GoalError::RoundNotAdmitted { round: 0 })?;
        let (goal_id, revision, round) = match source {
            crate::types::MessageSource::GoalRound { goal_id, revision, round } => {
                (goal_id, revision, round)
            }
            _ => unreachable!(),
        };
        let mut state = lock(&self.state);
        state.advance_round(&goal_id, revision, round)?;
        // 落 durable 事件：round 推进不改变快照，只更新 rounds_started 计数器。
        let snapshot = state.snapshot.clone();
        let rounds_started = state.rounds_started;
        let created_at_ms = state.created_at_ms;
        let updated_at_ms = state.updated_at_ms;
        drop(state);
        if let Some(session) = self.session.get() {
            session.append(crate::session::SessionEvent::GoalChange {
                operation: "round".to_string(),
                snapshot,
                rounds_started,
                created_at_ms,
                updated_at_ms,
            });
        }
        Ok(())
    }

    fn mutate(
        &self,
        operation: &str,
        f: impl FnOnce(&mut GoalState) -> Result<GoalSnapshot, GoalError>,
    ) -> Result<GoalSnapshot, GoalError> {
        let mut state = lock(&self.state);
        let snapshot = f(&mut state)?;
        let rounds_started = state.rounds_started;
        let created_at_ms = state.created_at_ms;
        let updated_at_ms = state.updated_at_ms;
        drop(state);
        if let Some(session) = self.session.get() {
            session.append(crate::session::SessionEvent::GoalChange {
                operation: operation.to_string(),
                snapshot: Some(snapshot.clone()),
                rounds_started,
                created_at_ms,
                updated_at_ms,
            });
        }
        Ok(snapshot)
    }

    pub fn create(
        &self,
        objective: impl Into<String>,
        max_goal_rounds: u32,
    ) -> Result<GoalSnapshot, GoalError> {
        let objective = objective.into();
        self.mutate("create", |state| {
            let now = crate::session::epoch_millis();
            state.create(objective, max_goal_rounds, now)
        })
    }

    pub fn edit(
        &self,
        reference: GoalRef,
        objective: Option<String>,
        max_goal_rounds: Option<u32>,
    ) -> Result<GoalSnapshot, GoalError> {
        self.mutate("edit", |state| {
            state.edit(reference, objective, max_goal_rounds, crate::session::epoch_millis())
        })
    }

    pub fn pause(&self, reference: GoalRef) -> Result<GoalSnapshot, GoalError> {
        self.mutate("pause", |state| {
            state.pause(reference, crate::session::epoch_millis())
        })
    }

    pub fn resume(&self, reference: GoalRef) -> Result<GoalSnapshot, GoalError> {
        self.mutate("resume", |state| {
            state.resume(reference, crate::session::epoch_millis())
        })
    }

    pub fn complete(&self, reference: GoalRef) -> Result<GoalSnapshot, GoalError> {
        self.mutate("complete", |state| {
            state.complete(reference, crate::session::epoch_millis())
        })
    }

    pub fn block(
        &self,
        reference: GoalRef,
        reason: GoalBlockReason,
    ) -> Result<GoalSnapshot, GoalError> {
        self.mutate("block", |state| {
            state.block(reference, reason, crate::session::epoch_millis())
        })
    }

    pub fn clear(&self, reference: GoalRef) -> Result<(), GoalError> {
        let mut state = lock(&self.state);
        state.clear(reference)?;
        let rounds_started = state.rounds_started;
        let created_at_ms = state.created_at_ms;
        let updated_at_ms = state.updated_at_ms;
        drop(state);
        if let Some(session) = self.session.get() {
            session.append(crate::session::SessionEvent::GoalChange {
                operation: "clear".to_string(),
                snapshot: None,
                rounds_started,
                created_at_ms,
                updated_at_ms,
            });
        }
        Ok(())
    }

    /// disarm：纯进程内（不落盘）。返回是否原本 armed。
    pub fn disarm(&self) -> bool {
        lock(&self.state).disarm()
    }

    /// 模型自阻塞阈值检查：round < 阈值时拒绝模型 blocked（direct-human 绕过）。
    pub fn check_block_threshold(&self, rounds_started: u32) -> Result<(), GoalError> {
        if rounds_started < super::driver::BLOCKED_AFTER_CONSECUTIVE_ROUNDS {
            return Err(GoalError::BlockThreshold {
                rounds: rounds_started,
                min: super::driver::BLOCKED_AFTER_CONSECUTIVE_ROUNDS,
            });
        }
        Ok(())
    }

    /// 权威分级（基于事件流 source 而非进程标记）。
    ///
    /// 判定依据是 durable 事件流中「最近一条 UserMessage」的来源：
    /// - 目标是 `GoalRound` 且 goal_id 匹配当前视图 → GoalRound 权威（本轮是 goal
    ///   自动续轮，模型可触发 complete/blocked）；
    /// - 否则（普通 user 输入或 None）→ DirectHuman 权威。
    ///
    ///   目标非 active 或无目标时不给 goal-round 权威。
    ///
    /// 相比旧实现（仅靠 `in_goal_round` 进程标记），source 化能区分「真实人类输入
    /// 轮」与「普通模型回合」，避免把非 goal 触发的模型回合误判为 goal-round 权威。
    pub fn authority(&self) -> Authority {
        let state = lock(&self.state);
        let active = match &state.snapshot {
            Some(goal) => goal.phase == GoalPhase::Active,
            None => return Authority::None,
        };
        let goal_id = state.snapshot.as_ref().map(|g| g.id.clone());
        let round = state.rounds_started;
        drop(state);

        if !active {
            return Authority::DirectHuman;
        }
        match self.current_turn_source() {
            Some(crate::types::MessageSource::GoalRound {
                goal_id: source_goal,
                round: source_round,
                ..
            }) if goal_id.as_deref() == Some(source_goal.as_str()) => {
                // goal 续轮权威：round 取当前已推进的轮号（source.round 与之一致）。
                let _ = source_round;
                Authority::GoalRound { round }
            }
            _ => Authority::DirectHuman,
        }
    }

    /// 注入 wrap-up 引导：goal-round 权威下
    /// complete/blocked 成功后，向会话追加一条模型可见的收尾 user 消息，让模型
    /// 在轮结束前向用户总结。仅追加，不改目标状态、不落 goal 事件。
    pub fn inject_wrapup(&self, outcome: &str) {
        let Some(session) = self.session.get() else { return };
        let objective = lock(&self.state)
            .snapshot
            .as_ref()
            .map(|g| g.objective.clone())
            .unwrap_or_default();
        let text = super::driver::wrapup_prompt(&objective, outcome);
        let message = crate::types::AgentMessage::user(text);
        session.append(crate::session::SessionEvent::UserMessage {
            turn: 0,
            step: 0,
            message,
        });
    }

    /// 从 durable 事件流读取「最近一条 UserMessage」的来源（权威判定锚点）。
    /// 若无 session 接线或事件流为空 → None（视为无来源，回落 DirectHuman）。
    fn current_turn_source(&self) -> Option<crate::types::MessageSource> {
        let session = self.session.get()?;
        let events = session.snapshot();
        // filter_map 取全部 UserMessage 的 source，rev 后取第一个 = 最近一条 UserMessage
        // 的 source（即使为 None 也停住，不往前找更早的 GoalRound）。
        // 压缩检查点消息不是回合输入（它是上下文替换载体），跳过以免污染权威判定。
        events
            .iter()
            .rev()
            .filter_map(|event| match event {
                crate::session::SessionEvent::UserMessage { message, .. } => {
                    match message.source {
                        Some(crate::types::MessageSource::CompactCheckpoint { .. }) => None,
                        _ => Some(message.source.clone()),
                    }
                }
                _ => None,
            })
            .next()
            .flatten()
    }
}

/// 权威（goal 工具面分级）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Authority {
    DirectHuman,
    GoalRound { round: u32 },
    None,
}

impl Default for GoalService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
