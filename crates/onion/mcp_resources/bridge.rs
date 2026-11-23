//! MCP 工具桥接（`syncTools` 两阶段 + 动态发现）。
//!
//! 连接后调 `client.listTools()` 动态发现 server 暴露的工具，把每个原始工具
//! 桥接成模型可见工具，命名空间化为 `mcp__<serverName>__<rawName>` 并注册进
//! ToolRuntime。本 crate 为教学子集：装配期对每个已配置 server 调一次 `tools/list`
//! （临时连接），把返回的工具注册为同命名空间的 [`ToolPort`]；模型调用时经
//! `tools/call` 转发到对应 server。
//!
//! 与资源工具（`mcp_resources`）的区别：资源工具是固定的 3 个（list/templates/read），
//! 而桥接工具是 server 暴露的**任意**工具，动态发现、动态注册。
//!
//! 运行期管理面：本集合跟踪各
//! server 已注册的精确工具名单与发现状态，[`McpBridgeToolSet::resync`] 支持运行期
//! 重发现并整批原子替换工具面（无长驻连接故无 list_changed 通知，由管理端点
//! 显式触发；重发现即等效重生）。

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::{json, Value};

use crate::tools::ToolPort;
use crate::types::{ContentBlock, ToolCall, ToolDefinition};
use crate::waterfall::BoxFuture;
use tokio_util::sync::CancellationToken;

use super::client::{McpClientProvider, McpServerConfig};
use super::manager::{McpServerState, McpServerView};
use super::seam::McpResourceService;

/// 桥接工具命名空间前缀（`mcp__<serverName>__<rawName>`）。
const NAMESPACE_SEP: &str = "__";
const NAMESPACE_PREFIX: &str = "mcp__";

/// 一次配置收敛（reconcile）的结果（挂载/卸载语义）。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct McpReconcileReport {
    /// 新增挂载的 server（此前不存在）。
    pub added: Vec<String>,
    /// 配置变更后重建的 server（先卸载再挂载）。
    pub rebuilt: Vec<String>,
    /// 配置消失后卸载的 server。
    pub removed: Vec<String>,
    /// 配置未变、保持原状的 server。
    pub unchanged: Vec<String>,
    /// 挂载失败等说明性信息（不中断其余 server 的收敛）。
    pub notes: Vec<String>,
}

/// 一个已配置 server 的桥接工具集：持有 provider，按原始工具名注册命名空间工具，
/// 并跟踪各 server 的运行期状态（管理面 re-sync / 状态聚合的基础）。
pub struct McpBridgeToolSet {
    /// server 名 → 连接 provider（运行期可增删，故加锁）。
    providers: Mutex<HashMap<String, Arc<McpClientProvider>>>,
    /// server 名 → 装配/收敛时的配置快照（变更检测依据）。
    configs: Mutex<HashMap<String, McpServerConfig>>,
    /// server 名 → 已注册进 registry 的公开工具名（精确名注销依据：server 名含
    /// `__` 时按 `mcp__<server>__` 前缀匹配会跨 server 碰撞，故按名单整批替换
    /// 而非前缀注销）。
    registered: Mutex<HashMap<String, Vec<String>>>,
    /// server 名 → 最近一次工具发现状态（三态聚合，REST 呈现）。
    states: Mutex<HashMap<String, McpServerState>>,
}

impl McpBridgeToolSet {
    /// 由装配期发现的 server 列表构造（各 server 初始为 [`McpServerState::Pending`]）。
    pub fn new(servers: Vec<McpServerConfig>) -> Self {
        let mut providers = HashMap::new();
        let mut configs = HashMap::new();
        let mut states = HashMap::new();
        for config in servers {
            let name = config.server_name().to_string();
            states.insert(name.clone(), McpServerState::Pending);
            providers.insert(name.clone(), Arc::new(McpClientProvider::new(config.clone())));
            configs.insert(name, config);
        }
        Self {
            providers: Mutex::new(providers),
            configs: Mutex::new(configs),
            registered: Mutex::new(HashMap::new()),
            states: Mutex::new(states),
        }
    }

    /// 是否配置了任何 server（供装配 / 冒烟诊断）。
    pub fn is_empty(&self) -> bool {
        self.providers.lock().unwrap_or_else(PoisonError::into_inner).is_empty()
    }

