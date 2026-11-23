//! goal 词汇表：相位 / 激活 / CAS 身份 / 快照 / 视图 / 阻塞原因 / 错误。
//!
//! 类型契约先行。
//! 相位（durable）与激活（process-local）分离；本模块不反向依赖 SessionLog。

use serde::{Deserialize, Serialize};

/// goal 持久化相位（资源预算排除在相位之外）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalPhase {
    Active,
    Paused,
    Blocked,
    Complete,
}

impl GoalPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Blocked => "blocked",
            Self::Complete => "complete",
        }
    }
}

/// 进程内激活：是否允许自动续新轮（永不持久化）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GoalActivation {
    Armed,
    Disarmed,
}

/// CAS 身份：每次 durable 变更 revision+1。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoalRef {
    pub id: String,
    pub revision: u32,
}

/// 阻塞原因：稳定 code（lower-kebab）+ 人/模型可读 message。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalBlockReason {
    pub code: String,
    pub message: String,
}

impl GoalBlockReason {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self { code: code.into(), message: message.into() }
    }
}

/// 持久化快照。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalSnapshot {
    pub id: String,
    pub revision: u32,
    pub objective: String,
    pub phase: GoalPhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<GoalBlockReason>,
    pub max_goal_rounds: u32,
}

impl GoalSnapshot {
    pub fn ref_of(&self) -> GoalRef {
        GoalRef { id: self.id.clone(), revision: self.revision }
    }
}

/// 完整视图：快照 + 派生值 + 进程内激活。
#[derive(Clone, Debug)]
pub struct GoalView {
    pub snapshot: GoalSnapshot,
    pub rounds_started: u32,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub activation: GoalActivation,
}

/// goal 错误（教学子集，稳定错误码）。
#[derive(Debug, thiserror::Error)]
pub enum GoalError {
    #[error("goal already exists (phase {0:?})")]
    AlreadyExists(GoalPhase),
    #[error("invalid transition {from:?} -> {to:?}")]
    InvalidTransition { from: GoalPhase, to: GoalPhase },
    #[error("revision mismatch: expected {expected} got {got}")]
    RevisionMismatch { expected: u32, got: u32 },
    #[error("goal round budget exhausted ({rounds_started}/{max})")]
    RoundBudgetExhausted { rounds_started: u32, max: u32 },
    #[error("goal round {round} is not the next admitted round of the active goal")]
    RoundNotAdmitted { round: u32 },
    #[error("goal tool requires direct human authority")]
    DirectHumanRequired,
    #[error("goal tool requires goal-round authority (round {round})")]
    GoalRoundAuthorityRequired { round: u32 },
    #[error("cannot resume a durable paused goal via model")]
    ResumePaused,
    #[error("model-reported block below threshold ({rounds}/{min})")]
    BlockThreshold { rounds: u32, min: u32 },
    #[error("goal replay 校验失败: {reason}")]
    Replay { reason: String },
}
