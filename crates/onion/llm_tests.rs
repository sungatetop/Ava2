//! `llm` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;
use crate::types::{AgentMessage, ContentBlock, Role};

fn tool_message(content: Vec<ContentBlock>) -> AgentMessage {
    AgentMessage {
        role: Role::Tool,
        blocks: vec![ContentBlock::ToolResult {
            tool_call_id: "call-1".into(),
            is_error: false,
            content,
        }],
        source: None,
    }
}

#[test]
fn tool_result_with_image_becomes_parts_array() {
    let message = tool_message(vec![
        ContentBlock::Text { text: "读取成功".into() },
        ContentBlock::Image { image_url: "data:image/jpeg;base64,AAAA".into() },
    ]);
    let wire = wire_message(&message).expect("tool 消息必须产生线协议条目");
    assert_eq!(wire["role"], json!("tool"));
    assert_eq!(wire["tool_call_id"], json!("call-1"));
    let parts = wire["content"].as_array().expect("含图结果 content 必须是 parts 数组");
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0], json!({ "type": "text", "text": "读取成功" }));
    assert_eq!(
        parts[1],
        json!({ "type": "image_url", "image_url": { "url": "data:image/jpeg;base64,AAAA" } })
    );
}

#[test]
fn tool_result_text_only_stays_string() {
    let message = tool_message(vec![ContentBlock::Text { text: "纯文本结果".into() }]);
    let wire = wire_message(&message).expect("tool 消息必须产生线协议条目");
    assert_eq!(
        wire,
        json!({ "role": "tool", "tool_call_id": "call-1", "content": "纯文本结果" })
    );
}

#[test]
fn classify_status_maps_provider_errors() {
    assert_eq!(classify_status(429, ""), LlmFailureCode::RateLimit);
    assert_eq!(classify_status(500, ""), LlmFailureCode::Server);
    assert_eq!(classify_status(503, ""), LlmFailureCode::Server);
    assert_eq!(classify_status(401, ""), LlmFailureCode::Auth);
    assert_eq!(classify_status(403, ""), LlmFailureCode::Auth);
    assert_eq!(classify_status(402, ""), LlmFailureCode::Quota);
    assert_eq!(classify_status(400, ""), LlmFailureCode::InvalidRequest);
    assert_eq!(classify_status(413, ""), LlmFailureCode::InvalidRequest);
    assert_eq!(classify_status(418, ""), LlmFailureCode::Http(418));
}

#[test]
fn classify_status_overflow_body_wins() {
    assert_eq!(
        classify_status(
            500,
            "{\"error\":{\"message\":\"This model's maximum context length is 65536 tokens\"}}"
        ),
        LlmFailureCode::ContextWindowExceeded
    );
    assert_eq!(
        classify_status(429, "prompt is too long"),
        LlmFailureCode::ContextWindowExceeded
    );
}

#[test]
fn parse_retry_after_accepts_delta_seconds_only() {
    assert_eq!(parse_retry_after_ms("2"), Some(2_000));
    assert_eq!(parse_retry_after_ms(" 30 "), Some(30_000));
    assert_eq!(parse_retry_after_ms("0"), None);
    assert_eq!(
        parse_retry_after_ms("Wed, 21 Oct 2015 07:28:00 GMT"),
        None,
        "HTTP-date 形态教学版不解析"
    );
    assert_eq!(parse_retry_after_ms("soon"), None);
}

#[test]
fn settle_empty_accum_yields_empty_response_failure() {
    let state = StreamState {
        body: None,
        buffer: String::new(),
        pending: VecDeque::new(),
        accum: SseAccum::default(),
        cancel: CancellationToken::new(),
        model: "fake".to_string(),
        finished: false,
    };
    let LlmStreamItem::Done(turn) = settle(&state) else {
        panic!("settle 必须产出 Done");
    };
    assert_eq!(
        turn.error.as_deref(),
        Some("供应商返回空响应（无任何内容块）")
    );
    let failure = turn.failure.expect("空响应必须携带结构化失败");
    assert_eq!(failure.code, LlmFailureCode::EmptyResponse);
}