    /// 已配置 server 名列表（排序，供 prompt 段与诊断）。
    pub fn server_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .providers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .keys()
            .cloned()
            .collect();
        names.sort();
        names
    }

    /// 当前运行期配置快照（排序，供 REST 状态视图；reconcile 后与装配期快照可能不同）。
    pub fn configs(&self) -> Vec<McpServerConfig> {
        let mut configs: Vec<McpServerConfig> = self
            .configs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
            .cloned()
            .collect();
        configs.sort_by(|a, b| a.server_name().cmp(b.server_name()));
        configs
    }

    /// 装配期对每个 server 调 `tools/list` 动态发现工具并注册为命名空间工具。
    ///
    /// 单个 server 的发现失败（未启动 / 协议错误）不 panic：状态记为
    /// [`McpServerState::DiscoveryFailed`]，错误记入 `notes` 呈现，工具面缺失；
    /// 成功发现的工具注册进 registry 并记录精确名单与状态。返回
    /// `(注册的公开工具名, 说明性 notes)`：工具名为动态发现结果，无法写进 Agent
    /// 预设清单，须由装配侧并入工具面白名单。
    pub fn register_all(
        &self,
        registry: &crate::tools::ToolRegistry,
    ) -> (Vec<String>, Vec<String>) {
        let mut notes = Vec::new();
        let mut names = Vec::new();
        let providers: Vec<(String, Arc<McpClientProvider>)> = self
            .providers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|(name, provider)| (name.clone(), provider.clone()))
            .collect();
        for (server, provider) in &providers {
            match fetch_raw_tools(provider) {
                Err(error) => {
                    self.set_state(
                        server,
                        McpServerState::DiscoveryFailed { error: error.clone() },
                    );
                    notes.push(format!("MCP server \"{server}\" 工具发现失败：{error}"));
                }
                Ok(raw) => {
                    let tools = build_bridge_tools(server, provider, raw);
                    for tool in &tools {
                        registry.register(tool.clone());
                    }
                    let registered: Vec<String> =
                        tools.iter().map(|tool| tool.definition().name).collect();
                    self.registered
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .insert(server.clone(), registered.clone());
                    self.set_state(
                        server,
                        McpServerState::Registered { tool_count: registered.len() },
                    );
                    names.extend(registered);
                }
            }
        }
        (names, notes)
    }

    /// 单 server 运行期视图（状态 + 当前注册的公开工具名）；server 未配置 → `None`。
    pub fn view(&self, name: &str) -> Option<McpServerView> {
        let provider = self
            .providers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(name)
            .cloned()?;
        let state = self
            .states
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(name)
            .cloned()
            .unwrap_or(McpServerState::Pending);
        Some(McpServerView {
            name: name.to_string(),
            transport: provider.config().transport_kind(),
            state,
            tools: self.registered_names(name),
        })
    }

    /// 运行期重同步单个 server 的工具面（管理端点显式触发）。
    ///
    /// 两阶段 all-or-nothing：
    /// Phase 1 在锁外重新发现（慢 IO），失败 → `Err`，现网工具面原封不动；
    /// Phase 2 经 [`crate::tools::ToolRegistry::replace`] 在单写锁内原子完成
    /// 「注销旧名 + 注册新代」，并同步增删 Agent 工具面白名单与状态记录。
    ///
    /// 返回 `(注销的公开工具名, 注册的公开工具名)`。
    pub fn resync(
        &self,
        server: &str,
        registry: &crate::tools::ToolRegistry,
    ) -> Result<(Vec<String>, Vec<String>), String> {
        self.resync_with(server, registry, fetch_raw_tools)
    }

    /// [`Self::resync`] 的可注入形态：测试以 fake fetch 验证两阶段语义，生产路径
    /// 传入 [`fetch_raw_tools`]。
    fn resync_with<F>(
        &self,
        server: &str,
        registry: &crate::tools::ToolRegistry,
        fetch: F,
    ) -> Result<(Vec<String>, Vec<String>), String>
    where
        F: FnOnce(&McpClientProvider) -> Result<Vec<RawTool>, String>,
    {
        let Some(provider) = self
            .providers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(server)
            .cloned()
        else {
            return Err(format!("unknown MCP server \"{server}\""));
        };
        // Phase 1：锁外重新发现；失败则现网不动（all-or-nothing）。
        let raw = fetch(&provider)?;
        let add = build_bridge_tools(server, &provider, raw);
        let add_names: Vec<String> = add.iter().map(|tool| tool.definition().name).collect();
        let old_names = self.registered_names(server);
        // Phase 2：单写锁内原子换装 + 白名单与状态同步（失效名移出、新名并入）。
        registry.replace(&old_names, add);
        registry.state().remove_agent_tools(old_names.iter().cloned());
        registry.state().add_agent_tools(add_names.iter().cloned());
        self.registered
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(server.to_string(), add_names.clone());
        self.set_state(server, McpServerState::Registered { tool_count: add_names.len() });
        Ok((old_names, add_names))
    }

    /// 配置收敛（reconcile）：把运行期实例集对齐到期望配置集。
    ///
    /// 逐 server 判定（JSON 深比较做变更检测）：
    /// - 期望集中不存在 → 卸载（注销 provider + 摘除工具 + 清状态）；
    /// - 配置变更 → 重建（先卸载再挂载）；
    /// - 新增 → 挂载；
    /// - 配置未变 → 保持原状（不重连、不重发现）。
    ///
    /// 挂载 = 建 provider + 注册进资源服务 + 发现工具并注册（失败仅记状态与 note，
    /// 不中断其余 server）。返回收敛报告。
    pub fn reconcile(
        &self,
        desired: Vec<McpServerConfig>,
        registry: &crate::tools::ToolRegistry,
        resources: &McpResourceService,
    ) -> McpReconcileReport {
        self.reconcile_with(desired, registry, resources, fetch_raw_tools)
    }

    /// [`Self::reconcile`] 的可注入形态：测试以 fake fetch 验证收敛语义，生产路径
    /// 传入 [`fetch_raw_tools`]。
    fn reconcile_with<F>(
        &self,
        desired: Vec<McpServerConfig>,
        registry: &crate::tools::ToolRegistry,
        resources: &McpResourceService,
        fetch: F,
    ) -> McpReconcileReport
    where
        F: Fn(&McpClientProvider) -> Result<Vec<RawTool>, String>,
    {
        let mut report = McpReconcileReport::default();

        // 阶段一：卸载消失的与配置变更的（先卸载再挂载）。
        let current: Vec<(String, McpServerConfig)> = self
            .configs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|(name, config)| (name.clone(), config.clone()))
            .collect();
        for (name, snapshot) in current {
            let next = desired.iter().find(|config| config.server_name() == name);
            match next {
                None => {
                    self.unmount(&name, registry, resources);
                    report.removed.push(name);
                }
                Some(config) if *config != snapshot => {
                    self.unmount(&name, registry, resources);
                    report.rebuilt.push(name);
                }
                Some(_) => report.unchanged.push(name),
            }
        }

        // 阶段二：挂载新增的与刚重建的。
        for config in desired {
            let name = config.server_name().to_string();
            if report.unchanged.contains(&name) {
                continue;
            }
            let provider = Arc::new(McpClientProvider::new(config.clone()));
            if let Err(error) = resources.register(&name, provider.clone()) {
                report.notes.push(format!("MCP server \"{name}\" 资源 provider 注册失败：{error}"));
                continue;
            }
            self.providers
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(name.clone(), provider.clone());
            self.configs
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(name.clone(), config);
            self.set_state(&name, McpServerState::Pending);
            match fetch(&provider) {
                Err(error) => {
                    self.set_state(&name, McpServerState::DiscoveryFailed { error: error.clone() });
                    report.notes.push(format!("MCP server \"{name}\" 工具发现失败：{error}"));
                }
                Ok(raw) => {
                    let tools = build_bridge_tools(&name, &provider, raw);
                    let names: Vec<String> =
                        tools.iter().map(|tool| tool.definition().name).collect();
                    for tool in tools {
                        registry.register(tool);
                    }
                    registry.state().add_agent_tools(names.iter().cloned());
                    self.registered
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .insert(name.clone(), names.clone());
                    self.set_state(&name, McpServerState::Registered { tool_count: names.len() });
                }
            }
            if !report.rebuilt.contains(&name) {
                report.added.push(name);
            }
        }
        report
    }

    /// 卸载单 server：摘除已注册工具与白名单、注销资源 provider、清状态与配置快照。
    fn unmount(
        &self,
        server: &str,
        registry: &crate::tools::ToolRegistry,
        resources: &McpResourceService,
    ) {
        let old_names = self.registered_names(server);
        if !old_names.is_empty() {
            registry.replace(&old_names, Vec::new());
            registry.state().remove_agent_tools(old_names.iter().cloned());
        }
        self.registered.lock().unwrap_or_else(PoisonError::into_inner).remove(server);
        self.states.lock().unwrap_or_else(PoisonError::into_inner).remove(server);
        self.configs.lock().unwrap_or_else(PoisonError::into_inner).remove(server);
        self.providers.lock().unwrap_or_else(PoisonError::into_inner).remove(server);
        resources.unregister(server);
    }

    /// 某 server 当前已注册的公开工具名（无记录 → 空表）。
    fn registered_names(&self, server: &str) -> Vec<String> {
        self.registered
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(server)
            .cloned()
            .unwrap_or_default()
    }

    /// 全部 server 已注册桥接工具的公开名汇总（`/agent` 切换重设白名单用，
    /// 替代原先的 `mcp_tools` 名单快照）。
    pub fn registered_tool_names(&self) -> Vec<String> {
        self.registered
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
            .flatten()
            .cloned()
            .collect()
    }

    /// 更新单 server 发现状态（内部助手，统一锁获取方式）。
    fn set_state(&self, server: &str, state: McpServerState) {
        self.states
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(server.to_string(), state);
    }
}

