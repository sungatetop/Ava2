//! `bridge` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::sync::Arc;

use serde_json::json;

use super::{BridgeTool, McpBridgeToolSet, RawTool, public_tool_name};
use crate::mcp_resources::client::{McpClientProvider, McpServerConfig};
use crate::mcp_resources::seam::McpResourceService;
use crate::mcp_resources::McpServerState;
use crate::tools::{ToolPort, ToolRegistry};

fn stdio_server(name: &str) -> McpServerConfig {
    McpServerConfig::Stdio {
        server_name: name.to_string(),
        command: "node".to_string(),
        args: vec![],
        env: Default::default(),
        cwd: None,
        tool_call_timeout_ms: 1000,
    }
}

#[test]
fn public_name_round_trip() {
    let public = public_tool_name("my-server", "echo");
    assert_eq!(public, "mcp__my-server__echo");
    let (server, raw) = BridgeTool::parse_public_name(&public).unwrap();
    assert_eq!(server, "my-server");
    assert_eq!(raw, "echo");
    assert!(BridgeTool::parse_public_name("not-mcp").is_none());
    assert!(BridgeTool::parse_public_name("mcp__only-server").is_none());
}

#[test]
fn empty_set_reports_no_servers() {
    let set = McpBridgeToolSet::new(vec![]);
    assert!(set.is_empty());
    assert!(set.server_names().is_empty());
}

#[test]
fn names_are_sorted() {
    let set = McpBridgeToolSet::new(vec![stdio_server("b"), stdio_server("a")]);
    assert_eq!(set.server_names(), vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn bridge_tool_definition_uses_namespace_name() {
    let provider = Arc::new(crate::mcp_resources::client::McpClientProvider::new(stdio_server("srv")));
    let tool = BridgeTool {
        server: "srv".to_string(),
        provider,
        raw_name: "greet".to_string(),
        description: "Say hi".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": { "name": { "type": "string" } },
            "required": ["name"]
        }),
    };
    let def = tool.definition();
    assert_eq!(def.name, "mcp__srv__greet");
    assert!(def.description.contains("MCP srv"));
    assert!(def.parameters.get("properties").is_some());
}

/// 构造一段原始发现结果（re-sync fake fetch 用）。
fn raw_tool(name: &str) -> RawTool {
    RawTool {
        raw_name: name.to_string(),
        description: format!("{name} tool"),
        input_schema: json!({ "type": "object", "properties": {} }),
    }
}

#[test]
fn resync_swaps_tools_and_survives_fetch_failure() {
    let set = McpBridgeToolSet::new(vec![stdio_server("srv")]);
    let registry = ToolRegistry::new();

    // 初次发现 [echo]：现网从空换入 echo，白名单同步并入。
    let (old, added) = set
        .resync_with("srv", &registry, |_provider| Ok(vec![raw_tool("echo")]))
        .unwrap();
    assert!(old.is_empty());
    assert_eq!(added, vec!["mcp__srv__echo".to_string()]);
    assert_eq!(
        registry.state().agent_tools(),
        ["mcp__srv__echo".to_string()].into_iter().collect()
    );

    // 二次发现 [greet]：echo 整体注销、greet 并入，白名单同步增删。
    let (old, added) = set
        .resync_with("srv", &registry, |_provider| Ok(vec![raw_tool("greet")]))
        .unwrap();
    assert_eq!(old, vec!["mcp__srv__echo".to_string()]);
    assert_eq!(added, vec!["mcp__srv__greet".to_string()]);
    assert_eq!(
        registry.state().agent_tools(),
        ["mcp__srv__greet".to_string()].into_iter().collect()
    );
    assert_eq!(
        registry.risk_map().keys().cloned().collect::<Vec<_>>(),
        vec!["mcp__srv__greet".to_string()]
    );

    // 发现失败：现网与白名单原封不动（all-or-nothing）。
    let error =
        set.resync_with("srv", &registry, |_provider| Err("boom".to_string())).unwrap_err();
    assert_eq!(error, "boom");
    assert_eq!(
        registry.state().agent_tools(),
        ["mcp__srv__greet".to_string()].into_iter().collect()
    );
    let view = set.view("srv").unwrap();
    assert!(matches!(view.state, McpServerState::Registered { tool_count: 1 }));
    assert_eq!(view.tools, vec!["mcp__srv__greet".to_string()]);
}

