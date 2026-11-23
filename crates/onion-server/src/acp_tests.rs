//! `acp` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//!
//! 覆盖协议形状与事件投影；轮次驱动路径与 A2A/REST 同源（`gate.enqueue` +
//! `inbox.followup` + `spawn_turn`），真实链路经二进制 stdio 冒烟验证。

use onion::session::SessionEvent;
use onion::types::{AgentMessage, ContentBlock, ToolCall, ToolResult, Usage};
use serde_json::json;

use super::*;

#[test]
fn initialize_reports_protocol_version_and_capabilities() {
    let result = initialize_result(&json!({ "protocolVersion": 1 }));
    assert_eq!(result["protocolVersion"], json!(PROTOCOL_VERSION));
    assert_eq!(result["agentCapabilities"]["loadSession"], json!(false));
    assert_eq!(result["agentCapabilities"]["promptCapabilities"]["image"], json!(false));
    assert_eq!(result["authMethods"], json!([]));
}

#[test]
fn prompt_text_joins_text_blocks_and_skips_other_kinds() {
    let params = json!({
        "sessionId": "s1",
        "prompt": [
            { "type": "text", "text": "first" },
            { "type": "image", "data": "…", "mimeType": "image/png" },
            { "type": "text", "text": "second" }
        ]
    });
    assert_eq!(prompt_text(&params).unwrap(), "first\nsecond");
}

#[test]
fn prompt_text_rejects_missing_or_non_text_prompt() {
    assert!(prompt_text(&json!({})).unwrap_err().contains("must be an array"));
    let only_image = json!({ "prompt": [{ "type": "image", "data": "" }] });
    assert!(prompt_text(&only_image).unwrap_err().contains("no text content"));
}

#[test]
fn assistant_message_projects_to_message_and_thought_chunks() {
    let message = AgentMessage::assistant(vec![
        ContentBlock::Thinking { thinking: "hmm".to_string() },
        ContentBlock::Text { text: "answer".to_string() },
    ]);
    let updates = session_updates(&SessionEvent::AssistantMessage {
        turn: 1,
        step: 1,
        message,
        usage: Usage::default(),
    });
    assert_eq!(
        updates,
        vec![
            json!({
                "sessionUpdate": "agent_thought_chunk",
                "content": { "type": "text", "text": "hmm" }
            }),
            json!({
                "sessionUpdate": "agent_message_chunk",
                "content": { "type": "text", "text": "answer" }
            }),
        ]
    );
}

#[test]
fn tool_call_and_result_project_to_acp_tool_updates() {
    let call = ToolCall {
        id: "call-1".to_string(),
        name: "Read".to_string(),
        input: json!({ "path": "a.txt" }),
    };
    assert_eq!(
        session_updates(&SessionEvent::ToolCall { turn: 1, step: 1, call }),
        vec![json!({
            "sessionUpdate": "tool_call",
            "toolCallId": "call-1",
            "title": "Read",
            "kind": "other",
            "status": "pending",
            "rawInput": { "path": "a.txt" }
        })]
    );
    let result = ToolResult {
        tool_call_id: "call-1".to_string(),
        name: "Read".to_string(),
        is_error: false,
        content: vec![ContentBlock::Text { text: "contents".to_string() }],
    };
    assert_eq!(
        session_updates(&SessionEvent::ToolResult { turn: 1, step: 1, result }),
        vec![json!({
            "sessionUpdate": "tool_call_update",
            "toolCallId": "call-1",
            "status": "completed",
            "content": [{ "type": "content", "content": { "type": "text", "text": "contents" } }]
        })]
    );
}

#[test]
fn tool_result_text_truncates_long_output() {
    let result = ToolResult {
        tool_call_id: "call-2".to_string(),
        name: "Bash".to_string(),
        is_error: true,
        content: vec![ContentBlock::Text { text: "x".repeat(MAX_UPDATE_TEXT_CHARS + 10) }],
    };
    let text = tool_result_text(&result);
    assert!(text.ends_with("[truncated]"));
    assert_eq!(text.chars().count(), MAX_UPDATE_TEXT_CHARS + "\n… [truncated]".chars().count());
    let updates = session_updates(&SessionEvent::ToolResult { turn: 1, step: 1, result });
    assert_eq!(updates[0]["status"], json!("failed"));
}

#[test]
fn unprojected_events_yield_no_updates() {
    assert!(session_updates(&SessionEvent::TurnStart { turn: 1 }).is_empty());
    assert!(session_updates(&SessionEvent::TurnEnd { turn: 1, reason: "completed".to_string() })
        .is_empty());
}

#[test]
fn responses_follow_jsonrpc_envelope() {
    assert_eq!(
        success_response(json!(3), json!({ "ok": true })),
        json!({ "jsonrpc": "2.0", "id": 3, "result": { "ok": true } })
    );
    assert_eq!(
        error_response(json!(4), METHOD_NOT_FOUND, "nope"),
        json!({ "jsonrpc": "2.0", "id": 4, "error": { "code": -32601, "message": "nope" } })
    );
}
