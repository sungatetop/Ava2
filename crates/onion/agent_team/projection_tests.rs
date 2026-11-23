//! `projection` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use crate::agent_team::projection::{apply_event, empty_team_state, ProjectionError, TeamEvent};
use crate::agent_team::types::{
    TeamContext, TeamMemberPhase, TeamMemberSnapshot, TeamTaskSnapshot, TeamTaskStatus,
};

fn member(id: &str, name: &str, phase: TeamMemberPhase) -> TeamMemberSnapshot {
    TeamMemberSnapshot {
        id: id.to_string(),
        name: name.to_string(),
        description: "".into(),
        provider: "spawn".into(),
        context: TeamContext::Fresh,
        phase,
        error: None,
    }
}

fn task(id: &str, revision: u64) -> TeamTaskSnapshot {
    TeamTaskSnapshot {
        id: id.to_string(),
        revision,
        subject: id.to_string(),
        description: "".into(),
        status: TeamTaskStatus::Pending,
        owner_id: None,
        blocked_by: vec![],
        write_scopes: vec![],
    }
}

#[test]
fn empty_state_starts_next_task_at_one() {
    let state = empty_team_state();
    assert_eq!(state.next_task_number, 1);
    assert!(state.members.is_empty());
    assert!(state.tasks.is_empty());
}

#[test]
fn member_must_start_provisioning() {
    let mut state = empty_team_state();
    let err = apply_event(&mut state, &TeamEvent::Member(member("m1", "alice", TeamMemberPhase::Active)))
        .unwrap_err();
    assert!(matches!(err, ProjectionError::MemberNotProvisioning(_)));
}

#[test]
fn member_provisioning_then_active() {
    let mut state = empty_team_state();
    apply_event(&mut state, &TeamEvent::Member(member("m1", "alice", TeamMemberPhase::Provisioning))).unwrap();
    apply_event(&mut state, &TeamEvent::Member(member("m1", "alice", TeamMemberPhase::Active))).unwrap();
    assert_eq!(state.members.len(), 1);
    assert_eq!(state.members[0].phase, TeamMemberPhase::Active);
}

#[test]
fn member_name_not_reused_by_other() {
    let mut state = empty_team_state();
    apply_event(&mut state, &TeamEvent::Member(member("m1", "alice", TeamMemberPhase::Provisioning))).unwrap();
    let err = apply_event(&mut state, &TeamEvent::Member(member("m2", "alice", TeamMemberPhase::Provisioning)))
        .unwrap_err();
    assert!(matches!(err, ProjectionError::NameReused(_)));
}

#[test]
fn member_immutable_fields_are_checked() {
    let mut state = empty_team_state();
    apply_event(&mut state, &TeamEvent::Member(member("m1", "alice", TeamMemberPhase::Provisioning))).unwrap();
    let mut changed = member("m1", "bob", TeamMemberPhase::Provisioning);
    changed.context = TeamContext::Fork;
    let err = apply_event(&mut state, &TeamEvent::Member(changed)).unwrap_err();
    assert!(matches!(err, ProjectionError::ImmutableFieldChanged(_)));
}

#[test]
fn task_revision_must_be_continuous() {
    let mut state = empty_team_state();
    apply_event(&mut state, &TeamEvent::Task(task("task-1", 1))).unwrap();
    let err = apply_event(&mut state, &TeamEvent::Task(task("task-1", 3))).unwrap_err();
    assert!(matches!(err, ProjectionError::RevisionGap(_)));
}

#[test]
fn new_task_must_be_revision_one() {
    let mut state = empty_team_state();
    let err = apply_event(&mut state, &TeamEvent::Task(task("task-1", 2))).unwrap_err();
    assert!(matches!(err, ProjectionError::RevisionGap(_)));
}

#[test]
fn next_task_number_advances() {
    let mut state = empty_team_state();
    apply_event(&mut state, &TeamEvent::Task(task("task-1", 1))).unwrap();
    assert_eq!(state.next_task_number, 2);
    apply_event(&mut state, &TeamEvent::Task(task("task-2", 1))).unwrap();
    assert_eq!(state.next_task_number, 3);
}
