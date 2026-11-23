//! JSON-RPC 2.0 线格式 + SSE 帧化（onion-server 内部实现，不依赖 ava-a2a）。
//!
//! 载体：HTTP POST /a2a（request/response）与 /a2a/sse（server-sent events
//! streaming）。方法集对齐 A2A v1.0：
//!
//! - `message/send`：同步一轮对话，返回 Task 快照
//! - `message/stream`：SSE 流式，返回事件流（status-update / artifact-update）
//! - `tasks/get`：任务查询
//! - `tasks/provideInput`：回填跨实例挂起的人机交互项（plan-review / ask-user）
//! - `tasks/cancel`：任务取消
//! - `tasks/pushNotificationConfig/set` / `get`：webhook 推送配置
//! - `tasks/resubscribe`：重订阅事件流

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::a2a::Task;

/// JSON-RPC 2.0 Request。
#[derive(Clone, Debug, Deserialize)]
pub struct JsonRpcRequest {
    /// 协议版本位（恒为 `"2.0"`）：反序列化时强制存在，实现侧不消费。
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

/// JSON-RPC 2.0 Response。
#[derive(Clone, Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 Error 对象。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// JSON-RPC 2.0 错误码。
pub mod codes {
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    pub const INTERNAL_ERROR: i64 = -32603;
}

impl JsonRpcResponse {
    pub fn success(id: Value, result: Value) -> Self {
        Self { jsonrpc: "2.0".to_string(), id, result: Some(result), error: None }
    }

    pub fn error(id: Value, code: i64, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError { code, message: message.into(), data: None }),
        }
    }
}

/// A2A 支持的方法枚举。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum A2aMethod {
    MessageSend,
    MessageStream,
    TasksGet,
    TasksProvideInput,
    TasksCancel,
    PushConfigSet,
    PushConfigGet,
    TasksResubscribe,
}

impl A2aMethod {
    pub fn parse(method: &str) -> Option<Self> {
        match method {
            "message/send" => Some(Self::MessageSend),
            "message/stream" => Some(Self::MessageStream),
            "tasks/get" => Some(Self::TasksGet),
            "tasks/provideInput" => Some(Self::TasksProvideInput),
            "tasks/cancel" => Some(Self::TasksCancel),
            "tasks/pushNotificationConfig/set" => Some(Self::PushConfigSet),
            "tasks/pushNotificationConfig/get" => Some(Self::PushConfigGet),
            "tasks/resubscribe" => Some(Self::TasksResubscribe),
            _ => None,
        }
    }
}

/// JSON-RPC 错误便捷构造器。
pub fn method_not_found(id: Value) -> JsonRpcResponse {
    JsonRpcResponse::error(id, codes::METHOD_NOT_FOUND, "Method not found")
}

pub fn invalid_params(id: Value, message: impl Into<String>) -> JsonRpcResponse {
    JsonRpcResponse::error(id, codes::INVALID_PARAMS, message)
}

/// Task → JSON result 转换（message/send / tasks/get / tasks/cancel 返回）。
pub fn task_to_value(task: &Task) -> Value {
    serde_json::to_value(task).unwrap_or(Value::Null)
}
