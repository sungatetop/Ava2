//! MCP 运行期管理面。
//!
//! 本 crate 为 spawn-per-call
//! （每次操作一次性连接，无长驻连接 / 无重连），连接监督与退避重连不适用，本模块
//! 提供可平移的子集：
//! - [`probe`]：一次性 connect → `tools/list` 探活，失败按阶段归因（失败为返回值
//!   而非异常，`{ok, stage, message, durationMs}`）；
//! - [`McpServerState`]：server 工具面状态聚合（已注册 / 发现失败 / 未发现三态）；
//! - [`McpServerView`]：单 server 运行期视图（REST 呈现）。
//!
//! `restart` 在 onion 无独立语义（无实例可重建），由 `McpBridgeToolSet::resync`
//! （重发现 + 工具面整代替换）承担；`reconcile`（配置变更收敛）留待后续阶段。

use std::time::Instant;

use super::client::{McpClientProvider, McpServerConfig};

/// probe 返回的工具名数量预算（缺省 512，超出截断）。
const PROBE_MAX_TOOL_NAMES: usize = 512;

/// probe 失败的阶段归因（`connect` / `list`）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum McpProbeStage {
    /// 连接阶段失败（凭据解析 / spawn / initialize 握手）。
    Connect,
    /// 连接成功但 `tools/list` 失败。
    List,
}

impl McpProbeStage {
    /// 呈现用小写标识。
    pub fn as_str(&self) -> &'static str {
        match self {
            McpProbeStage::Connect => "connect",
            McpProbeStage::List => "list",
        }
    }
}

/// probe 结果（失败为返回值，不抛异常；探活独立于 live 实例，
/// 不注册工具、不占命名空间）。
#[derive(Clone, Debug)]
pub struct McpProbeResult {
    /// 探活是否成功。
    pub ok: bool,
    /// 失败阶段归因；成功时无意义（`None`）。
    pub stage: Option<McpProbeStage>,
    /// 失败信息；成功时为空。
    pub message: Option<String>,
    /// 探活耗时（毫秒）。
    pub duration_ms: u64,
    /// 发现的工具名（原始名，超出 [`PROBE_MAX_TOOL_NAMES`] 截断）。
    pub tool_names: Vec<String>,
}

/// server 工具面运行期状态（三态子集；
/// spawn-per-call 无长驻连接即无 connecting / reconnecting 等中间态）。
#[derive(Clone, Debug)]
pub enum McpServerState {
    /// `tools/list` 成功并已注册 N 个桥接工具。
    Registered { tool_count: usize },
    /// `tools/list` 失败（server 未启动 / 协议错误），工具面缺失。
    DiscoveryFailed { error: String },
    /// 尚未执行过工具发现（预留 reconcile 阶段）。
    Pending,
}

/// 单 server 运行期视图（REST `GET /v1/mcp/servers` 与诊断呈现）。
#[derive(Clone, Debug)]
pub struct McpServerView {
    /// server 名。
    pub name: String,
    /// transport 类型（`stdio` | `streamable-http`）。
    pub transport: &'static str,
    /// 工具面状态。
    pub state: McpServerState,
    /// 当前注册的桥接工具公开名（`mcp__<server>__<raw>`；失败 / 未发现 → 空）。
    pub tools: Vec<String>,
}

/// 对一个已配置 server 做一次性探活：connect → `tools/list`，失败按阶段归因。
///
/// 每次探活建立独立一次性连接（onion spawn-per-call 语义，与装配期发现一致）；
/// 探活是纯诊断操作，不改动任何运行期状态。
pub fn probe(config: &McpServerConfig) -> McpProbeResult {
    let started = Instant::now();
    let provider = McpClientProvider::new(config.clone());
    // 外层 Err = connect 阶段；内层 Err = list 阶段（双层 Result 由
    // `McpClientProvider::probe_tools` 归因）。
    let outcome = provider.probe_tools();
    let duration_ms = started.elapsed().as_millis() as u64;
    match outcome {
        Err(error) => McpProbeResult {
            ok: false,
            stage: Some(McpProbeStage::Connect),
            message: Some(error.to_string()),
            duration_ms,
            tool_names: Vec::new(),
        },
        Ok(Err(message)) => McpProbeResult {
            ok: false,
            stage: Some(McpProbeStage::List),
            message: Some(message),
            duration_ms,
            tool_names: Vec::new(),
        },
        Ok(Ok(names)) => McpProbeResult {
            ok: true,
            stage: None,
            message: None,
            duration_ms,
            tool_names: truncate_tool_names(names),
        },
    }
}

/// 超出预算截断（只影响呈现，不影响注册）。
fn truncate_tool_names(mut names: Vec<String>) -> Vec<String> {
    names.truncate(PROBE_MAX_TOOL_NAMES);
    names
}

#[cfg(test)]
#[path = "manager_tests.rs"]
mod tests;
