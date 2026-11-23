//! `state` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use crate::goal::types::{GoalActivation, GoalBlockReason, GoalPhase};
use super::GoalState;

fn now() -> u64 {
    1_000
}

#[test]
fn create_initial_active_armed() {
    let mut state = GoalState::default();
    let snapshot = state.create("fix bug", 5, now()).unwrap();
    assert_eq!(snapshot.phase, GoalPhase::Active);
    assert_eq!(snapshot.revision, 1);
    assert_eq!(state.activation, GoalActivation::Armed);
    assert_eq!(state.rounds_started, 0);
}

#[test]
fn create_rejects_existing_non_complete() {
    let mut state = GoalState::default();
    state.create("first", 5, now()).unwrap();
    let err = state.create("second", 5, now()).unwrap_err();
    assert!(matches!(err, super::GoalError::AlreadyExists(GoalPhase::Active)));
}

#[test]
fn create_replaces_complete() {
    let mut state = GoalState::default();
    let first = state.create("first", 5, now()).unwrap();
    state.complete(first.ref_of(), now()).unwrap();
    state.disarm();
    let second = state.create("second", 5, now() + 1).unwrap();
    assert_eq!(second.objective, "second");
    assert_eq!(state.activation, GoalActivation::Armed);
}

#[test]
fn pause_only_active() {
    let mut state = GoalState::default();
    state.create("g", 5, now()).unwrap();
    let r = state.current_ref().unwrap();
    let snap = state.pause(r, now() + 1).unwrap();
    assert_eq!(snap.phase, GoalPhase::Paused);
    assert_eq!(state.activation, GoalActivation::Disarmed);
    // pause paused rejected
    let r = state.current_ref().unwrap();
    assert!(matches!(
        state.pause(r, now() + 2),
        Err(super::GoalError::InvalidTransition { .. })
    ));
}

#[test]
fn resume_from_paused_and_blocked() {
    let mut state = GoalState::default();
    state.create("g", 5, now()).unwrap();
    let r = state.current_ref().unwrap();
    state.pause(r, now() + 1).unwrap();
    let r = state.current_ref().unwrap();
    let snap = state.resume(r, now() + 2).unwrap();
    assert_eq!(snap.phase, GoalPhase::Active);
    assert_eq!(state.activation, GoalActivation::Armed);
}

#[test]
fn resume_rejects_active_armed() {
    let mut state = GoalState::default();
    state.create("g", 5, now()).unwrap();
    let r = state.current_ref().unwrap();
    assert!(matches!(
        state.resume(r, now() + 1),
        Err(super::GoalError::InvalidTransition { .. })
    ));
}

#[test]
fn resume_rejects_exhausted_budget() {
    let mut state = GoalState::default();
    state.create("g", 2, now()).unwrap();
    let r = state.current_ref().unwrap();
    state.pause(r, now() + 1).unwrap();
    // 手动推进到预算耗尽
    let goal_id = state.snapshot.as_ref().unwrap().id.clone();
    state.rounds_started = 2;
    let r = state.current_ref().unwrap();
    let err = state.resume(r, now() + 2).unwrap_err();
    assert!(matches!(err, super::GoalError::RoundBudgetExhausted { .. }));
    assert_eq!(state.snapshot.as_ref().unwrap().id, goal_id);
}

#[test]
fn complete_from_any_non_complete() {
    let mut state = GoalState::default();
    state.create("g", 5, now()).unwrap();
    let r = state.current_ref().unwrap();
    let snap = state.complete(r, now() + 1).unwrap();
    assert_eq!(snap.phase, GoalPhase::Complete);
    assert_eq!(state.activation, GoalActivation::Disarmed);
}

#[test]
fn block_sets_reason() {
    let mut state = GoalState::default();
    state.create("g", 5, now()).unwrap();
    let r = state.current_ref().unwrap();
    let snap =
        state.block(r, GoalBlockReason::new("round-limit", "budget exhausted"), now() + 1).unwrap();
    assert_eq!(snap.phase, GoalPhase::Blocked);
    assert_eq!(snap.blocked_reason.as_ref().unwrap().code, "round-limit");
    assert_eq!(state.activation, GoalActivation::Disarmed);
}

#[test]
fn revision_mismatch_rejected() {
    let mut state = GoalState::default();
    state.create("g", 5, now()).unwrap();
    let bad = super::GoalRef { id: state.snapshot.as_ref().unwrap().id.clone(), revision: 99 };
    assert!(matches!(
        state.pause(bad, now() + 1),
        Err(super::GoalError::RevisionMismatch { .. })
    ));
}

#[test]
fn clear_removes_snapshot() {
    let mut state = GoalState::default();
    state.create("g", 5, now()).unwrap();
    let r = state.current_ref().unwrap();
    state.clear(r).unwrap();
    assert!(state.snapshot.is_none());
    assert_eq!(state.activation, GoalActivation::Disarmed);
}

