//! JSON-RPC 2.0 协议类型 + 方法分派。
//!
//! 方法集：7 请求 + 4 通知 + 2 审批回填。`session/prompt` 只返回入队
//! 回执（`message_id`），结果靠 `session.event` 通知流观察（贴合 agent 长 turn）。

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// 请求/响应/通知通用信封。
#[derive(Serialize, Deserialize)]
pub struct RpcMessage {
    #[serde(rename = "jsonrpc")]
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(rename = "method", skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

/// JSON-RPC 错误对象。
#[derive(Serialize, Deserialize, Clone)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl RpcError {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), data: None }
    }
}

/// 标准错误码。
pub mod code {
    pub const PARSE_ERROR: i64 = -32700;
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    pub const BUSINESS_ERROR: i64 = -32000;
    pub const INTERNAL_ERROR: i64 = -32603;
}

/// 构造响应（成功）。
pub fn response(id: u64, result: Value) -> String {
    serde_json::to_string(&RpcMessage {
        jsonrpc: "2.0".to_string(),
        id: Some(id),
        method: None,
        params: None,
        result: Some(result),
        error: None,
    })
    .unwrap_or_else(|_| "{}".to_string())
}

/// 构造响应（错误）。
pub fn error_response(id: u64, error: RpcError) -> String {
    serde_json::to_string(&RpcMessage {
        jsonrpc: "2.0".to_string(),
        id: Some(id),
        method: None,
        params: None,
        result: None,
        error: Some(error),
    })
    .unwrap_or_else(|_| "{}".to_string())
}

/// 构造通知（无 id，单向）。
pub fn notification(method: &str, params: Value) -> String {
    serde_json::to_string(&RpcMessage {
        jsonrpc: "2.0".to_string(),
        id: None,
        method: Some(method.to_string()),
        params: Some(params),
        result: None,
        error: None,
    })
    .unwrap_or_else(|_| "{}".to_string())
}

/// 会话事件通知。
pub fn session_event(session_id: &str, event: &Value) -> String {
    notification("session.event", json!({ "session_id": session_id, "event": event }))
}