/// `tools/list` 原始发现结果（未命名空间化）。
struct RawTool {
    raw_name: String,
    description: String,
    input_schema: Value,
}

/// 对单个 server 执行 `tools/list` 并解析为原始工具列表（发现 / re-sync Phase 1：
/// 锁外慢 IO，失败时现网不动）。
fn fetch_raw_tools(provider: &McpClientProvider) -> Result<Vec<RawTool>, String> {
    let value = provider.list_tools().map_err(|e| e.to_string())?;
    let list = value
        .get("tools")
        .and_then(Value::as_array)
        .ok_or_else(|| "tools/list 响应缺 tools 数组".to_string())?;
    Ok(list
        .iter()
        .filter_map(|tool| {
            let raw_name = tool.get("name").and_then(Value::as_str)?.to_string();
            let description =
                tool.get("description").and_then(Value::as_str).unwrap_or("").to_string();
            let input_schema = tool
                .get("inputSchema")
                .cloned()
                .unwrap_or_else(|| json!({ "type": "object", "properties": {} }));
            Some(RawTool { raw_name, description, input_schema })
        })
        .collect())
}

/// 把原始工具列表构造为命名空间桥接工具（注册 / re-sync Phase 2 的提交物）。
fn build_bridge_tools(
    server: &str,
    provider: &Arc<McpClientProvider>,
    raw: Vec<RawTool>,
) -> Vec<Arc<dyn ToolPort>> {
    raw.into_iter()
        .map(|tool| {
            Arc::new(BridgeTool {
                server: server.to_string(),
                provider: provider.clone(),
                raw_name: tool.raw_name,
                description: tool.description,
                input_schema: tool.input_schema,
            }) as Arc<dyn ToolPort>
        })
        .collect()
}

