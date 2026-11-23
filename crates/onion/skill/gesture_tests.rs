//! `skill::gesture` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::sync::{Arc, Mutex};

use crate::listener::{ClaimSource, ClaimedInput, Listener};
use crate::session::SessionLog;
use crate::skill::gesture::SkillGestureListener;
use crate::skill::registry::SkillRegistry;
use crate::skill::{SkillInvocationPolicy, SkillRegistration, SkillSource};
use crate::types::{AgentMessage, MessageSource};

fn registry() -> Arc<Mutex<SkillRegistry>> {
    Arc::new(Mutex::new(SkillRegistry::new()))
}

fn gesture(registry: Arc<Mutex<SkillRegistry>>) -> SkillGestureListener {
    SkillGestureListener::new(registry)
}

fn registration(name: &str, user_invocable: bool, content: &str) -> SkillRegistration {
    SkillRegistration {
        name: name.to_string(),
        description: format!("desc {name}"),
        when_to_use: None,
        source: SkillSource::Runtime,
        resource_base: None,
        content: content.to_string(),
        invocation: Some(SkillInvocationPolicy {
            model_invocable: true,
            user_invocable,
        }),
        provider: None,
    }
}

fn input(messages: Vec<AgentMessage>) -> ClaimedInput {
    ClaimedInput {
        turn: 1,
        step: 1,
        source: ClaimSource::TurnOpen,
        messages,
        starts_request_series: true,
    }
}

async fn run(
    listener: &SkillGestureListener,
    session: Arc<SessionLog>,
    messages: Vec<AgentMessage>,
) -> Vec<AgentMessage> {
    let ctx = crate::listener::ListenerCtx {
        inbox: Arc::new(crate::inbox::Inbox::new(session.bus().clone())),
        session,
        state: "Running".to_string(),
    };
    match listener
        .pre_step(
            ctx,
            input(messages),
            Box::new(|i| {
                Box::pin(async move {
                    crate::listener::PreStepDecision::Enter {
                        messages: i.messages,
                        starts_request_series: true,
                    }
                })
            }),
        )
        .await
    {
        crate::listener::PreStepDecision::Enter { messages, .. } => messages,
        crate::listener::PreStepDecision::Reject { reason } => panic!("unexpected reject: {reason}"),
    }
}

fn injections(messages: &[AgentMessage]) -> Vec<&AgentMessage> {
    messages
        .iter()
        .filter(|m| matches!(m.source, Some(MessageSource::SkillInvocation { .. })))
        .collect()
}

#[tokio::test]
async fn injects_user_invocable_skill_body_on_gesture() {
    let reg = registry();
    {
        let mut guard = reg.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.register(registration("my-skill", true, "Do the thing."));
    }
    let session = Arc::new(SessionLog::in_memory());
    let messages = run(
        &gesture(reg),
        session,
        vec![AgentMessage::user("please use /my-skill now")],
    )
    .await;
    let injected = injections(&messages);
    assert_eq!(injected.len(), 1);
    let MessageSource::SkillInvocation { name } = injected[0].source.as_ref().unwrap() else {
        panic!("expected skill-invocation");
    };
    assert_eq!(name, "my-skill");
    let text = injected[0].text_content();
    assert!(text.contains("<skill_content name=\"my-skill\">"), "{text}");
    assert!(text.contains("Do the thing."), "{text}");
}

#[tokio::test]
async fn unknown_or_user_disabled_skill_stays_prose() {
    let reg = registry();
    {
        let mut guard = reg.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.register(registration("user-only", false, "User only body."));
    }
    let session = Arc::new(SessionLog::in_memory());
    // 未知名 + user-disabled。
    let messages = run(
        &gesture(reg),
        session,
        vec![AgentMessage::user("/missing and /user-only")],
    )
    .await;
    assert!(injections(&messages).is_empty());
}

#[tokio::test]
async fn does_not_scan_non_user_messages() {
    let reg = registry();
    {
        let mut guard = reg.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.register(registration("my-skill", true, "body"));
    }
    let session = Arc::new(SessionLog::in_memory());
    // 源标记消息（如 runtime context）含 /my-skill，但不得被手势扫描。
    let marked = AgentMessage {
        role: crate::types::Role::User,
        blocks: vec![crate::types::ContentBlock::Text {
            text: "/my-skill inside a system frame".to_string(),
        }],
        source: Some(MessageSource::RuntimeContext),
    };
    let messages = run(&gesture(reg), session, vec![marked]).await;
    assert!(injections(&messages).is_empty());
}

#[tokio::test]
async fn dedups_gesture_tokens_in_first_seen_order() {
    let reg = registry();
    {
        let mut guard = reg.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.register(registration("a-skill", true, "A."));
        guard.register(registration("b-skill", true, "B."));
    }
    let session = Arc::new(SessionLog::in_memory());
    let messages = run(
        &gesture(reg),
        session,
        vec![AgentMessage::user("/b-skill and /a-skill then /b-skill again")],
    )
    .await;
    let injected = injections(&messages);
    assert_eq!(injected.len(), 2);
    let names: Vec<&str> = injected
        .iter()
        .map(|m| match m.source.as_ref().unwrap() {
            MessageSource::SkillInvocation { name } => name.as_str(),
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(names, vec!["b-skill", "a-skill"]);
}

#[test]
fn rejects_paths_and_fractions() {
    // 直接验证 invoked_skill_names 不把文件路径 / 分数当手势。
    let messages = vec![
        AgentMessage::user("see /usr/bin and the 5/8 ratio"),
        AgentMessage::user("but /ok-skill works"),
    ];
    // 通过 listener 间接验证：注册 ok-skill，仅它被注入。
    let names = super::invoked_skill_names(&messages);
    assert_eq!(names, vec!["ok-skill"]);
}

#[tokio::test]
async fn injections_appended_after_existing_messages() {
    let reg = registry();
    {
        let mut guard = reg.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.register(registration("my-skill", true, "body"));
    }
    let session = Arc::new(SessionLog::in_memory());
    let messages = run(
        &gesture(reg),
        session,
        vec![AgentMessage::user("hello /my-skill world")],
    )
    .await;
    // 注入在末尾，且保留原 user 消息。
    assert!(messages.len() >= 2);
    assert!(injections(&messages).len() == 1);
    let last = messages.last().unwrap();
    assert!(matches!(last.source, Some(MessageSource::SkillInvocation { .. })));
}
