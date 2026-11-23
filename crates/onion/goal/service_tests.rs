//! `service` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::sync::Arc;

use crate::session::SessionLog;

use crate::goal::types::{GoalActivation, GoalPhase};
use super::{Authority, GoalService};

fn wire(service: &GoalService) {
    let session = Arc::new(SessionLog::in_memory());
    service.wire_session(session);
}

#[test]
fn create_appends_durable_event_and_arms() {
    let service = GoalService::new();
    wire(&service);
    let snapshot = service.create("fix bug", 5).unwrap();
    assert_eq!(snapshot.phase, GoalPhase::Active);
    assert_eq!(service.activation(), GoalActivation::Armed);
    assert!(service.should_continue());
}

#[test]
fn should_continue_gates_on_phase_and_activation() {
    let service = GoalService::new();
    wire(&service);
    service.create("g", 5).unwrap();
    assert!(service.should_continue());
    // pause → disarm → 不续
    let r = service.view().unwrap().snapshot.ref_of();
    service.pause(r).unwrap();
    assert!(!service.should_continue());
}

#[test]
fn should_continue_ignores_budget_so_driver_can_block() {
    let service = GoalService::new();
    wire(&service);
    service.create("g", 1).unwrap();
    assert!(service.should_continue());
    // 推进到预算耗尽：仍返回 true（drivable 不含预算判据），由 driver 落 round-limit block。
    service.record_round_entered().unwrap();
    assert!(service.should_continue());
}

#[test]
fn record_round_entered_advances_and_persists() {
    let service = GoalService::new();
    let session = Arc::new(SessionLog::in_memory());
    service.wire_session(session.clone());
    service.create("g", 5).unwrap();
    let before = session.snapshot().len();
    service.record_round_entered().unwrap();
    assert_eq!(service.view().unwrap().rounds_started, 1);
    assert_eq!(session.snapshot().len(), before + 1);
    // 快照未变（round 不改变相位/objective）
    let events = session.snapshot();
    let last = events.last().unwrap();
    assert!(matches!(
        last,
        crate::session::SessionEvent::GoalChange { operation, .. } if operation == "round"
    ));
}

#[test]
fn cold_recovery_is_disarmed() {
    let service = GoalService::new();
    let session = Arc::new(SessionLog::in_memory());
    service.wire_session(session.clone());
    service.create("g", 5).unwrap();
    // 模拟冷恢复：新 service 从同一 session 重放
    let recovered = GoalService::new();
    recovered.wire_session(session);
    assert_eq!(recovered.activation(), GoalActivation::Disarmed);
    assert!(!recovered.should_continue());
    // durable 相位已恢复
    assert_eq!(recovered.view().unwrap().snapshot.objective, "g");
}

#[test]
fn authority_direct_human_by_default() {
    let service = GoalService::new();
    wire(&service);
    service.create("g", 5).unwrap();
    // 无任意 UserMessage → 回落 DirectHuman
    assert_eq!(service.authority(), Authority::DirectHuman);
}

#[test]
fn authority_direct_human_when_latest_user_is_plain() {
    let service = GoalService::new();
    let session = Arc::new(SessionLog::in_memory());
    service.wire_session(session.clone());
    service.create("g", 5).unwrap();
    // 普通 user 消息（无 source）→ DirectHuman，即使曾在 goal 续轮
    service.record_round_entered().unwrap();
    let msg = crate::types::AgentMessage::user("普通人类输入");
    session.append(crate::session::SessionEvent::UserMessage {
        turn: 1,
        step: 1,
        message: msg,
    });
    assert_eq!(service.authority(), Authority::DirectHuman);
}

#[test]
fn authority_goal_round_when_latest_user_is_goal_source() {
    let service = GoalService::new();
    let session = Arc::new(SessionLog::in_memory());
    service.wire_session(session.clone());
    let snapshot = service.create("g", 5).unwrap();
    service.record_round_entered().unwrap();
    // 模拟 goal 续轮已进入历史：最近一条 UserMessage 带 GoalRound source（匹配 goal_id）
    let msg = crate::types::AgentMessage {
        role: crate::types::Role::User,
        blocks: vec![crate::types::ContentBlock::Text { text: "续轮".to_string() }],
        source: Some(crate::types::MessageSource::GoalRound {
            goal_id: snapshot.id.clone(),
            revision: snapshot.revision,
            round: 1,
        }),
    };
    session.append(crate::session::SessionEvent::UserMessage { turn: 1, step: 1, message: msg });
    let view = service.view().unwrap();
    assert_eq!(service.authority(), Authority::GoalRound { round: view.rounds_started });
    // 退出续轮（后一条为普通 user）→ 回到 direct human
    let plain = crate::types::AgentMessage::user("人类输入");
    session.append(crate::session::SessionEvent::UserMessage { turn: 1, step: 2, message: plain });
    assert_eq!(service.authority(), Authority::DirectHuman);
}

#[test]
fn authority_none_when_no_goal() {
    let service = GoalService::new();
    wire(&service);
    assert_eq!(service.authority(), Authority::None);
}

#[test]
fn complete_replaces_via_create() {
    let service = GoalService::new();
    wire(&service);
    service.create("first", 5).unwrap();
    let r = service.view().unwrap().snapshot.ref_of();
    service.complete(r).unwrap();
    service.disarm();
    let second = service.create("second", 5).unwrap();
    assert_eq!(second.objective, "second");
}

#[test]
fn inject_wrapup_appends_user_message() {
    let service = GoalService::new();
    let session = Arc::new(SessionLog::in_memory());
    service.wire_session(session.clone());
    service.create("fix the parser", 5).unwrap();
    let before = session.snapshot().len();
    service.inject_wrapup("complete");
    let after = session.snapshot();
    assert_eq!(after.len(), before + 1);
    let last = after.last().unwrap();
    // 追加的是带 wrap-up 文本的 UserMessage
    let crate::session::SessionEvent::UserMessage { message, .. } = last else {
        panic!("expected UserMessage");
    };
    let text = message.text_content();
    assert!(text.contains("<goal_complete>"), "text: {text}");
    assert!(text.contains("Do not call any more tools"));
    // 不落 goal 事件（不改目标状态）
    let goal_events = after
        .iter()
        .filter(|e| matches!(e, crate::session::SessionEvent::GoalChange { .. }))
        .count();
    assert_eq!(goal_events, 1); // 仅 create 一条
}
