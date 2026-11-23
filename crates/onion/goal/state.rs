//! goal 状态机：相位（durable）+ 激活（process-local）双轨。
//!
//! 相位转换前置条件 + 激活边确定分离。
//! 本模块只做纯状态转换（不写 durable 事件），落盘由 service 层负责。

use super::types::{
    GoalActivation, GoalBlockReason, GoalError, GoalPhase, GoalRef, GoalSnapshot,
};

/// goal 进程内存状态：durable 相位 + 本地激活双轨。
#[derive(Clone, Debug)]
pub struct GoalState {
    /// durable 相位（None = 无目标）。
    pub snapshot: Option<GoalSnapshot>,
    /// 进程内激活（永不落盘）。
    pub activation: GoalActivation,
    /// 派生：只随真实进入历史的 goal 轮递增。
    pub rounds_started: u32,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

impl Default for GoalState {
    fn default() -> Self {
        Self {
            snapshot: None,
            activation: GoalActivation::Disarmed,
            rounds_started: 0,
            created_at_ms: 0,
            updated_at_ms: 0,
        }
    }
}

impl GoalState {
    pub fn current_ref(&self) -> Option<GoalRef> {
        self.snapshot.as_ref().map(GoalSnapshot::ref_of)
    }

    /// create：无目标或 phase=Complete 时创建（active+armed，rounds=0）。
    pub fn create(
        &mut self,
        objective: impl Into<String>,
        max_goal_rounds: u32,
        now_ms: u64,
    ) -> Result<GoalSnapshot, GoalError> {
        if let Some(current) = &self.snapshot {
            if current.phase != GoalPhase::Complete {
                return Err(GoalError::AlreadyExists(current.phase));
            }
        }
        let snapshot = GoalSnapshot {
            id: format!("goal-{now_ms}"),
            revision: 1,
            objective: objective.into(),
            phase: GoalPhase::Active,
            blocked_reason: None,
            max_goal_rounds,
        };
        self.snapshot = Some(snapshot.clone());
        self.activation = GoalActivation::Armed;
        self.rounds_started = 0;
        self.created_at_ms = now_ms;
        self.updated_at_ms = now_ms;
        Ok(snapshot)
    }

    /// edit：revision 匹配；至少一个字段；相位不变；沿用当前激活。
    pub fn edit(
        &mut self,
        reference: GoalRef,
        objective: Option<String>,
        max_goal_rounds: Option<u32>,
        now_ms: u64,
    ) -> Result<GoalSnapshot, GoalError> {
        let current = self.snapshot.as_mut().ok_or(GoalError::InvalidTransition {
            from: GoalPhase::Active,
            to: GoalPhase::Active,
        })?;
        Self::check_revision(reference, current)?;
        if objective.is_none() && max_goal_rounds.is_none() {
            return Err(GoalError::InvalidTransition {
                from: current.phase,
                to: current.phase,
            });
        }
        if let Some(objective) = objective {
            current.objective = objective;
        }
        if let Some(max) = max_goal_rounds {
            current.max_goal_rounds = max;
        }
        current.revision += 1;
        self.updated_at_ms = now_ms;
        Ok(current.clone())
    }

    /// pause：仅 active → paused（disarm）。
    pub fn pause(&mut self, reference: GoalRef, now_ms: u64) -> Result<GoalSnapshot, GoalError> {
        let current = self.snapshot.as_mut().ok_or(GoalError::InvalidTransition {
            from: GoalPhase::Active,
            to: GoalPhase::Paused,
        })?;
        Self::check_revision(reference, current)?;
        if current.phase != GoalPhase::Active {
            return Err(GoalError::InvalidTransition { from: current.phase, to: GoalPhase::Paused });
        }
        current.phase = GoalPhase::Paused;
        current.revision += 1;
        self.activation = GoalActivation::Disarmed;
        self.updated_at_ms = now_ms;
        Ok(current.clone())
    }

