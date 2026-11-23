//! `task_board` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use crate::agent_team::task_board::{create_task, get_task, list_tasks, update_task};
use crate::agent_team::types::{
    CreateTeamTaskRequest, TeamMemberPhase, TeamMemberSnapshot, TeamRole, TeamState, TeamTaskAction,
    TeamTaskStatus, UpdateTeamTaskRequest,
};
use crate::agent_team::{projection, TeamError};

fn empty_state() -> TeamState {
    projection::empty_team_state()
}

fn member(id: &str, name: &str) -> TeamMemberSnapshot {
    TeamMemberSnapshot {
        id: id.to_string(),
        name: name.to_string(),
        description: "".into(),
        provider: "spawn".into(),
        context: crate::agent_team::types::TeamContext::Fresh,
        phase: TeamMemberPhase::Active,
        error: None,
    }
}

fn create(state: &mut TeamState, id: &str, blocked: &[&str]) -> String {
    let req = CreateTeamTaskRequest {
        subject: id.into(),
        description: "".into(),
        blocked_by: blocked.iter().map(ToString::to_string).collect(),
        write_scopes: vec![],
    };
    create_task(state, &req, 256).unwrap().id
}

#[test]
fn create_assigns_incrementing_ids() {
    let mut state = empty_state();
    let id1 = create(&mut state, "a", &[]);
    let id2 = create(&mut state, "b", &[]);
    assert_eq!(id1, "task-1");
    assert_eq!(id2, "task-2");
}

#[test]
fn create_enforces_max_tasks() {
    let mut state = empty_state();
    for i in 0..2 {
        let req = CreateTeamTaskRequest {
            subject: format!("t{i}"),
            description: "".into(),
            blocked_by: vec![],
            write_scopes: vec![],
        };
        create_task(&mut state, &req, 2).unwrap();
    }
    let req = CreateTeamTaskRequest {
        subject: "over".into(),
        description: "".into(),
        blocked_by: vec![],
        write_scopes: vec![],
    };
    assert!(matches!(create_task(&mut state, &req, 2), Err(TeamError::TaskLimit)));
}

#[test]
fn create_requires_subject() {
    let mut state = empty_state();
    let req = CreateTeamTaskRequest {
        subject: "   ".into(),
        description: "".into(),
        blocked_by: vec![],
        write_scopes: vec![],
    };
    assert!(matches!(create_task(&mut state, &req, 256), Err(TeamError::InvalidArgument(_))));
}

#[test]
fn claim_requires_ready_and_pending() {
    let mut state = empty_state();
    let id = create(&mut state, "a", &[]);
    let req = UpdateTeamTaskRequest {
        task_id: id,
        expected_revision: 1,
        action: Some(TeamTaskAction::Claim),
        ..Default::default()
    };
    let t = update_task(&mut state, "m1", TeamRole::Teammate, &req).unwrap();
    assert_eq!(t.status, TeamTaskStatus::InProgress);
    assert_eq!(t.owner_id.as_deref(), Some("m1"));
    assert_eq!(t.revision, 2);
}

#[test]
fn claim_blocked_when_blocker_not_completed() {
    let mut state = empty_state();
    create(&mut state, "dep", &[]);
    let id = create(&mut state, "a", &["task-1"]);
    let req = UpdateTeamTaskRequest {
        task_id: id,
        expected_revision: 1,
        action: Some(TeamTaskAction::Claim),
        ..Default::default()
    };
    assert!(matches!(update_task(&mut state, "m1", TeamRole::Teammate, &req), Err(TeamError::TaskBlocked)));
}

#[test]
fn claim_rejects_second_owner() {
    let mut state = empty_state();
    let id = create(&mut state, "a", &[]);
    let claim = UpdateTeamTaskRequest {
        task_id: id.clone(),
        expected_revision: 1,
        action: Some(TeamTaskAction::Claim),
        ..Default::default()
    };
    update_task(&mut state, "m1", TeamRole::Teammate, &claim).unwrap();
    let again = UpdateTeamTaskRequest {
        task_id: id,
        expected_revision: 2,
        action: Some(TeamTaskAction::Claim),
        ..Default::default()
    };
    assert!(matches!(update_task(&mut state, "m2", TeamRole::Teammate, &again), Err(TeamError::TaskAlreadyClaimed)));
}

#[test]
fn stale_revision_rejected() {
    let mut state = empty_state();
    let id = create(&mut state, "a", &[]);
    let req = UpdateTeamTaskRequest {
        task_id: id,
        expected_revision: 99,
        action: Some(TeamTaskAction::Complete),
        ..Default::default()
    };
    assert!(matches!(update_task(&mut state, "m1", TeamRole::Teammate, &req), Err(TeamError::TaskStaleRevision)));
}

