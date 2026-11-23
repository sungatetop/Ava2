//! `state` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;
use crate::session::EventBus;

#[test]
fn legal_full_chain() {
    let mut machine = Machine::new(EventBus::new());
    assert_eq!(machine.state(), SessionState::Idle);
    machine.start_run().unwrap();
    machine.claimed().unwrap();
    machine.enter_step().unwrap();
    machine.enter_requesting().unwrap();
    machine.enter_streaming().unwrap();
    machine.enter_settling().unwrap();
    machine.step_continues().unwrap();
    machine.enter_step().unwrap();
    machine.enter_requesting().unwrap();
    machine.enter_streaming().unwrap();
    machine.enter_executing().unwrap();
    machine.enter_settling().unwrap();
    machine.enter_stopping().unwrap();
    machine.stop_continue().unwrap();
    machine.enter_step().unwrap();
    machine.enter_settling().unwrap(); // 幂等：Settling -> Settling
    machine.enter_stopping().unwrap();
    machine.stop_end().unwrap();
    machine.turn_closed().unwrap();
    assert_eq!(machine.state(), SessionState::Idle);
    machine.end().unwrap();
    assert_eq!(machine.state(), SessionState::Ended);
}

#[test]
fn legal_reject_and_abort_paths() {
    let mut machine = Machine::new(EventBus::new());
    machine.start_run().unwrap();
    machine.no_input().unwrap();
    machine.turn_closed().unwrap();
    assert_eq!(machine.state(), SessionState::Idle);

    machine.start_run().unwrap();
    machine.claimed().unwrap();
    machine.reject_step().unwrap();
    machine.turn_closed().unwrap();

    machine.start_run().unwrap();
    machine.claimed().unwrap();
    machine.enter_step().unwrap();
    machine.abort_step().unwrap();
    machine.turn_closed().unwrap();

    machine.start_run().unwrap();
    machine.abort_step().unwrap(); // Claiming 也可中止
    machine.turn_closed().unwrap();
    assert_eq!(machine.state(), SessionState::Idle);
}

#[test]
fn illegal_transitions_rejected() {
    let mut machine = Machine::new(EventBus::new());
    // Idle 不能直接进入 Step 子态
    assert!(machine.enter_streaming().is_err());
    assert!(machine.turn_closed().is_err());
    machine.start_run().unwrap();
    // Claiming 不能跳 Step 层
    assert!(machine.enter_streaming().is_err());
    assert!(machine.enter_step().is_err()); // 未 claimed
    machine.claimed().unwrap();
    // Admitting 不能到 Settling
    assert!(machine.enter_settling().is_err());
    machine.enter_step().unwrap();
    // Preparing 不能直接到 Streaming
    assert!(machine.enter_streaming().is_err());
    // Closing 不能回 Admitting
    machine.abort_step().unwrap();
    assert!(machine.claimed().is_err());
    assert!(machine.stop_end().is_err());
}

#[test]
fn state_changed_broadcast_on_bus() {
    let bus = EventBus::new();
    let mut rx = bus.subscribe();
    let mut machine = Machine::new(bus);
    machine.start_run().unwrap();
    let event = rx.try_recv().expect("start_run 应发 StateChanged");
    match event {
        LiveEvent::StateChanged { from, to } => {
            assert_eq!(from, "Idle");
            assert_eq!(to, "Running(Claiming)");
        }
        other => panic!("期望 StateChanged，实际 {other:?}"),
    }
    // Preparing -> Settling 是合法转移（pre-cancel 安全点直接收尾），发事件
    machine.claimed().unwrap();
    machine.enter_step().unwrap();
    machine.enter_settling().unwrap();
    let mut seen = Vec::new();
    while let Ok(event) = rx.try_recv() {
        seen.push(event);
    }
    assert_eq!(seen.len(), 3, "start_run/claimed/enter_step/enter_settling 各一次");
}
