//! MCP 资源工具（资源 seam + 客户端 + 工具桥接 + 管理面）。
//!
//! - `seam.rs`：可注册 provider 的服务 + 3 个共享资源工具壳（`list_mcp_resources` /
//!   `list_mcp_resource_templates` / `read_mcp_resource`）；
//! - `client.rs`：MCP 客户端（stdio / streamable-http 两种 transport），server 由
//!   配置文件描述；
//! - `bridge.rs`：MCP 工具桥接（`tools/list` 动态发现 → `mcp__<server>__<tool>` 命名空间
//!   注册 → `tools/call` 转发）；
//! - `manager.rs`：运行期管理面子集（probe 探活按阶段归因 + server 工具面状态聚合 +
//!   视图）。
//!
//! 只读、非 deferred。缺省无配置 server → 结构化 `MCP_UNAVAILABLE`。

pub mod bridge;
pub mod client;
pub mod manager;
pub mod seam;

pub use bridge::{BridgeTool, McpBridgeToolSet, McpReconcileReport};
pub use client::{McpClientProvider, McpServerConfig, discover_servers, load_servers, parse_servers};
pub use manager::{McpProbeResult, McpProbeStage, McpServerState, McpServerView, probe};
pub use seam::{
    McpResourceError, McpResourceErrorCode, McpResourceRequest, McpResourceService,
    McpResourceToolSet, render_resource_result,
};