#[test]
fn complete_requires_in_progress() {
    let mut state = empty_state();
    let id = create(&mut state, "a", &[]);
    // 非 owner 的 teammate 对 pending 任务 complete → 未授权（授权先于状态检查）。
    let complete = UpdateTeamTaskRequest {
        task_id: id.clone(),
        expected_revision: 1,
        action: Some(TeamTaskAction::Complete),
        ..Default::default()
    };
    assert!(matches!(update_task(&mut state, "m1", TeamRole::Teammate, &complete), Err(TeamError::TaskUnauthorized)));
    // 先 claim（m1 成为 owner），再 complete → 成功。
    let claim = UpdateTeamTaskRequest {
        task_id: id.clone(),
        expected_revision: 1,
        action: Some(TeamTaskAction::Claim),
        ..Default::default()
    };
    update_task(&mut state, "m1", TeamRole::Teammate, &claim).unwrap();
    let complete2 = UpdateTeamTaskRequest {
        task_id: id,
        expected_revision: 2,
        action: Some(TeamTaskAction::Complete),
        ..Default::default()
    };
    let t = update_task(&mut state, "m1", TeamRole::Teammate, &complete2).unwrap();
    assert_eq!(t.status, TeamTaskStatus::Completed);
}

#[test]
fn unauthorized_member_cannot_edit() {
    let mut state = empty_state();
    let id = create(&mut state, "a", &[]);
    let req = UpdateTeamTaskRequest {
        task_id: id,
        expected_revision: 1,
        action: Some(TeamTaskAction::Edit),
        subject: Some("hacked".into()),
        ..Default::default()
    };
    assert!(matches!(update_task(&mut state, "m2", TeamRole::Teammate, &req), Err(TeamError::TaskUnauthorized)));
}

#[test]
fn reassign_requires_lead_and_resolves_member() {
    let mut state = empty_state();
    state.members.push(member("m1", "alice"));
    state.members.push(member("m2", "bob"));
    let id = create(&mut state, "a", &[]);
    // 让 m1 成为 owner（claim），但 m1 非 lead → reassign 应报 LeadRequired。
    let claim = UpdateTeamTaskRequest {
        task_id: id.clone(),
        expected_revision: 1,
        action: Some(TeamTaskAction::Claim),
        ..Default::default()
    };
    update_task(&mut state, "m1", TeamRole::Teammate, &claim).unwrap();
    let reassign = UpdateTeamTaskRequest {
        task_id: id,
        expected_revision: 2,
        action: Some(TeamTaskAction::Reassign),
        owner: Some("bob".into()),
        ..Default::default()
    };
    assert!(matches!(update_task(&mut state, "m1", TeamRole::Teammate, &reassign), Err(TeamError::LeadRequired)));
    // lead 指派给 bob。
    let t = update_task(&mut state, "lead", TeamRole::Lead, &reassign).unwrap();
    assert_eq!(t.status, TeamTaskStatus::InProgress);
    assert_eq!(t.owner_id.as_deref(), Some("m2"));
}

#[test]
fn delete_rejects_when_dependents_exist() {
    let mut state = empty_state();
    let dep = create(&mut state, "dep", &[]);
    create(&mut state, "a", &["task-1"]);
    let del = UpdateTeamTaskRequest {
        task_id: dep,
        expected_revision: 1,
        action: Some(TeamTaskAction::Delete),
        ..Default::default()
    };
    assert!(matches!(update_task(&mut state, "lead", TeamRole::Lead, &del), Err(TeamError::TaskHasDependents)));
}

#[test]
fn dependency_cycle_rejected() {
    let mut state = empty_state();
    let a = create(&mut state, "a", &[]);
    let b = create(&mut state, "b", &[]);
    // a -> b。
    let set_dep = UpdateTeamTaskRequest {
        task_id: a.clone(),
        expected_revision: 1,
        action: Some(TeamTaskAction::SetDependencies),
        blocked_by: Some(vec![b.clone()]),
        ..Default::default()
    };
    update_task(&mut state, "lead", TeamRole::Lead, &set_dep).unwrap();
    // b -> a 成环。
    let cycle = UpdateTeamTaskRequest {
        task_id: b,
        expected_revision: 1,
        action: Some(TeamTaskAction::SetDependencies),
        blocked_by: Some(vec![a]),
        ..Default::default()
    };
    assert!(matches!(update_task(&mut state, "lead", TeamRole::Lead, &cycle), Err(TeamError::TaskDependencyCycle)));
}

#[test]
fn list_filters_deleted() {
    let mut state = empty_state();
    let id = create(&mut state, "a", &[]);
    let del = UpdateTeamTaskRequest {
        task_id: id,
        expected_revision: 1,
        action: Some(TeamTaskAction::Delete),
        ..Default::default()
    };
    update_task(&mut state, "lead", TeamRole::Lead, &del).unwrap();
    assert!(list_tasks(&state).is_empty());
}

#[test]
fn get_returns_tombstone() {
    let mut state = empty_state();
    let id = create(&mut state, "a", &[]);
    let del = UpdateTeamTaskRequest {
        task_id: id.clone(),
        expected_revision: 1,
        action: Some(TeamTaskAction::Delete),
        ..Default::default()
    };
    update_task(&mut state, "lead", TeamRole::Lead, &del).unwrap();
    let t = get_task(&state, &id).unwrap();
    assert_eq!(t.status, TeamTaskStatus::Deleted);
}
