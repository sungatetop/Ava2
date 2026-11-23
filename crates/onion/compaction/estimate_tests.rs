//! `estimate` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use crate::types::{AgentMessage, ContentBlock, Role};

#[test]
fn empty_message_costs_four_tokens() {
    let message = AgentMessage::user("");
    assert_eq!(super::estimate_message(&message), 4);
}

#[test]
fn four_chars_cost_one_token() {
    let message = AgentMessage::user("abcdefgh"); // 8 chars → 2 tokens
    assert_eq!(super::estimate_message(&message), 2 + 4);
}

#[test]
fn unicode_chars_counted_per_char() {
    // 4 个汉字 = 4 chars → 1 token（ceil）。
    let message = AgentMessage::user("中文消息");
    assert_eq!(super::estimate_message(&message), 4 + 1);
}

#[test]
fn tool_use_blocks_counted() {
    let message = AgentMessage {
        role: Role::Assistant,
        blocks: vec![ContentBlock::ToolUse {
            id: "call_1".to_string(),
            name: "echo".to_string(),
            input: serde_json::json!({"x": 1}),
        }],
        source: None,
    };
    assert!(super::estimate_message(&message) > 4);
}

#[test]
fn image_blocks_cost_fixed_estimate() {
    // 图片块按预算后固定单图估算（IMAGE_TOKEN_ESTIMATE），与 data URL 字符数
    // 无关——两张不同体积的图片估算一致。
    let tiny = AgentMessage {
        role: Role::User,
        blocks: vec![ContentBlock::Image { image_url: "data:image/png;base64,AAAA".to_string() }],
        source: None,
    };
    let huge = AgentMessage {
        role: Role::User,
        blocks: vec![ContentBlock::Image {
            image_url: format!("data:image/png;base64,{}", "A".repeat(80_000)),
        }],
        source: None,
    };
    assert_eq!(super::estimate_message(&tiny), 4 + super::IMAGE_TOKEN_ESTIMATE);
    assert_eq!(super::estimate_message(&huge), 4 + super::IMAGE_TOKEN_ESTIMATE);
}

#[test]
fn estimate_context_sums_all_messages() {
    let context = crate::context::Context {
        system: Some("system prompt".to_string()),
        messages: vec![AgentMessage::user("hello"), AgentMessage::user("world")],
        tools: Vec::new(),
    };
    let total = super::estimate_context(&context);
    let expected = 4 // ceil(13/4) = 4（"system prompt" 13 字节）
        + super::estimate_message(&context.messages[0])
        + super::estimate_message(&context.messages[1]);
    assert_eq!(total, expected);
}
