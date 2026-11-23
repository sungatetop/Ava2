//! `relay` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;

#[test]
fn relay_steer_pushes_subagent_message_into_next_step() {
    let bus = crate::session::EventBus::new();
    let inbox = crate::inbox::Inbox::new(bus);
    let final_message = final_assistant_text("subagent done: files written");

    relay(&inbox, "agent-1", final_message, Delivery::Steer).expect("relay should succeed");

    assert!(inbox.has_next_step());
    assert!(!inbox.has_next_turn());
    let drained = inbox.drain_next_step();
    assert_eq!(drained.len(), 1);
    let message = &drained[0];
    assert_eq!(
        message.source,
        Some(crate::types::MessageSource::SubagentMessage {
            agent_id: "agent-1".to_string(),
        })
    );
    assert_eq!(message.text_content(), "subagent done: files written");
}

#[test]
fn relay_queue_pushes_into_next_turn_for_idle_parent() {
    let bus = crate::session::EventBus::new();
    let inbox = crate::inbox::Inbox::new(bus);

    let delivery = Delivery::for_parent_idle(true);
    assert_eq!(delivery, Delivery::Queue, "父代理空闲 → queue");
    relay(&inbox, "agent-7", final_assistant_text("late result"), delivery)
        .expect("relay should succeed");

    assert!(inbox.has_next_turn(), "queue 必须落 next-turn");
    assert!(!inbox.has_next_step());
}

#[test]
fn delivery_parse_defaults_to_queue_and_rejects_unknown() {
    assert_eq!(Delivery::parse(None).expect("缺省合法"), Delivery::Queue);
    assert_eq!(Delivery::parse(Some("queue")).expect("queue 合法"), Delivery::Queue);
    assert_eq!(Delivery::parse(Some("steer")).expect("steer 合法"), Delivery::Steer);
    assert!(Delivery::parse(Some("now")).is_err(), "未知取值必须报错");
    assert_eq!(Delivery::for_parent_idle(false), Delivery::Steer, "运行中 → steer");
}

#[test]
fn notice_carries_summary_and_stop_reason() {
    let bus = crate::session::EventBus::new();
    let inbox = crate::inbox::Inbox::new(bus);

    notice(&inbox, "agent-2", "scout", "completed", true, true, Delivery::Steer)
        .expect("notice should succeed");

    assert!(inbox.has_next_step());
    let drained = inbox.drain_next_step();
    assert_eq!(drained.len(), 1);
    let message = &drained[0];
    assert_eq!(
        message.source,
        Some(crate::types::MessageSource::SubagentSettled {
            agent_id: "agent-2".to_string(),
            stop_reason: Some("completed".to_string()),
            summary: Some(
                "Background subagent scout finished and will do no further work unless you send it more."
                    .to_string()
            ),
        })
    );
    // notice 是 model-visible 的一行账目 + 收尾说明。
    assert_eq!(message.role, crate::types::Role::User);
    let text = message.text_content();
    assert!(text.starts_with("Background subagent scout finished"));
    assert!(text.contains("closing message is omitted"));
}

#[test]
fn notice_reports_failure_reason_for_parent() {
    let bus = crate::session::EventBus::new();
    let inbox = crate::inbox::Inbox::new(bus);

    notice(&inbox, "agent-9", "scout", "error", true, false, Delivery::Steer)
        .expect("notice should succeed");

    let message = inbox.drain_next_step().remove(0);
    assert!(message.text_content().contains("failed before it finished"));
    assert!(message.text_content().contains("It left no closing message."));
}

#[test]
fn relay_and_notice_both_wake_parent() {
    let bus = crate::session::EventBus::new();
    let inbox = crate::inbox::Inbox::new(bus);

    notice(&inbox, "agent-3", "scout", "completed", true, false, Delivery::Steer)
        .expect("notice should succeed");
    assert!(inbox.take_wakeup(), "notice 应唤醒父会话");

    relay(&inbox, "agent-4", final_assistant_text("ok"), Delivery::Steer)
        .expect("relay should succeed");
    assert!(inbox.take_wakeup(), "relay 应唤醒父会话");
}

#[test]
fn relay_empty_blocks_assistant_message() {
    // 空 blocks 消息经 Context::apply 投影时被跳过——落日志但不进模型可见上下文
    // （「model-visible means logged」的反面：logged but not model-visible）。
    let message = AgentMessage {
        role: crate::types::Role::Assistant,
        blocks: Vec::new(),
        source: Some(crate::types::MessageSource::SubagentMessage {
            agent_id: "agent-5".to_string(),
        }),
    };
    let mut context = crate::context::Context::default();
    let event = crate::session::SessionEvent::UserMessage {
        turn: 1,
        step: 1,
        message,
    };
    context.apply(&event);
    assert!(
        context.messages.is_empty(),
        "空 blocks 消息不投影（Context::apply 过滤）"
    );
}
