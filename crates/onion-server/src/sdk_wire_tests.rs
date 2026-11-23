//! `sdk_wire` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//!
//! 覆盖协议形状与文本提取；入队/事件泵路径与 ACP 同源（gate + inbox + spawn_turn），
//! 真实链路经二进制 stdio 冒烟验证。

use serde_json::json;

use super::*;

#[test]
fn initialize_result_carries_wire_stable_server_info() {
    let result = initialize_result();
    assert_eq!(result["serverInfo"]["name"], json!("onion-server"));
    assert_eq!(result["serverInfo"]["version"], json!(env!("CARGO_PKG_VERSION")));
}

#[test]
fn content_blocks_text_joins_text_and_skips_images() {
    let params = json!({
        "sessionId": "sdk-1",
        "contentBlocks": [
            { "type": "text", "text": "hello" },
            { "type": "image", "data": "…", "mimeType": "image/png" },
            { "type": "text", "text": "world" }
        ]
    });
    assert_eq!(content_blocks_text(&params).unwrap(), "hello\nworld");
}

#[test]
fn content_blocks_text_rejects_missing_or_empty_content() {
    assert!(content_blocks_text(&json!({ "sessionId": "s" }))
        .unwrap_err()
        .contains("must be an array"));
    let blank = json!({ "contentBlocks": [{ "type": "text", "text": "   " }] });
    assert!(content_blocks_text(&blank).unwrap_err().contains("no text content"));
}

#[test]
fn notification_and_response_envelopes_are_jsonrpc() {
    assert_eq!(
        notification("session.status", json!({ "sessionId": "s", "status": "idle" })),
        json!({
            "jsonrpc": "2.0",
            "method": "session.status",
            "params": { "sessionId": "s", "status": "idle" }
        })
    );
    assert_eq!(
        success_response(json!(1), json!({ "messageId": "m1" })),
        json!({ "jsonrpc": "2.0", "id": 1, "result": { "messageId": "m1" } })
    );
    assert_eq!(
        error_response(json!(2), METHOD_NOT_FOUND, "unsupported"),
        json!({ "jsonrpc": "2.0", "id": 2, "error": { "code": -32601, "message": "unsupported" } })
    );
}
