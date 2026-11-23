//! `commands` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use super::{parse_plan_command, run_plan_command, PlanCommand};
use crate::plan::service::PlanMode;

#[test]
fn parse_bare_plan_enters() {
    assert_eq!(parse_plan_command("/plan"), Some(PlanCommand::Enter { message: None }));
    assert_eq!(parse_plan_command("/plan "), Some(PlanCommand::Enter { message: None }));
}

#[test]
fn parse_plan_with_message() {
    assert_eq!(
        parse_plan_command("/plan 设计三阶段方案"),
        Some(PlanCommand::Enter { message: Some("设计三阶段方案".to_string()) })
    );
}

#[test]
fn parse_plan_off_exits() {
    assert_eq!(parse_plan_command("/plan off"), Some(PlanCommand::Exit));
    assert_eq!(parse_plan_command("/plan  off"), Some(PlanCommand::Exit));
}

#[test]
fn parse_non_plan_returns_none() {
    assert_eq!(parse_plan_command("/goal"), None);
    assert_eq!(parse_plan_command("/planx"), None);
    assert_eq!(parse_plan_command("hello"), None);
}

#[test]
fn run_plan_command_writes_wanted_and_commits_when_idle() {
    use crate::session::SessionLog;
    let plan_mode = Arc::new(PlanMode::new());
    let session = Arc::new(SessionLog::in_memory());
    plan_mode.wire_session(session.clone());
    let busy = AtomicBool::new(false);
    run_plan_command(&plan_mode, &busy, PlanCommand::Enter { message: None });
    assert!(plan_mode.active());
    // active=true, wanted=true → 无 pending（已生效）。
    let projection = crate::plan::store::fold_plan(&session.snapshot());
    assert!(projection.active);
    assert!(projection.wanted);
    assert!(!projection.pending());
}

#[test]
fn run_plan_command_busy_queues_pending() {
    use crate::session::SessionLog;
    let plan_mode = Arc::new(PlanMode::new());
    let session = Arc::new(SessionLog::in_memory());
    plan_mode.wire_session(session.clone());
    let busy = AtomicBool::new(true);
    run_plan_command(&plan_mode, &busy, PlanCommand::Enter { message: None });
    // busy：active 仍 false，但 wanted=true → pending 可重放推导。
    assert!(!plan_mode.active());
    let projection = crate::plan::store::fold_plan(&session.snapshot());
    assert!(!projection.active);
    assert!(projection.wanted);
    assert!(projection.pending());
    // step 边界 flush 后生效。
    assert!(plan_mode.flush_pending());
    assert!(plan_mode.active());
}

#[test]
fn run_plan_command_off_exits_and_writes_wanted_false() {
    use crate::session::SessionLog;
    let plan_mode = Arc::new(PlanMode::new());
    let session = Arc::new(SessionLog::in_memory());
    plan_mode.wire_session(session.clone());
    let busy = AtomicBool::new(false);
    run_plan_command(&plan_mode, &busy, PlanCommand::Enter { message: None });
    run_plan_command(&plan_mode, &busy, PlanCommand::Exit);
    assert!(!plan_mode.active());
    let projection = crate::plan::store::fold_plan(&session.snapshot());
    assert!(!projection.active);
    assert!(!projection.wanted);
    assert!(!projection.pending());
}
