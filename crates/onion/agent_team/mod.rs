//! `agent-team` 工具族：具名队友 + 直接消息 + durable 共享任务板。
//!
//! 核心语义：
//! - Lead 派生具名队友（provisioning -> active|failed，名字唯一永不复用）；
//! - 成员间直接消息（走目标 Agent inbox，运行中 steer / 冷恢复）；
//! - durable 共享任务板（CAS revision、blockedBy DAG、write-scope 重叠告警）；
//! - wait_agent 等待团队变化；interrupt_agent 仅 Lead 中断当前 turn 保留 inbox。
//!
//! 装配为 opt-in（`ONION_AGENT_TEAM=1`）：启用时共享名工具
//! `send_message`/`list_agents`/`interrupt_agent` 覆盖 subagent 控制三件
//! （同名工具互斥，由装配组合决定挂载哪一组）。

pub mod activity;
pub mod config;
pub mod journal;
pub mod lifecycle;
pub mod projection;
pub mod service;
pub mod task_board;
pub mod task_graph;
pub mod task_view;
pub mod tools;
pub mod types;
pub mod validation;

pub use service::{TeamMembership, TeamService, TeamId};
pub use tools::TeamToolSet;

/// agent-team 错误（全部 `TEAM_*` 错误码）。
#[derive(Debug, thiserror::Error)]
pub enum TeamError {
    #[error("team config is invalid")]
    InvalidConfig,
    #[error("team is disposed")]
    Disposed,
    #[error("not a team member")]
    NotMember,
    #[error("team member not found: {0}")]
    MemberNotFound(String),
    #[error("team lead required")]
    LeadRequired,
    #[error("invalid team target: {0}")]
    InvalidTarget(String),
    #[error("invalid team member name: {0}")]
    InvalidMemberName(String),
    #[error("team member name already taken: {0}")]
    MemberNameTaken(String),
    #[error("team member limit reached")]
    MemberLimit,
    #[error("team provisioning conflict")]
    ProvisioningConflict,
    #[error("cannot send a message to yourself")]
    SelfMessage,
    #[error("team message too large")]
    MessageTooLarge,
    #[error("team task limit reached")]
    TaskLimit,
    #[error("team task not found: {0}")]
    TaskNotFound(String),
    #[error("team task stale revision")]
    TaskStaleRevision,
    #[error("team task is deleted")]
    TaskDeleted,
    #[error("team task unauthorized")]
    TaskUnauthorized,
    #[error("team task already claimed")]
    TaskAlreadyClaimed,
    #[error("team task is blocked")]
    TaskBlocked,
    #[error("invalid team task transition")]
    TaskInvalidTransition,
    #[error("invalid team argument: {0}")]
    InvalidArgument(String),
    #[error("team task dependency cycle")]
    TaskDependencyCycle,
    #[error("team task has dependents")]
    TaskHasDependents,
    #[error("invalid team timeout")]
    InvalidTimeout,
    #[error("team wait aborted")]
    WaitAborted,
    #[error("team disposal timeout")]
    DisposalTimeout,
    #[error("invalid team write scope: {0}")]
    InvalidWriteScope(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
