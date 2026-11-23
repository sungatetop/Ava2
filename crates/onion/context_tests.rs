//! `context` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;
use crate::session::SessionEvent;
use crate::types::{AgentMessage, ContentBlock, ToolCall, Usage};

fn call(id: &str, name: &str) -> ToolCall {
    ToolCall { id: id.to_string(), name: name.to_string(), input: serde_json::json!({"text": "hi"}) }
}

/// 覆盖全部投影分支的事件序列。
fn sample_events() -> Vec<SessionEvent> {
    let tool_call = call("call-1", "echo");
    vec![
        SessionEvent::SessionStarted { session_id: "s".to_string(), model: "m".to_string() },
        SessionEvent::TurnStart { turn: 1 },
        SessionEvent::SystemMessage { turn: 1, step: 1, message: "system prompt".to_string() },
        SessionEvent::UserMessage { turn: 1, step: 1, message: AgentMessage::user("hello") },
        SessionEvent::StepStart { turn: 1, step: 1 },
        SessionEvent::AssistantMessage {
            turn: 1,
            step: 1,
            message: AgentMessage::assistant(vec![
                ContentBlock::Text { text: "thinking output".to_string() },
                ContentBlock::ToolUse {
                    id: tool_call.id.clone(),
                    name: tool_call.name.clone(),
                    input: tool_call.input.clone(),
                },
            ]),
            usage: Usage { input_tokens: 10, output_tokens: 5, cache_read_tokens: 0, cache_write_tokens: 0 },
        },
        SessionEvent::ToolCall { turn: 1, step: 1, call: tool_call.clone() },
        SessionEvent::ToolResult {
            turn: 1,
            step: 1,
            result: crate::types::ToolResult::text(&tool_call, "echo: hi"),
        },
        SessionEvent::AssistantAttempt { turn: 1, step: 1, error: "boom".to_string(), cancelled: false },
        SessionEvent::StepEnd { turn: 1, step: 1 },
        SessionEvent::TurnEnd { turn: 1, reason: "completed".to_string() },
        SessionEvent::SessionEnd { reason: "end".to_string() },
    ]
}

#[test]
fn project_equals_incremental_apply() {
    let events = sample_events();
    let mut incremental = Context { system: None, messages: Vec::new(), tools: Vec::new() };
    for event in &events {
        incremental.apply(event);
    }
    let projected = Context::project(&events, Vec::new());
    assert_eq!(incremental, projected);
    // 投影内容断言：system 1 条、消息 3 条（user + assistant + tool_result）
    assert_eq!(projected.system.as_deref(), Some("system prompt"));
    assert_eq!(projected.messages.len(), 3);
    assert_eq!(projected.messages[0].role, crate::types::Role::User);
    assert_eq!(projected.messages[1].role, crate::types::Role::Assistant);
    assert_eq!(projected.messages[2].role, crate::types::Role::Tool);
}

#[test]
fn empty_assistant_message_not_projected() {
    let mut context = Context::default();
    context.apply(&SessionEvent::AssistantMessage {
        turn: 1,
        step: 1,
        message: AgentMessage::assistant(Vec::new()),
        usage: Usage::default(),
    });
    assert!(context.messages.is_empty());
}

#[test]
fn derive_request_appends_pending() {
    let events = sample_events();
    let context = Context::project(&events, Vec::new());
    let pending = vec![AgentMessage::user("second turn")];
    let draft = context.derive_request("model-x".to_string(), 1024, None, pending.clone(), true);
    assert_eq!(draft.model, "model-x");
    assert_eq!(draft.max_tokens, 1024);
    assert!(draft.starts_request_series);
    assert_eq!(draft.system.as_deref(), Some("system prompt"));
    // context.messages(3) + pending(1)
    assert_eq!(draft.messages.len(), 4);
    assert_eq!(draft.messages[3], pending[0]);
    // 重试路径：pending 为空则不重复
    let retry_draft = context.derive_request("model-x".to_string(), 1024, None, Vec::new(), false);
    assert_eq!(retry_draft.messages.len(), 3);
    assert!(!retry_draft.starts_request_series);
}