#[test]
fn advance_round_sequential_only() {
    let mut state = GoalState::default();
    state.create("g", 3, now()).unwrap();
    let r = state.current_ref().unwrap();
    // 非法：跳轮
    assert!(matches!(
        state.advance_round(&r.id, r.revision, 2),
        Err(super::GoalError::RoundNotAdmitted { .. })
    ));
    // 合法：恰好下一轮
    state.advance_round(&r.id, r.revision, 1).unwrap();
    assert_eq!(state.rounds_started, 1);
    // 预算耗尽
    let r = state.current_ref().unwrap();
    assert!(matches!(
        state.advance_round(&r.id, r.revision, 99),
        Err(super::GoalError::RoundBudgetExhausted { .. })
    ));
}

#[test]
fn replay_restores_durable_phase_with_disarmed() {
    use crate::session::SessionEvent;
    let mut state = GoalState::default();
    let snap = state.create("g", 5, now()).unwrap();
    let events = vec![SessionEvent::GoalChange {
        operation: "create".to_string(),
        snapshot: Some(snap.clone()),
        rounds_started: 0,
        created_at_ms: now(),
        updated_at_ms: now(),
    }];
    let replayed = super::replay_goal(&events, GoalActivation::Disarmed).unwrap();
    assert_eq!(replayed.snapshot.as_ref().unwrap(), &snap);
    assert_eq!(replayed.activation, GoalActivation::Disarmed);
    assert_eq!(replayed.rounds_started, 0);
}

#[test]
fn replay_rejects_goal_id_reuse() {
    use crate::session::SessionEvent;
    use crate::goal::types::GoalSnapshot;
    let snap_a = GoalSnapshot {
        id: "goal-1".to_string(),
        revision: 1,
        objective: "a".to_string(),
        phase: GoalPhase::Active,
        blocked_reason: None,
        max_goal_rounds: 5,
    };
    let snap_b = GoalSnapshot {
        id: "goal-2".to_string(),
        revision: 1,
        objective: "b".to_string(),
        phase: GoalPhase::Active,
        blocked_reason: None,
        max_goal_rounds: 5,
    };
    // 合法：goal-1 → clear → goal-2（id 不复用）。
    let ok_events = vec![
        SessionEvent::GoalChange {
            operation: "create".into(), snapshot: Some(snap_a.clone()),
            rounds_started: 0, created_at_ms: 1, updated_at_ms: 1,
        },
        SessionEvent::GoalChange {
            operation: "clear".into(), snapshot: None,
            rounds_started: 0, created_at_ms: 1, updated_at_ms: 2,
        },
        SessionEvent::GoalChange {
            operation: "create".into(),
            snapshot: Some(snap_b),
            rounds_started: 0,
            created_at_ms: 2,
            updated_at_ms: 2,
        },
    ];
    assert!(super::replay_goal(&ok_events, GoalActivation::Disarmed).is_ok());
    // 非法：goal-1 被 clear 后 id 复用再次出现。
    let bad_events = vec![
        SessionEvent::GoalChange {
            operation: "create".into(), snapshot: Some(snap_a.clone()),
            rounds_started: 0, created_at_ms: 1, updated_at_ms: 1,
        },
        SessionEvent::GoalChange {
            operation: "clear".into(), snapshot: None,
            rounds_started: 0, created_at_ms: 1, updated_at_ms: 2,
        },
        SessionEvent::GoalChange {
            operation: "create".into(), snapshot: Some(snap_a),
            rounds_started: 0, created_at_ms: 2, updated_at_ms: 2,
        },
    ];
    let err = super::replay_goal(&bad_events, GoalActivation::Disarmed).unwrap_err();
    assert!(matches!(err, super::GoalError::Replay { .. }));
}

#[test]
fn replay_fails_loud_on_invalid_fields() {
    use crate::session::SessionEvent;
    use crate::goal::types::GoalSnapshot;
    let snap = GoalSnapshot {
        id: "goal-1".to_string(),
        revision: 1,
        objective: "g".to_string(),
        phase: GoalPhase::Active,
        blocked_reason: None,
        max_goal_rounds: 5,
    };
    // updated_at < created_at → fail-loud。
    let bad_time = vec![SessionEvent::GoalChange {
        operation: "create".into(),
        snapshot: Some(snap.clone()),
        rounds_started: 0,
        created_at_ms: 100,
        updated_at_ms: 50,
    }];
    assert!(matches!(
        super::replay_goal(&bad_time, GoalActivation::Disarmed).unwrap_err(),
        super::GoalError::Replay { .. }
    ));
    // rounds_started 回退 → fail-loud。
    let bad_rounds = vec![
        SessionEvent::GoalChange {
            operation: "round".into(),
            snapshot: Some(snap.clone()),
            rounds_started: 3,
            created_at_ms: 1,
            updated_at_ms: 1,
        },
        SessionEvent::GoalChange {
            operation: "round".into(),
            snapshot: Some(snap),
            rounds_started: 1,
            created_at_ms: 1,
            updated_at_ms: 1,
        },
    ];
    assert!(matches!(
        super::replay_goal(&bad_rounds, GoalActivation::Disarmed).unwrap_err(),
        super::GoalError::Replay { .. }
    ));
}
