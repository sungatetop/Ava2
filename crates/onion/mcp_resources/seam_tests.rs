//! `seam` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::sync::Arc;

use serde_json::json;

use super::{
    McpResourceError, McpResourceProvider, McpResourceRequest, McpResourceService,
    McpResourceToolSet, render_resource_result,
};

/// 简易 provider：按 server 返回固定结果。
struct FakeProvider {
    server: String,
}

impl McpResourceProvider for FakeProvider {
    fn request(&self, request: &McpResourceRequest) -> Result<serde_json::Value, McpResourceError> {
        match request {
            McpResourceRequest::List { .. } => Ok(json!({
                "resources": [{ "uri": "file:///a.txt", "name": "a" }],
                "nextCursor": null
            })),
            McpResourceRequest::ListTemplates { .. } => Ok(json!({
                "resourceTemplates": [{ "uriTemplate": "file:///{name}", "name": "tpl" }]
            })),
            McpResourceRequest::Read { uri } => Ok(json!({
                "contents": [{ "uri": uri, "text": "hello" }]
            })),
        }
    }
}

#[test]
fn service_resolves_provider_by_server() {
    let service = McpResourceService::new();
    service
        .register("mem", Arc::new(FakeProvider { server: "mem".to_string() }))
        .unwrap();
    assert!(service.has_providers());
    assert_eq!(service.server_names(), vec!["mem".to_string()]);
    let result = service
        .request("mem", &McpResourceRequest::Read { uri: "file:///a.txt".to_string() })
        .unwrap();
    assert_eq!(result["contents"][0]["text"], "hello");
}

#[test]
fn unavailable_server_does_not_network() {
    let service = McpResourceService::new();
    let err = service
        .request("ghost", &McpResourceRequest::List { cursor: None })
        .unwrap_err();
    assert_eq!(err.code, super::McpResourceErrorCode::Unavailable);
    assert!(err.message.contains("ghost"));
}

#[test]
fn duplicate_server_name_conflicts() {
    let service = McpResourceService::new();
    service.register("mem", Arc::new(FakeProvider { server: "mem".to_string() })).unwrap();
    let err = service
        .register("mem", Arc::new(FakeProvider { server: "mem".to_string() }))
        .unwrap_err();
    assert_eq!(err.code, super::McpResourceErrorCode::Unavailable);
}

#[test]
fn empty_server_name_rejected() {
    let service = McpResourceService::new();
    let err = service.register("", Arc::new(FakeProvider { server: "".to_string() })).unwrap_err();
    assert_eq!(err.code, super::McpResourceErrorCode::Unavailable);
}

#[test]
fn unregister_removes_provider_and_is_idempotent() {
    let service = McpResourceService::new();
    service.register("mem", Arc::new(FakeProvider { server: "mem".to_string() })).unwrap();
    assert!(service.unregister("mem").is_some());
    assert!(!service.has_providers());
    assert!(service.server_names().is_empty());
    // 摘除后资源请求回到 unavailable（不发起网络请求）。
    let err = service
        .request("mem", &McpResourceRequest::List { cursor: None })
        .unwrap_err();
    assert_eq!(err.code, super::McpResourceErrorCode::Unavailable);
    // 幂等：重复摘除不报错，返回 None。
    assert!(service.unregister("mem").is_none());
    // 摘除后可重新注册同名 server（配置收敛后重建 provider）。
    service.register("mem", Arc::new(FakeProvider { server: "mem".to_string() })).unwrap();
    assert_eq!(service.server_names(), vec!["mem".to_string()]);
}

#[test]
fn render_redacts_blobs() {
    let value = json!({
        "contents": [{ "uri": "file:///img.png", "mimeType": "image/png", "blob": "aGVsbG8=" }]
    });
    let text = render_resource_result("mem", &value);
    assert!(text.starts_with("MCP server: mem\n"));
    assert!(text.contains("[binary resource: 8 base64 characters"));
    assert!(!text.contains("aGVsbG8="));
}

#[test]
fn tool_set_registers_three_tools() {
    let mut registry = crate::tools::ToolRegistry::new();
    let service = Arc::new(McpResourceService::new());
    service.register("mem", Arc::new(FakeProvider { server: "mem".to_string() })).unwrap();
    let set = McpResourceToolSet::new(service);
    set.register_all(&mut registry);
    registry.state().set_agent([
        "list_mcp_resources".into(),
        "list_mcp_resource_templates".into(),
        "read_mcp_resource".into(),
    ]);
    let names: Vec<String> = registry
        .definitions()
        .iter()
        .map(|d| d.name.clone())
        .collect();
    assert!(names.contains(&"list_mcp_resources".to_string()));
    assert!(names.contains(&"list_mcp_resource_templates".to_string()));
    assert!(names.contains(&"read_mcp_resource".to_string()));
}