    /// resume：active/paused/blocked → active（armed）。
    /// 拒绝：已是 active+armed；预算耗尽。
    pub fn resume(&mut self, reference: GoalRef, now_ms: u64) -> Result<GoalSnapshot, GoalError> {
        let current = self.snapshot.as_mut().ok_or(GoalError::InvalidTransition {
            from: GoalPhase::Active,
            to: GoalPhase::Active,
        })?;
        Self::check_revision(reference, current)?;
        if !matches!(current.phase, GoalPhase::Active | GoalPhase::Paused | GoalPhase::Blocked) {
            return Err(GoalError::InvalidTransition { from: current.phase, to: GoalPhase::Active });
        }
        if current.phase == GoalPhase::Active && self.activation == GoalActivation::Armed {
            return Err(GoalError::InvalidTransition { from: current.phase, to: GoalPhase::Active });
        }
        if self.rounds_started >= current.max_goal_rounds {
            return Err(GoalError::RoundBudgetExhausted {
                rounds_started: self.rounds_started,
                max: current.max_goal_rounds,
            });
        }
        current.phase = GoalPhase::Active;
        current.revision += 1;
        self.activation = GoalActivation::Armed;
        self.updated_at_ms = now_ms;
        Ok(current.clone())
    }

    /// complete：任意非 complete 相位 → complete（disarm）。
    pub fn complete(&mut self, reference: GoalRef, now_ms: u64) -> Result<GoalSnapshot, GoalError> {
        let current = self.snapshot.as_mut().ok_or(GoalError::InvalidTransition {
            from: GoalPhase::Complete,
            to: GoalPhase::Complete,
        })?;
        Self::check_revision(reference, current)?;
        if current.phase == GoalPhase::Complete {
            return Err(GoalError::InvalidTransition { from: current.phase, to: GoalPhase::Complete });
        }
        current.phase = GoalPhase::Complete;
        current.revision += 1;
        self.activation = GoalActivation::Disarmed;
        self.updated_at_ms = now_ms;
        Ok(current.clone())
    }

    /// block：仅 active → blocked（disarm），带 blockedReason。
    pub fn block(
        &mut self,
        reference: GoalRef,
        reason: GoalBlockReason,
        now_ms: u64,
    ) -> Result<GoalSnapshot, GoalError> {
        let current = self.snapshot.as_mut().ok_or(GoalError::InvalidTransition {
            from: GoalPhase::Active,
            to: GoalPhase::Blocked,
        })?;
        Self::check_revision(reference, current)?;
        if current.phase != GoalPhase::Active {
            return Err(GoalError::InvalidTransition { from: current.phase, to: GoalPhase::Blocked });
        }
        current.phase = GoalPhase::Blocked;
        current.blocked_reason = Some(reason);
        current.revision += 1;
        self.activation = GoalActivation::Disarmed;
        self.updated_at_ms = now_ms;
        Ok(current.clone())
    }

    /// clear：写 tombstone（snapshot → None），revision 语义由 service 落盘。
    pub fn clear(&mut self, reference: GoalRef) -> Result<(), GoalError> {
        let current = self.snapshot.as_ref().ok_or(GoalError::InvalidTransition {
            from: GoalPhase::Complete,
            to: GoalPhase::Complete,
        })?;
        Self::check_revision(reference, current)?;
        self.snapshot = None;
        self.activation = GoalActivation::Disarmed;
        Ok(())
    }

    /// disarm：纯进程内（不落盘、不增 revision）。返回是否原本 armed
    /// （仅 Active 且 Armed 时生效并返回 true；幂等）。
    pub fn disarm(&mut self) -> bool {
        if !matches!(
            self.activation,
            GoalActivation::Armed
        ) {
            return false;
        }
        self.activation = GoalActivation::Disarmed;
        true
    }

