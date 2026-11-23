//! `driver` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//!
//! 覆盖 D1「子代理互斥显式化」：`GoalRoundDriver::should_continue()` 在 subagent
//! 在飞（inflight > 0）时拒绝自动续轮，settle 后恢复允许。判据经 `wire_subagents`
//! 注入可控谓词观测，无需真实 subagent 运行时。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::inbox::Inbox;
use crate::session::{EventBus, SessionLog};

use super::GoalRoundDriver;
use crate::goal::service::GoalService;

/// 构造一个已接线、已创建 active goal 的 driver（默认无 subagent 判据）。
fn wired_driver() -> (Arc<GoalRoundDriver>, Arc<SessionLog>) {
    let session = Arc::new(SessionLog::in_memory());
    let service = Arc::new(GoalService::new());
    service.wire_session(session.clone());
    service.create("d1 goal", 5).unwrap();
    let inbox = Arc::new(Inbox::new(EventBus::new()));
    let driver = Arc::new(GoalRoundDriver::new(service, inbox, session.clone()));
    (driver, session)
}

#[test]
fn should_continue_true_without_subagent_predicate() {
    let (driver, _session) = wired_driver();
    // 未接线判据 → 视为无 subagent 在飞 → 允许续轮
    assert!(driver.should_continue());
}

#[test]
fn should_continue_gates_on_subagent_inflight() {
    let (driver, _session) = wired_driver();
    let inflight = Arc::new(AtomicBool::new(false));
    let predicate: Arc<dyn Fn() -> bool + Send + Sync> = {
        let inflight = inflight.clone();
        Arc::new(move || inflight.load(Ordering::SeqCst))
    };
    driver.wire_subagents(predicate);

    // subagent 不在飞 → 允许续轮
    assert!(driver.should_continue());

    // subagent 在飞 → 拒绝自动续轮（互斥）
    inflight.store(true, Ordering::SeqCst);
    assert!(!driver.should_continue());

    // settle 后 → 恢复允许
    inflight.store(false, Ordering::SeqCst);
    assert!(driver.should_continue());
}

#[test]
fn should_continue_gates_on_inbox_pending_even_when_no_subagent() {
    let (driver, _session) = wired_driver();
    // 有排队人类消息 → 不自动续轮
    driver
        .inbox
        .followup(crate::types::AgentMessage::user("排队输入"))
        .unwrap();
    assert!(!driver.should_continue());
}

#[test]
fn budget_exhaustion_blocks_with_round_limit() {
    let session = Arc::new(SessionLog::in_memory());
    let service = Arc::new(GoalService::new());
    service.wire_session(session.clone());
    service.create("budget goal", 1).unwrap();
    let inbox = Arc::new(Inbox::new(EventBus::new()));
    let driver = Arc::new(GoalRoundDriver::new(service.clone(), inbox, session));

    // 预算未耗尽：可续轮
    assert!(driver.should_continue());
    assert!(driver.continuation_input().is_some());
    driver.record_round().unwrap();

    // 预算耗尽：should_continue 仍为 true（drivable 不含预算），
    // continuation_input 返回 None 并落 round-limit block。
    assert!(driver.should_continue());
    assert!(driver.continuation_input().is_none());
    let view = service.view().unwrap();
    assert_eq!(view.snapshot.phase, crate::goal::types::GoalPhase::Blocked);
    assert_eq!(view.snapshot.blocked_reason.unwrap().code, "round-limit");
    // block 后 disarm → 不再续轮
    assert!(!driver.should_continue());
}

#[test]
fn disarm_stops_auto_continuation() {
    let (driver, _session) = wired_driver();
    // 默认 armed + active → 允许续轮
    assert!(driver.should_continue());
    // D2：轮因 max-tokens 结束 → disarm → 拒绝自动续轮（防无限续轮）
    driver.disarm();
    assert!(!driver.should_continue());
    // 重新 armed（resume）后恢复
    let service = driver.service.clone();
    let r = service.view().unwrap().snapshot.ref_of();
    service.resume(r).unwrap();
    assert!(driver.should_continue());
}
