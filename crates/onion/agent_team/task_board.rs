//! 任务板命令：CAS revision、状态机、授权、blockedBy DAG、write-scope、id 分配。
//!
//! task_board 是纯逻辑层：对 `TeamState` 就地变更并返回结果；durable 事件
//! append 由 service 层在事务内完成（本模块不触碰 SessionLog）。

use crate::agent_team::task_graph::{GraphCheck, GraphError};
use crate::agent_team::task_view;
use crate::agent_team::types::{
    CreateTeamTaskRequest, TeamRole, TeamState, TeamTaskAction, TeamTaskSnapshot, TeamTaskStatus,
    TeamTaskView, UpdateTeamTaskRequest,
};
use crate::agent_team::{validation, TeamError};

/// 创建任务。
///
/// - id = `task-<next_task_number>`；id 空间耗尽或活跃任务数 >= max_tasks →
///   `TEAM_TASK_LIMIT`；
/// - 初始 revision=1，subject<=200，description<=16384；
/// - blockedBy 做 DAG 校验；writeScopes 归一化。
pub fn create_task(
    state: &mut TeamState,
    req: &CreateTeamTaskRequest,
    max_tasks: u64,
) -> Result<TeamTaskSnapshot, TeamError> {
    // 活跃（非 deleted）任务数上限。
    let active = state.tasks.iter().filter(|t| t.status != TeamTaskStatus::Deleted).count() as u64;
    if active >= max_tasks {
        return Err(TeamError::TaskLimit);
    }
    let id = format!("task-{}", state.next_task_number);
    if state.tasks.iter().any(|t| t.id == id) {
        return Err(TeamError::TaskLimit);
    }
    let subject = validation::required_text(&req.subject, 200)?;
    // description 可选（允许空串，缺省为空）。
    let description = if req.description.trim().is_empty() {
        String::new()
    } else {
        validation::required_text(&req.description, 16_384)?
    };
    let blocked_by = resolve_blockers(state, &req.blocked_by, &id)?;
    let write_scopes = validation::write_scopes(&req.write_scopes)?;
    let task = TeamTaskSnapshot {
        id,
        revision: 1,
        subject,
        description,
        status: TeamTaskStatus::Pending,
        owner_id: None,
        blocked_by,
        write_scopes,
    };
    state.tasks.push(task.clone());
    // next_task_number 单调推进（永不复用）。
    state.next_task_number += 1;
    Ok(task)
}

/// 读取单个任务（含 deleted tombstone）。
pub fn get_task(state: &TeamState, id: &str) -> Result<TeamTaskSnapshot, TeamError> {
    state
        .tasks
        .iter()
        .find(|t| t.id == id)
        .cloned()
        .ok_or_else(|| TeamError::TaskNotFound(id.to_string()))
}

/// 列出非 deleted 任务（按数字创建序）。
pub fn list_tasks(state: &TeamState) -> Vec<TeamTaskView> {
    state
        .tasks
        .iter()
        .filter(|t| t.status != TeamTaskStatus::Deleted)
        .map(|t| task_view::project_task_view(state, t))
        .collect()
}

