//! `manager` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::{probe, truncate_tool_names, McpProbeStage};
use crate::mcp_resources::client::McpServerConfig;

/// 命令必然不存在的 stdio 配置（spawn 失败 → connect 阶段归因）。
fn missing_command_server(name: &str) -> McpServerConfig {
    McpServerConfig::Stdio {
        server_name: name.to_string(),
        command: "onion-missing-probe-command".to_string(),
        args: vec![],
        env: Default::default(),
        cwd: None,
        tool_call_timeout_ms: 1000,
    }
}

#[test]
fn probe_attributes_connect_failure_for_missing_command() {
    let result = probe(&missing_command_server("ghost"));
    assert!(!result.ok);
    assert_eq!(result.stage, Some(McpProbeStage::Connect));
    assert!(result.message.is_some());
    assert!(result.tool_names.is_empty());
}

#[test]
fn truncate_tool_names_caps_at_budget() {
    let names: Vec<String> = (0..600).map(|index| format!("tool-{index}")).collect();
    let truncated = truncate_tool_names(names.clone());
    assert_eq!(truncated.len(), 512);
    assert_eq!(truncated, names[..512].to_vec());
    assert!(truncate_tool_names(Vec::new()).is_empty());
}
