//! agent-team 纯类型（成员/任务/视图/请求结构）。
//!
//! types.rs 不反向依赖 durable 层（SessionLog），保持纯数据模型可测。
//! 快照类型（`TeamMemberSnapshot`/`TeamTaskSnapshot`）需 `Serialize`/`Deserialize`
//! 以承载进 `SessionEvent`（durable JSONL 落盘）；视图/请求类仅内存交互。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 成员角色。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TeamRole {
    Lead,
    Teammate,
}

/// 成员创建上下文（全新独立会话或继承父会话前缀）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TeamContext {
    Fresh,
    Fork,
}

/// 成员阶段（durable，roster 状态机 provisioning -> active|failed）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TeamMemberPhase {
    Provisioning,
    Active,
    Failed,
}

/// 成员运行时状态（model-facing view）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TeamMemberStatus {
    Running,
    Inactive,
    Provisioning,
    Failed,
}

/// 成员快照（全量 durable 值，每次变更 append 新快照）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamMemberSnapshot {
    pub id: String,
    pub name: String,
    pub description: String,
    pub provider: String,
    pub context: TeamContext,
    pub phase: TeamMemberPhase,
    pub error: Option<String>,
}

/// 成员运行时视图（model-facing；Lead 伪行省略 provisioning 字段）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamMemberView {
    pub id: String,
    pub name: String,
    pub role: TeamRole,
    pub status: TeamMemberStatus,
    pub description: Option<String>,
    pub provider: Option<String>,
    pub context: Option<TeamContext>,
    pub diagnostics: Vec<String>,
}

/// 任务状态机。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TeamTaskStatus {
    Pending,
    InProgress,
    Completed,
    Deleted,
}

/// 任务快照（全量 durable 值）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamTaskSnapshot {
    pub id: String,
    pub revision: u64,
    pub subject: String,
    pub description: String,
    pub status: TeamTaskStatus,
    pub owner_id: Option<String>,
    pub blocked_by: Vec<String>,
    pub write_scopes: Vec<String>,
}

/// 任务视图（model-facing）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamTaskView {
    pub id: String,
    pub revision: u64,
    pub subject: String,
    pub description: String,
    pub status: TeamTaskStatus,
    pub owner_name: Option<String>,
    pub blocked_by: Vec<String>,
    pub write_scopes: Vec<String>,
    pub ready: bool,
    pub write_scope_warnings: Vec<String>,
}

/// 任务创建请求。
#[derive(Clone, Debug, Default)]
pub struct CreateTeamTaskRequest {
    pub subject: String,
    pub description: String,
    pub blocked_by: Vec<String>,
    pub write_scopes: Vec<String>,
}

/// 任务更新动作。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeamTaskAction {
    Claim,
    Release,
    Edit,
    SetDependencies,
    Complete,
    Reopen,
    Reassign,
    Delete,
}

/// 任务更新请求。
#[derive(Clone, Debug, Default)]
pub struct UpdateTeamTaskRequest {
    pub task_id: String,
    pub expected_revision: u64,
    pub action: Option<TeamTaskAction>,
    pub subject: Option<String>,
    pub description: Option<String>,
    pub blocked_by: Option<Vec<String>>,
    pub write_scopes: Option<Vec<String>>,
    pub owner: Option<String>,
}

/// 成员派生请求。
#[derive(Clone, Debug)]
pub struct SpawnTeammateRequest {
    pub name: String,
    pub description: String,
    pub prompt: String,
    pub context: TeamContext,
    pub provider: String,
}

/// 消息发送请求。
#[derive(Clone, Debug)]
pub struct SendTeamMessageRequest {
    pub target: String,
    pub content: Vec<crate::types::ContentBlock>,
}

/// 团队投影状态（重放重建）。
#[derive(Clone, Debug, Default)]
pub struct TeamState {
    pub members: Vec<TeamMemberSnapshot>,
    pub tasks: Vec<TeamTaskSnapshot>,
    /// 下一个任务序号（单调推进，永不复用）。
    pub next_task_number: u64,
    /// 成员名 → 快照索引（name 唯一性快速判定）。
    pub names: BTreeMap<String, usize>,
}
