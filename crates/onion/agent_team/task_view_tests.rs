//! `task_view` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use crate::agent_team::task_view::{owner_name, project_task_view, scopes_overlap, task_ready};
use crate::agent_team::types::{
    TeamMemberPhase, TeamMemberSnapshot, TeamState, TeamTaskSnapshot, TeamTaskStatus, TeamTaskView,
};

fn task(id: &str, status: TeamTaskStatus, blocked_by: Vec<&str>, scopes: Vec<&str>) -> TeamTaskSnapshot {
    TeamTaskSnapshot {
        id: id.to_string(),
        revision: 1,
        subject: id.to_string(),
        description: "".into(),
        status,
        owner_id: None,
        blocked_by: blocked_by.into_iter().map(str::to_string).collect(),
        write_scopes: scopes.into_iter().map(str::to_string).collect(),
    }
}

#[test]
fn scopes_overlap_matches_prefix() {
    assert!(scopes_overlap("src", "src"));
    assert!(scopes_overlap("src/lib", "src"));
    assert!(scopes_overlap("src", "src/lib"));
    assert!(!scopes_overlap("src", "test"));
}

#[test]
fn task_ready_requires_all_blockers_completed() {
    let state = TeamState {
        tasks: vec![
            task("task-1", TeamTaskStatus::Completed, vec![], vec![]),
            task("task-2", TeamTaskStatus::Pending, vec![], vec![]),
        ],
        ..Default::default()
    };
    // task-1 已完成 → 依赖它的任务就绪。
    let dep = task("task-3", TeamTaskStatus::Pending, vec!["task-1"], vec![]);
    assert!(task_ready(&state, &dep));
    // 依赖未完成 task-2 → 未就绪。
    let dep2 = task("task-4", TeamTaskStatus::Pending, vec!["task-2"], vec![]);
    assert!(!task_ready(&state, &dep2));
}

#[test]
fn owner_name_resolves_member() {
    let state = TeamState {
        members: vec![TeamMemberSnapshot {
            id: "m1".into(),
            name: "reviewer".into(),
            description: "".into(),
            provider: "spawn".into(),
            context: crate::agent_team::types::TeamContext::Fresh,
            phase: TeamMemberPhase::Active,
            error: None,
        }],
        ..Default::default()
    };
    assert_eq!(owner_name(&state, Some("m1")).as_deref(), Some("reviewer"));
    assert_eq!(owner_name(&state, Some("m2")), None);
    assert_eq!(owner_name(&state, None), None);
}

#[test]
fn project_task_view_reports_overlap_warnings() {
    let state = TeamState {
        tasks: vec![
            task("task-1", TeamTaskStatus::InProgress, vec![], vec!["src"]),
            task("task-2", TeamTaskStatus::Pending, vec![], vec!["src/lib"]),
        ],
        ..Default::default()
    };
    let view: TeamTaskView = project_task_view(&state, &state.tasks[1]);
    assert!(view.write_scope_warnings.contains(&"write scopes overlap with task-1".to_string()));
    // ready = pending && 无 blocker。
    assert!(view.ready);
}
