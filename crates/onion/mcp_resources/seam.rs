//! MCP 资源 seam：可注册 provider 的服务 + 3 个共享资源工具。
//!
//! 核心角色：
//! - `McpResourceProvider`：一个已配置 server 的资源访问（连接拥有）；
//! - `McpResourceRequest`：闭集操作（list / templates/list / read）；
//! - `McpResourceService`：按 server 名注册 provider，工具转发到 provider；
//! - 缺省无 provider → 结构化 `MCP_UNAVAILABLE`（无可见 server 不产生网络请求）。
//!
//! 本模块只负责 seam 与工具壳，真实 JSON-RPC 客户端（stdio / streamable-http）在
//! `client.rs`，由装配侧作为 provider 注册进来。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::json;

use crate::tools::ToolPort;
use crate::types::{ContentBlock, ToolCall, ToolDefinition};
use crate::waterfall::BoxFuture;
use tokio_util::sync::CancellationToken;

/// 一种受支持的资源操作，带 server 拥有的游标与 URI。
#[derive(Clone, Debug)]
pub enum McpResourceRequest {
    /// `resources/list`（可带 `cursor` 翻页）。
    List { cursor: Option<String> },
    /// `resources/templates/list`（可带 `cursor` 翻页）。
    ListTemplates { cursor: Option<String> },
    /// `resources/read`（按 URI 读）。
    Read { uri: String },
}

/// 一个已配置 server 的资源访问，由其 MCP 连接插件拥有。
pub trait McpResourceProvider: Send + Sync {
    /// 对指定操作执行一次请求，返回无损 JSON 协议结果。
    fn request(&self, request: &McpResourceRequest) -> Result<serde_json::Value, McpResourceError>;
}

/// 结构化 MCP 失败，带稳定 `code`（调用方据此路由，而非解析 message）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpResourceError {
    pub message: String,
    pub code: McpResourceErrorCode,
}

/// 稳定错误码（资源工具失败语义）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum McpResourceErrorCode {
    /// 指定的 server 未配置 / 不在调用方作用域。
    Unavailable,
    /// 连接 / 协议请求失败。
    Failed,
    /// provider 返回畸形响应。
    MalformedResponse,
}

impl McpResourceError {
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self { message: message.into(), code: McpResourceErrorCode::Unavailable }
    }
    pub fn failed(message: impl Into<String>) -> Self {
        Self { message: message.into(), code: McpResourceErrorCode::Failed }
    }
    pub fn malformed(message: impl Into<String>) -> Self {
        Self { message: message.into(), code: McpResourceErrorCode::MalformedResponse }
    }
}

impl std::fmt::Display for McpResourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for McpResourceError {}

/// MCP 资源能力 seam：按 server 名注册 provider。
/// 无配置的 server 在解析阶段即失败，不发起网络请求（unavailable 不产生请求）。
#[derive(Clone, Default)]
pub struct McpResourceService {
    providers: Arc<Mutex<HashMap<String, Arc<dyn McpResourceProvider>>>>,
}

impl McpResourceService {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一个 server 的 provider（同名重复注册报冲突）。
    pub fn register(
        &self,
        server: &str,
        provider: Arc<dyn McpResourceProvider>,
    ) -> Result<(), McpResourceError> {
        if server.trim().is_empty() {
            return Err(McpResourceError::unavailable(
                "an MCP server name must be a non-empty string",
            ));
        }
        let mut providers = self
            .providers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if providers.contains_key(server) {
            return Err(McpResourceError::unavailable(format!(
                "MCP server \"{server}\" is already registered"
            )));
        }
        providers.insert(server.to_string(), provider);
        Ok(())
    }

    /// 注销一个 server 的 provider，返回被摘除的 provider（未注册时为 `None`）。
    ///
    /// 幂等：对不存在的 server 调用不报错，便于配置收敛（reconcile）按期望态
    /// 批量摘除。摘除后该 server 的资源请求立即回到 `MCP_UNAVAILABLE`。
    pub fn unregister(&self, server: &str) -> Option<Arc<dyn McpResourceProvider>> {
        self.providers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(server)
    }

    /// 是否有任何已注册 provider（供装配 / 冒烟诊断）。
    pub fn has_providers(&self) -> bool {
        !self
            .providers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
    }

    /// 已注册 server 名列表（排序，供 prompt 段与诊断）。
    pub fn server_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .providers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .keys()
            .cloned()
            .collect();
        names.sort();
        names
    }

    /// 解析调用方可见 server 并执行一次操作（先解析再请求）。
    pub fn request(
        &self,
        server: &str,
        request: &McpResourceRequest,
    ) -> Result<serde_json::Value, McpResourceError> {
        let provider = self
            .providers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(server)
            .cloned();
        let Some(provider) = provider else {
            return Err(McpResourceError::unavailable(format!(
                "MCP resource server \"{server}\" is unavailable in this agent's scope"
            )));
        };
        provider.request(request)
    }
}

