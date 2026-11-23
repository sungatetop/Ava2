//! `store` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::sync::Arc;

use crate::session::SessionLog;

use super::{append_plan, append_plan_run_done, append_plan_wanted, fold_plan, PlanProjection};

#[test]
fn fold_plan_active_last_wins() {
    let session = Arc::new(SessionLog::in_memory());
    let p = fold_plan(&session.snapshot());
    assert!(!p.active);
    append_plan(&session, true);
    assert!(fold_plan(&session.snapshot()).active);
    append_plan(&session, false);
    assert!(!fold_plan(&session.snapshot()).active);
    append_plan(&session, true);
    assert!(fold_plan(&session.snapshot()).active);
}

#[test]
fn pending_derived_from_wanted_not_active() {
    let session = Arc::new(SessionLog::in_memory());
    // 初始：active=false, wanted=false → 无 pending。
    let p = fold_plan(&session.snapshot());
    assert!(!p.pending());
    // 宿主想进入 plan（wanted=true），但 active 仍 false → pending。
    append_plan_wanted(&session, true);
    let p = fold_plan(&session.snapshot());
    assert!(p.pending());
    assert!(p.wanted);
    assert!(!p.active);
    // 边界追加生效（active=true），wanted=true → 无 pending。
    append_plan(&session, true);
    let p = fold_plan(&session.snapshot());
    assert!(!p.pending());
    // 宿主想退出（wanted=false），active 仍 true → pending。
    append_plan_wanted(&session, false);
    let p = fold_plan(&session.snapshot());
    assert!(p.pending());
    assert!(!p.wanted);
    assert!(p.active);
}

#[test]
fn fold_plan_run_done_lifecycle() {
    let session = Arc::new(SessionLog::in_memory());
    // 无 run 事件 → running=false。
    assert!(!fold_plan(&session.snapshot()).running);
    // 追加 run-done → running=false（运行态由 commands.rs 的 running 计数维护）。
    append_plan_run_done(&session);
    assert!(!fold_plan(&session.snapshot()).running);
}

#[test]
fn replay_recovers_four_tracks() {
    let session = Arc::new(SessionLog::in_memory());
    append_plan_wanted(&session, true);
    append_plan(&session, true);
    append_plan_run_done(&session);
    // 冷重放：从事件流恢复 active=wanted=true, running=false。
    let recovered = fold_plan(&session.snapshot());
    assert_eq!(
        recovered,
        PlanProjection { active: true, wanted: true, running: false }
    );
    assert!(!recovered.pending());
}
