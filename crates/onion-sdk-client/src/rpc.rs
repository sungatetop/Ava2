//! 协议信封类型（与 server 的 `rpc` 模块同构；client 独立定义不依赖 server）。

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 请求/响应/通知通用信封。
#[derive(Serialize, Deserialize, Clone)]
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

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RPC 错误 {}：{}", self.code, self.message)
    }
}