/// 渲染资源操作结果：成功前缀 `MCP server: <server>` + JSON；
/// 字符串型 `blob` 字段替换为二进制占位描述。
pub fn render_resource_result(server: &str, value: &serde_json::Value) -> String {
    let mut value = value.clone();
    redact_blobs(&mut value);
    format!(
        "MCP server: {server}\n{}",
        serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string())
    )
}

/// 递归将字符串型 `blob` 字段替换为二进制占位描述（二进制不内联，
/// 程序化调用者保留完整 base64）。
fn redact_blobs(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, v) in map.iter_mut() {
                if key == "blob" {
                    if let serde_json::Value::String(s) = v {
                        *v = json!(format!(
                            "[binary resource: {} base64 characters; available to programmatic callers]",
                            s.len()
                        ));
                    }
                } else {
                    redact_blobs(v);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr.iter_mut() {
                redact_blobs(v);
            }
        }
        _ => {}
    }
}

/// 共享资源工具载体：持有 seam，注册 3 个工具。
pub struct McpResourceToolSet {
    service: Arc<McpResourceService>,
}

impl McpResourceToolSet {
    pub fn new(service: Arc<McpResourceService>) -> Self {
        Self { service }
    }

    /// 注册 3 个共享资源工具进注册表。
    pub fn register_all(&self, registry: &mut crate::tools::ToolRegistry) {
        let service = self.service.clone();
        registry.register(Arc::new(ListMcpResourcesTool { service: service.clone() }));
        registry.register(Arc::new(ListMcpResourceTemplatesTool { service: service.clone() }));
        registry.register(Arc::new(ReadMcpResourceTool { service }));
    }
}

/// list_mcp_resources：列出某 MCP server 可用的资源。
struct ListMcpResourcesTool {
    service: Arc<McpResourceService>,
}

impl ToolPort for ListMcpResourcesTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "list_mcp_resources".to_string(),
            description: "List resources available from an MCP server."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "server": { "type": "string", "description": "Configured MCP server name." },
                    "cursor": { "type": "string", "description": "Continuation cursor returned by this server." }
                },
                "required": ["server"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let service = self.service.clone();
        Box::pin(async move {
            let server = call.input.get("server").and_then(serde_json::Value::as_str).unwrap_or("");
            if server.trim().is_empty() {
                return Err("server must be a non-empty string".to_string());
            }
            let cursor = call.input.get("cursor").and_then(serde_json::Value::as_str).map(str::to_string);
            let request = McpResourceRequest::List { cursor };
            service
                .request(server, &request)
                .map(|value| vec![ContentBlock::Text { text: render_resource_result(server, &value) }])
                .map_err(|e| e.to_string())
        })
    }
}

/// list_mcp_resource_templates：列出某 MCP server 的参数化资源 URI 模板。
struct ListMcpResourceTemplatesTool {
    service: Arc<McpResourceService>,
}

impl ToolPort for ListMcpResourceTemplatesTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "list_mcp_resource_templates".to_string(),
            description: "List parameterized resource URI templates from an MCP server."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "server": { "type": "string", "description": "Configured MCP server name." },
                    "cursor": { "type": "string", "description": "Continuation cursor returned by this server." }
                },
                "required": ["server"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let service = self.service.clone();
        Box::pin(async move {
            let server = call.input.get("server").and_then(serde_json::Value::as_str).unwrap_or("");
            if server.trim().is_empty() {
                return Err("server must be a non-empty string".to_string());
            }
            let cursor = call.input.get("cursor").and_then(serde_json::Value::as_str).map(str::to_string);
            let request = McpResourceRequest::ListTemplates { cursor };
            service
                .request(server, &request)
                .map(|value| vec![ContentBlock::Text { text: render_resource_result(server, &value) }])
                .map_err(|e| e.to_string())
        })
    }
}

/// read_mcp_resource：按 URI 读取某 MCP server 的资源。
struct ReadMcpResourceTool {
    service: Arc<McpResourceService>,
}

impl ToolPort for ReadMcpResourceTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "read_mcp_resource".to_string(),
            description: "Read an MCP resource by URI from the named server. Use a listed URI or an expanded resource template."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "server": { "type": "string", "description": "Configured MCP server name." },
                    "uri": { "type": "string", "description": "Resource URI to read." }
                },
                "required": ["server", "uri"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let service = self.service.clone();
        Box::pin(async move {
            let server = call.input.get("server").and_then(serde_json::Value::as_str).unwrap_or("");
            let uri = call.input.get("uri").and_then(serde_json::Value::as_str).unwrap_or("");
            if server.trim().is_empty() {
                return Err("server must be a non-empty string".to_string());
            }
            if uri.trim().is_empty() {
                return Err("uri must be a non-empty string".to_string());
            }
            let request = McpResourceRequest::Read { uri: uri.to_string() };
            service
                .request(server, &request)
                .map(|value| vec![ContentBlock::Text { text: render_resource_result(server, &value) }])
                .map_err(|e| e.to_string())
        })
    }
}

#[cfg(test)]
#[path = "seam_tests.rs"]
mod tests;
