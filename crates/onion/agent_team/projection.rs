//! durable 事件重放投影。
//!
//! 从 Lead Session 的 durable 事件流回放重建 `TeamState`，并做强不变量校验：
//! name 不复用、新成员必须 provisioning 起步、任务 revision 连续、nextTaskNumber
//! 单调推进。投影只读，变更由 service 事务内 append 事件后触发增量重放。

use crate::agent_team::types::{
    TeamMemberPhase, TeamMemberSnapshot, TeamState, TeamTaskSnapshot,
};

/// 可被投影消费的团队事件（与 SessionEvent 的 `TeamMemberChange`/`TeamTaskChange`
/// 载荷对应）。
#[derive(Clone, Debug)]
pub enum TeamEvent {
    Member(TeamMemberSnapshot),
    Task(TeamTaskSnapshot),
}

impl TeamEvent {
    /// 事件类型名。
    pub fn type_name(&self) -> &'static str {
        match self {
            TeamEvent::Member(_) => "team/member",
            TeamEvent::Task(_) => "team/task",
        }
    }
}

/// 空团队状态（无成员、无任务，任务号从 1 起步）。
pub fn empty_team_state() -> TeamState {
    TeamState {
        members: Vec::new(),
        tasks: Vec::new(),
        next_task_number: 1,
        names: Default::default(),
    }
}

/// 增量应用一个团队事件到投影状态。
///
/// 返回 `Err(ProjectionError)` 表示强不变量被违反（事件流损坏）。
pub fn apply_event(state: &mut TeamState, event: &TeamEvent) -> Result<(), ProjectionError> {
    match event {
        TeamEvent::Member(member) => apply_member(state, member),
        TeamEvent::Task(task) => apply_task(state, task),
    }
}

fn apply_member(state: &mut TeamState, member: &TeamMemberSnapshot) -> Result<(), ProjectionError> {
    // name 不可被他人复用。
    if let Some(&existing_idx) = state.names.get(&member.name) {
        let existing = &state.members[existing_idx];
        if existing.id != member.id {
            return Err(ProjectionError::NameReused(member.name.clone()));
        }
    }
    // 已有该成员 → 增量替换（保留引用语义，仅替换该槽）。
    if let Some(idx) = state.members.iter().position(|m| m.id == member.id) {
        // 不可变字段（name/provider/context）不可变。
        let old = &state.members[idx];
        if old.name != member.name || old.provider != member.provider || old.context != member.context {
            return Err(ProjectionError::ImmutableFieldChanged(member.id.clone()));
        }
        state.members[idx] = member.clone();
        return Ok(());
    }
    // 新成员必须以 provisioning 起步。
    if member.phase != TeamMemberPhase::Provisioning {
        return Err(ProjectionError::MemberNotProvisioning(member.name.clone()));
    }
    state.names.insert(member.name.clone(), state.members.len());
    state.members.push(member.clone());
    Ok(())
}

fn apply_task(state: &mut TeamState, task: &TeamTaskSnapshot) -> Result<(), ProjectionError> {
    if let Some(idx) = state.tasks.iter().position(|t| t.id == task.id) {
        // revision 必须连续 +1。
        if task.revision != state.tasks[idx].revision + 1 {
            return Err(ProjectionError::RevisionGap(task.id.clone()));
        }
        state.tasks[idx] = task.clone();
    } else {
        // 新任务必须 revision=1。
        if task.revision != 1 {
            return Err(ProjectionError::RevisionGap(task.id.clone()));
        }
        state.tasks.push(task.clone());
    }
    // next_task_number 推进为 max(现有, number===MAX_SAFE ? number : number+1)。
    let number = task
        .id
        .strip_prefix("task-")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    let candidate = if number == u64::MAX { number } else { number + 1 };
    if candidate > state.next_task_number {
        state.next_task_number = candidate;
    }
    Ok(())
}

/// 投影错误（事件流损坏，不可恢复）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionError {
    NameReused(String),
    ImmutableFieldChanged(String),
    MemberNotProvisioning(String),
    RevisionGap(String),
}

impl std::fmt::Display for ProjectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectionError::NameReused(name) => write!(f, "member name reused: {name}"),
            ProjectionError::ImmutableFieldChanged(id) => write!(f, "immutable field changed: {id}"),
            ProjectionError::MemberNotProvisioning(name) => write!(f, "member not provisioning: {name}"),
            ProjectionError::RevisionGap(id) => write!(f, "revision gap: {id}"),
        }
    }
}

#[cfg(test)]
#[path = "projection_tests.rs"]
mod tests;