/// 更新任务（CAS revision 校验）。
pub fn update_task(
    state: &mut TeamState,
    caller: &str,
    role: TeamRole,
    req: &UpdateTeamTaskRequest,
) -> Result<TeamTaskSnapshot, TeamError> {
    let action = req.action.ok_or_else(|| {
        TeamError::InvalidArgument("missing action".into())
    })?;
    let idx = state
        .tasks
        .iter()
        .position(|t| t.id == req.task_id)
        .ok_or_else(|| TeamError::TaskNotFound(req.task_id.clone()))?;
    let current = state.tasks[idx].clone();
    if current.revision != req.expected_revision {
        return Err(TeamError::TaskStaleRevision);
    }
    if current.status == TeamTaskStatus::Deleted {
        return Err(TeamError::TaskDeleted);
    }

    let is_lead = role == TeamRole::Lead;
    let is_owner = current.owner_id.as_deref() == Some(caller);
    // Claim 允许任意成员；其余动作需 owner 或 lead。
    if action != TeamTaskAction::Claim && !is_lead && !is_owner {
        return Err(TeamError::TaskUnauthorized);
    }

    let next = match action {
        TeamTaskAction::Claim => {
            if current.owner_id.is_some() && current.owner_id.as_deref() != Some(caller) {
                return Err(TeamError::TaskAlreadyClaimed);
            }
            if current.status != TeamTaskStatus::Pending {
                return Err(TeamError::TaskAlreadyClaimed);
            }
            if !task_view::task_ready(state, &current) {
                return Err(TeamError::TaskBlocked);
            }
            TeamTaskSnapshot {
                status: TeamTaskStatus::InProgress,
                owner_id: Some(caller.to_string()),
                ..current.clone()
            }
        }
        TeamTaskAction::Release => {
            if current.status != TeamTaskStatus::InProgress {
                return Err(TeamError::TaskInvalidTransition);
            }
            TeamTaskSnapshot {
                status: TeamTaskStatus::Pending,
                owner_id: None,
                ..current.clone()
            }
        }
        TeamTaskAction::Edit => {
            if req.subject.is_none() && req.description.is_none() && req.write_scopes.is_none() {
                return Err(TeamError::InvalidArgument("nothing to edit".into()));
            }
            let mut next = current.clone();
            if let Some(s) = &req.subject {
                next.subject = validation::required_text(s, 200)?;
            }
            if let Some(d) = &req.description {
                next.description = validation::required_text(d, 16_384)?;
            }
            if let Some(ws) = &req.write_scopes {
                next.write_scopes = validation::write_scopes(ws)?;
            }
            next
        }
        TeamTaskAction::SetDependencies => {
            let blocked_by = req.blocked_by.clone().ok_or_else(|| {
                TeamError::InvalidArgument("missing blockedBy".into())
            })?;
            let resolved = resolve_blockers(state, &blocked_by, &current.id)?;
            // 全图环检测：以当前任务为起点，沿 blockedBy 可达性是否回到自身。
            let successors = |id: &str| {
                if id == current.id {
                    resolved.clone()
                } else {
                    state
                        .tasks
                        .iter()
                        .find(|t| t.id == id)
                        .map(|t| t.blocked_by.clone())
                        .unwrap_or_default()
                }
            };
            if crate::agent_team::task_graph::GraphError::find_cycle(&current.id, &successors).is_some() {
                return Err(TeamError::TaskDependencyCycle);
            }
            TeamTaskSnapshot {
                blocked_by: resolved,
                ..current.clone()
            }
        }
        TeamTaskAction::Complete => {
            if current.status != TeamTaskStatus::InProgress {
                return Err(TeamError::TaskInvalidTransition);
            }
            TeamTaskSnapshot {
                status: TeamTaskStatus::Completed,
                ..current.clone()
            }
        }
        TeamTaskAction::Reopen => {
            if current.status != TeamTaskStatus::Completed {
                return Err(TeamError::TaskInvalidTransition);
            }
            TeamTaskSnapshot {
                status: TeamTaskStatus::Pending,
                owner_id: None,
                ..current.clone()
            }
        }
        TeamTaskAction::Reassign => {
            if !is_lead {
                return Err(TeamError::LeadRequired);
            }
            if current.status != TeamTaskStatus::Pending && current.status != TeamTaskStatus::InProgress {
                return Err(TeamError::TaskInvalidTransition);
            }
            match &req.owner {
                None => TeamTaskSnapshot {
                    status: TeamTaskStatus::Pending,
                    owner_id: None,
                    ..current.clone()
                },
                Some(assignee) => {
                    // assignee 按名字解析为活跃成员。
                    let member = state
                        .members
                        .iter()
                        .find(|m| m.name == *assignee)
                        .ok_or_else(|| TeamError::MemberNotFound(assignee.clone()))?;
                    if !task_view::task_ready(state, &current) {
                        return Err(TeamError::TaskBlocked);
                    }
                    TeamTaskSnapshot {
                        status: TeamTaskStatus::InProgress,
                        owner_id: Some(member.id.clone()),
                        ..current.clone()
                    }
                }
            }
        }
        TeamTaskAction::Delete => {
            // 若有非 deleted 任务 blockedBy 指向自己 → 有依赖者。
            let has_dependents = state.tasks.iter().any(|t| {
                t.status != TeamTaskStatus::Deleted && t.blocked_by.iter().any(|b| b == &current.id)
            });
            if has_dependents {
                return Err(TeamError::TaskHasDependents);
            }
            TeamTaskSnapshot {
                status: TeamTaskStatus::Deleted,
                ..current.clone()
            }
        }
    };

    let next = TeamTaskSnapshot {
        revision: current.revision + 1,
        ..next
    };
    state.tasks[idx] = next.clone();
    Ok(next)
}

/// 解析并校验 blockedBy（存在性 + 自指/重复校验）。
fn resolve_blockers(
    state: &TeamState,
    candidates: &[String],
    self_id: &str,
) -> Result<Vec<String>, TeamError> {
    let exists = |id: &str| state.tasks.iter().any(|t| t.id == id);
    GraphCheck::validate(candidates, self_id, exists).map(|c| c.blockers).map_err(map_graph_error)
}

/// 图错误 → TeamError 错误码映射。
fn map_graph_error(e: GraphError) -> TeamError {
    match e {
        GraphError::Missing(id) => TeamError::TaskNotFound(id),
        GraphError::Duplicate(_) => TeamError::InvalidArgument("duplicate blocker".into()),
        GraphError::Cycle(_) => TeamError::TaskDependencyCycle,
    }
}

#[cfg(test)]
#[path = "task_board_tests.rs"]
mod tests;