/// 派生模型面向的桥接工具公开名：`mcp__<serverName>__<rawName>`。
fn public_tool_name(server: &str, raw: &str) -> String {
    format!("{NAMESPACE_PREFIX}{server}{NAMESPACE_SEP}{raw}")
}

/// 一个已注册的桥接工具：模型调用 `mcp__<server>__<raw>` 时转发 `tools/call`。
pub struct BridgeTool {
    server: String,
    provider: Arc<McpClientProvider>,
    raw_name: String,
    description: String,
    input_schema: Value,
}

impl BridgeTool {
    /// 从公开名解析 server 名与原始工具名（供 execute 路由）。未命中返回 None。
    pub fn parse_public_name(public: &str) -> Option<(String, String)> {
        let rest = public.strip_prefix(NAMESPACE_PREFIX)?;
        let (server, raw) = rest.split_once(NAMESPACE_SEP)?;
        Some((server.to_string(), raw.to_string()))
    }
}

impl ToolPort for BridgeTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: public_tool_name(&self.server, &self.raw_name),
            description: format!("[MCP {}] {}", self.server, self.description),
            parameters: self.input_schema.clone(),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let provider = self.provider.clone();
        let raw_name = self.raw_name.clone();
        let server = self.server.clone();
        Box::pin(async move {
            let arguments = call.input.clone();
            provider
                .call_tool(&raw_name, arguments)
                .map(|value| {
                    vec![ContentBlock::Text { text: render_call_result(&server, &raw_name, &value) }]
                })
                .map_err(|e| e.to_string())
        })
    }
}

/// 渲染工具调用结果：`MCP tool <server>/<raw>` 前缀 + JSON；`blob` 字段脱敏。
fn render_call_result(server: &str, raw: &str, value: &Value) -> String {
    let mut value = value.clone();
    redact_blobs(&mut value);
    format!(
        "MCP tool {server}/{raw}\n{}",
        serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string())
    )
}

/// 递归将字符串型 `blob` 字段替换为二进制占位描述（对齐资源工具语义）。
fn redact_blobs(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, v) in map.iter_mut() {
                if key == "blob" {
                    if let Value::String(s) = v {
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
        Value::Array(arr) => {
            for v in arr.iter_mut() {
                redact_blobs(v);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
#[path = "bridge_tests.rs"]
mod tests;