    /// 推进一轮：校验「目标 active + id/revision 匹配 + round 恰好为下一轮 + 预算未耗尽」。
    /// 非法推进返回错误（fail-loud，严格重放）。
    pub fn advance_round(
        &mut self,
        goal_id: &str,
        revision: u32,
        round: u32,
    ) -> Result<(), GoalError> {
        let current = self.snapshot.as_ref().ok_or(GoalError::RoundNotAdmitted { round })?;
        if current.phase != GoalPhase::Active {
            return Err(GoalError::RoundNotAdmitted { round });
        }
        if current.id != goal_id || current.revision != revision {
            return Err(GoalError::RevisionMismatch { expected: current.revision, got: revision });
        }
        if round > current.max_goal_rounds {
            return Err(GoalError::RoundBudgetExhausted {
                rounds_started: self.rounds_started,
                max: current.max_goal_rounds,
            });
        }
        if round != self.rounds_started + 1 {
            return Err(GoalError::RoundNotAdmitted { round });
        }
        self.rounds_started = round;
        Ok(())
    }

    fn check_revision(reference: GoalRef, current: &GoalSnapshot) -> Result<(), GoalError> {
        if reference.revision != current.revision {
            return Err(GoalError::RevisionMismatch {
                expected: current.revision,
                got: reference.revision,
            });
        }
        Ok(())
    }
}

/// 从 durable 事件重放投影出 durable 相位（激活一律 Disarmed，冷恢复不 armed）。
/// 外部/重放写入的 goal/change 一律 disarmed + fold 严格校验：
/// - **id 复用拒绝**（`seenGoalIds`）：一个 goal id 在整个事件流中只能出现一次，
///   被 clear（snapshot=None）或切换目标后，旧 id 不得再次出现；
/// - **字段 fail-loud**：`rounds_started` 必须非递减；`updated_at >= created_at`。
///   校验失败返回 `Replay` 错误（拒绝用非法事件流污染投影）。
///
pub fn replay_goal(
    events: &[crate::session::SessionEvent],
    activation: GoalActivation,
) -> Result<GoalState, GoalError> {
    let mut state = GoalState { activation, ..GoalState::default() };
    let mut seen_goal_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut current_id: Option<String> = None;
    let mut last_rounds: Option<u32> = None;
    for event in events {
        let crate::session::SessionEvent::GoalChange {
            operation,
            snapshot,
            rounds_started,
            created_at_ms,
            updated_at_ms,
        } = event
        else {
            continue;
        };
        let _ = operation;
        // 字段 fail-loud。
        if created_at_ms > updated_at_ms {
            return Err(GoalError::Replay {
                reason: format!("created_at({created_at_ms}) > updated_at({updated_at_ms})"),
            });
        }
        // id 复用拒绝（seenGoalIds）。
        if let Some(snap) = snapshot {
            let reuse = match &current_id {
                Some(cur) => cur != &snap.id && seen_goal_ids.contains(&snap.id),
                None => seen_goal_ids.contains(&snap.id),
            };
            if reuse {
                return Err(GoalError::Replay {
                    reason: format!("goal id 复用: {}", snap.id),
                });
            }
            // rounds 属于 goal 实例：换新 goal 时计数从 0 重新开始，
            // 非递减校验只在同一 id 内成立（clear 后重建是合法序列）。
            if current_id.as_deref() != Some(snap.id.as_str()) {
                last_rounds = None;
            }
            seen_goal_ids.insert(snap.id.clone());
            current_id = Some(snap.id.clone());
        } else {
            current_id = None;
            last_rounds = None;
        }
        // 字段 fail-loud：同一 goal 内 rounds_started 必须非递减。
        if let Some(last) = last_rounds {
            if *rounds_started < last {
                return Err(GoalError::Replay {
                    reason: format!("rounds_started 回退: {last} -> {rounds_started}"),
                });
            }
        }
        last_rounds = Some(*rounds_started);
        state.rounds_started = *rounds_started;
        state.created_at_ms = *created_at_ms;
        state.updated_at_ms = *updated_at_ms;
        state.snapshot = snapshot.clone();
    }
    Ok(state)
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