#[test]
fn resync_unknown_server_reports_error_without_io() {
    let set = McpBridgeToolSet::new(vec![stdio_server("srv")]);
    let registry = ToolRegistry::new();
    let error = set.resync("other", &registry).unwrap_err();
    assert!(error.contains("unknown MCP server"), "unexpected: {error}");
    assert!(set.view("other").is_none());
}

#[test]
fn registered_tool_names_aggregate_across_servers() {
    let set = McpBridgeToolSet::new(vec![stdio_server("a"), stdio_server("b")]);
    let registry = ToolRegistry::new();
    set.resync_with("a", &registry, |_provider| Ok(vec![raw_tool("echo")])).unwrap();
    set.resync_with("b", &registry, |_provider| Ok(vec![raw_tool("ping")])).unwrap();
    let mut names = set.registered_tool_names();
    names.sort();
    assert_eq!(names, vec!["mcp__a__echo".to_string(), "mcp__b__ping".to_string()]);
}

#[test]
fn reconcile_classifies_added_unchanged_rebuilt_removed() {
    let set = McpBridgeToolSet::new(vec![stdio_server("keep"), stdio_server("drop")]);
    let registry = ToolRegistry::new();
    let resources = McpResourceService::new();
    // 装配期 provider 由 new() 建立，但资源服务需按同一份配置注册（模拟装配侧）。
    for name in ["keep", "drop"] {
        resources
            .register(name, Arc::new(McpClientProvider::new(stdio_server(name))))
            .unwrap();
    }
    // 先给 drop 造一份已注册工具面，验证卸载会摘除。
    set.resync_with("drop", &registry, |_provider| Ok(vec![raw_tool("gone")])).unwrap();
    assert_eq!(registry.state().agent_tools(), ["mcp__drop__gone".to_string()].into_iter().collect());

    // 期望集：keep 不变、drop 消失、fresh 新增。
    let report = set.reconcile_with(
        vec![stdio_server("keep"), stdio_server("fresh")],
        &registry,
        &resources,
        |_provider| Ok(vec![raw_tool("fresh_tool")]),
    );

    assert_eq!(report.unchanged, vec!["keep".to_string()]);
    assert_eq!(report.removed, vec!["drop".to_string()]);
    assert_eq!(report.added, vec!["fresh".to_string()]);
    assert!(report.rebuilt.is_empty());
    // drop 的工具面与白名单已摘除，provider 与资源 provider 均已注销。
    assert!(registry.risk_map().get("mcp__drop__gone").is_none());
    assert_eq!(
        registry.state().agent_tools(),
        ["mcp__fresh__fresh_tool".to_string()].into_iter().collect()
    );
    assert!(set.view("drop").is_none());
    assert!(!resources.server_names().contains(&"drop".to_string()));
    // fresh 已挂载并注册工具。
    let view = set.view("fresh").unwrap();
    assert!(matches!(view.state, McpServerState::Registered { tool_count: 1 }));
    assert_eq!(view.tools, vec!["mcp__fresh__fresh_tool".to_string()]);
    assert!(resources.server_names().contains(&"fresh".to_string()));
}

#[test]
fn reconcile_rebuilds_server_whose_config_changed() {
    let set = McpBridgeToolSet::new(vec![stdio_server("srv")]);
    let registry = ToolRegistry::new();
    let resources = McpResourceService::new();
    resources
        .register("srv", Arc::new(McpClientProvider::new(stdio_server("srv"))))
        .unwrap();

    // 同名但 command 变更 → 重建（先卸载再挂载）。
    let mut changed = stdio_server("srv");
    if let McpServerConfig::Stdio { command, .. } = &mut changed {
        *command = "node-changed".to_string();
    }
    let report = set.reconcile_with(vec![changed], &registry, &resources, |_provider| {
        Ok(vec![raw_tool("echo")])
    });

    assert_eq!(report.rebuilt, vec!["srv".to_string()]);
    assert!(report.added.is_empty());
    assert!(report.removed.is_empty());
    assert!(report.unchanged.is_empty());
    assert!(set.view("srv").is_some());
    assert!(resources.server_names().contains(&"srv".to_string()));
}
