//! 纯任务视图派生。
//!
//! readiness、owner-name、write-scope 重叠告警——由任务板与客户端视图共享。

use crate::agent_team::types::{TeamState, TeamTaskSnapshot, TeamTaskStatus, TeamTaskView};

/// write-scope 是否重叠（前缀路径包含也算重叠）。
pub fn scopes_overlap(left: &str, right: &str) -> bool {
    left == right || left.starts_with(&format!("{right}/")) || right.starts_with(&format!("{left}/"))
}

/// 任务是否就绪（blocked_by 全部 completed）。
pub fn task_ready(state: &TeamState, task: &TeamTaskSnapshot) -> bool {
    task.blocked_by.iter().all(|id| {
        state
            .tasks
            .iter()
            .find(|t| t.id == *id)
            .is_some_and(|t| t.status == TeamTaskStatus::Completed)
    })
}

/// 从成员快照解析 owner 名。
pub fn owner_name(state: &TeamState, owner_id: Option<&str>) -> Option<String> {
    owner_id.and_then(|id| state.members.iter().find(|m| m.id == *id).map(|m| m.name.clone()))
}

/// 投影一个任务视图。
pub fn project_task_view(state: &TeamState, task: &TeamTaskSnapshot) -> TeamTaskView {
    let mut warnings = Vec::new();
    // 对每个 in_progress 的其他任务，若任一 writeScope 重叠则告警（advisory，不阻断）。
    for other in state.tasks.iter().filter(|t| t.id != task.id) {
        if other.status != TeamTaskStatus::InProgress {
            continue;
        }
        let overlap = other
            .write_scopes
            .iter()
            .any(|other_scope| task.write_scopes.iter().any(|scope| scopes_overlap(scope, other_scope)));
        if overlap {
            warnings.push(format!("write scopes overlap with {}", other.id));
        }
    }
    let ready = task.status == TeamTaskStatus::Pending && task_ready(state, task);
    TeamTaskView {
        id: task.id.clone(),
        revision: task.revision,
        subject: task.subject.clone(),
        description: task.description.clone(),
        status: task.status,
        owner_name: owner_name(state, task.owner_id.as_deref()),
        blocked_by: task.blocked_by.clone(),
        write_scopes: task.write_scopes.clone(),
        ready,
        write_scope_warnings: warnings,
    }
}

#[cfg(test)]
#[path = "task_view_tests.rs"]
mod tests;
