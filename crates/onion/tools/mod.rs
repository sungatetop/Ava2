//! ToolPort trait + 工具注册表（Agent 工具面过滤 / deferred 门禁 / search）。
//!
//! 模块拆分：`basic.rs` 沙箱文件工具（read / read_image / write）、
//! `edit.rs` 编辑工具（edit / str_replace_editor）、`search.rs` tool_search
//! （deferred 工具检索 + 解锁）、`subagent.rs` 子代理五件套。
//! 安全模型三层防线中本模块承载第一层（Agent 工具面准入）与 deferred 下发契约
//! （见 docs/design/onion-tools-design.md）。
//!
//! - `definitions()` 按当前 Agent 工具名集合过滤；deferred 工具投影为 stub
//!   （空 parameters + 提示后缀）——延迟只发生在「下发」侧，执行侧工具恒定在线；
//! - `execute()` 统一治理取消/超时，并校验 Agent 面与 deferred 解锁门禁；
//!   未知工具 / 面外 / 未解锁 / 取消 / 超时 / 端口错误均合成 is_error ToolResult，
//!   不中断 step（tool_use / tool_result 配对不变式维持）。
//!
//! 命名沿用 `tool-fs`：`read` / `read_image` / `write`。
//!
//! 2026-10-20：工具面由「工作模式（chat/work）」改为「Agent
//! 工具名集合（coding/work + 自定义）」过滤；`tool_search` 与 deferred 执行路径
//! **代码保留但不使用**——工具默认全部 `defer_loading() == false` 完整披露，
//! 不做 stub→完整解锁（连接即全量装载）。

mod a2a;
mod basic;
mod edit;
mod glob_grep;
pub mod jobs;
pub mod office;
mod patch;
mod patch_tool;
pub mod subagent;
mod search;
mod shell;
pub mod workflow;

pub use a2a::{A2aApproveTool, A2aCallTool, A2aSendTool, DelegationLink, links_path, read_links};
pub use basic::{ReadImageTool, ReadTool, WriteTool};
pub use edit::{EditTool, StrReplaceEditorTool};
pub use glob_grep::{GlobTool, GrepTool};
pub use office::{ConvertTool, RecalculateTool, RenderTool};
pub use patch_tool::ApplyPatchTool;
pub use jobs::{JobKillTool, JobListTool, JobOutputTool};
pub use search::ToolSearchTool;
pub use shell::{BashTool, PwshTool, SandboxEscalator};
pub use crate::todo::TodoWriteTool;
pub use crate::web::{WebFetchTool, WebSearchTool};
pub use crate::present::PresentTool;
pub use crate::schedule::tools::ScheduleToolSet;
pub use crate::skill::tools::SkillTool;
pub use crate::working_dir::WorkingDirectoryTool;
pub use workflow::WorkflowTool;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use serde_json::json;
use tokio_util::sync::CancellationToken;

use crate::approval::ToolRisk;
use crate::types::{ContentBlock, ToolCall, ToolDefinition, ToolResult};
use crate::waterfall::BoxFuture;

/// 工具端口：定义（下发模型）+ 执行（cancel / 超时由注册表统一包裹）。
/// 自描述扩展（缺省实现保证既有工具零改动）：
/// - `modes()`：已废弃（不再参与工具面过滤；保留以兼容历史自描述）。
/// - `defer_loading()`：deferred 工具——当前全部返回 `false`（机制保留不使用）。
pub trait ToolPort: Send + Sync {
    fn definition(&self) -> ToolDefinition;
    fn execute(&self, call: ToolCall, cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>>;
    fn modes(&self) -> Option<Vec<String>> {
        None
    }
    fn defer_loading(&self) -> bool {
        false
    }
    /// 工具风险等级（审批判定元数据，默认只读）：仅会写文件系统 / 派生子会话的
    /// 工具覆写。风险是事实描述而非策略——审批行为由部署侧沙箱与审批策略派生
    /// （`crate::approval::decide`）。
    fn risk(&self) -> ToolRisk {
        ToolRisk::ReadOnly
    }
    /// 阻断型工具（交互工具省略 `timeoutMs` = 无 deadline）：
    /// 执行不参与注册表的统一 tool_timeout，默认阻塞直至取消/宿主回填。
    /// 典型：`exit_plan_mode`（plan-review 等待宿主复决）。
    fn blocking(&self) -> bool {
        false
    }
    /// 按调用内容分类风险等级（「按操作边界判定」而非整体工具名）：
    /// 缺省退回静态 [`Self::risk`]；需要按参数细分的工具（如 shell 读/写命令）
    /// 覆写此方法。审批判定经 [`ToolRegistry::classify`] 取用。
    fn classify_risk(&self, _call: &ToolCall) -> ToolRisk {
        self.risk()
    }
}

/// 注册表共享状态：当前 Agent 工具名集合 + 已 search 解锁的 deferred 工具（会话级）。
/// Arc 共享给 registry 与 tool_search；Agent 面与解锁对执行侧实时生效。
///
/// 2026-10-20：`mode`（chat/work）改为 `agent_tools`（Agent 工具名集合，白名单过滤）。
/// 工具面由「Agent 预设工具名列表」决定，`tool_search`/deferred 保留不使用。
pub struct ToolState {
    agent_tools: RwLock<HashSet<String>>,
    unlocked: Mutex<HashSet<String>>,
}

impl Default for ToolState {
    fn default() -> Self {
        Self { agent_tools: RwLock::new(HashSet::new()), unlocked: Mutex::new(HashSet::new()) }
    }
}

impl ToolState {
    /// 当前 Agent 工具名集合（空 = 未装配，工具面为空）。
    pub fn agent_tools(&self) -> HashSet<String> {
        self.agent_tools.read().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
    }

    /// 设置当前 Agent 工具名集合（装配期 / `/agent` 运行时切换；白名单过滤）。
    pub fn set_agent(&self, tools: impl IntoIterator<Item = String>) {
        *self.agent_tools.write().unwrap_or_else(std::sync::PoisonError::into_inner) =
            tools.into_iter().collect();
    }

    /// 向当前 Agent 工具面增量加入工具名（MCP 桥接 re-sync 注册新代后调用；
    /// 与 set_agent 的整体替换互补，运行期动态扩面）。
    pub fn add_agent_tools(&self, tools: impl IntoIterator<Item = String>) {
        let mut agent = self.agent_tools.write().unwrap_or_else(std::sync::PoisonError::into_inner);
        agent.extend(tools);
    }

    /// 从当前 Agent 工具面移除工具名（MCP 桥接 re-sync 注销旧代后调用；
    /// 不在面内的名字静默忽略）。
    pub fn remove_agent_tools(&self, tools: impl IntoIterator<Item = String>) {
        let mut agent = self.agent_tools.write().unwrap_or_else(std::sync::PoisonError::into_inner);
        for name in tools {
            agent.remove(&name);
        }
    }

    fn is_unlocked(&self, name: &str) -> bool {
        self.unlocked.lock().unwrap_or_else(std::sync::PoisonError::into_inner).contains(name)
    }

    fn unlock(&self, name: &str) {
        self.unlocked.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(name.to_string());
    }
}

/// 工具注册表：定义集合 + 按名分发执行（模式过滤、deferred 门禁、取消与超时统一治理）。
/// Clone 与 `Default` 共享同一份工具集合（`Arc<RwLock<…>>`）——装配后克隆句柄
/// （如 SessionHandle 的 `tools_handle`）上的运行期注册/注销对执行侧实时生效，
/// 这是 MCP 桥接 re-sync 的基础；`ToolState` 同理共享（/agent 切换既有路径）。
#[derive(Default)]
pub struct ToolRegistry {
    tools: Arc<RwLock<Vec<Arc<dyn ToolPort>>>>,
    state: Arc<ToolState>,
}

impl Clone for ToolRegistry {
    fn clone(&self) -> Self {
        Self { tools: self.tools.clone(), state: self.state.clone() }
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 运行期注册一个工具（装配期与 MCP 桥接 re-sync 共用入口）。
    pub fn register(&self, tool: Arc<dyn ToolPort>) {
        self.tools
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(tool);
    }

    /// 按名整批原子替换（MCP 桥接 re-sync 专用：两次注册间不留中间态——
    /// syncTools 两阶段 all-or-nothing 语义）。移除 `remove` 中命中注册表的名字、
    /// 追加 `add`，在**同一把写锁**内完成；返回实际被移除的名字（保序）。
    pub fn replace(&self, remove: &[String], add: Vec<Arc<dyn ToolPort>>) -> Vec<String> {
        let mut tools =
            self.tools.write().unwrap_or_else(std::sync::PoisonError::into_inner);
        let removed: Vec<String> = tools
            .iter()
            .filter(|tool| remove.contains(&tool.definition().name))
            .map(|tool| tool.definition().name)
            .collect();
        let removed_set: std::collections::HashSet<&String> = removed.iter().collect();
        tools.retain(|tool| !removed_set.contains(&tool.definition().name));
        tools.extend(add);
        removed
    }

    /// 共享状态句柄（main 的 /mode 切换入口）。
    pub fn state(&self) -> Arc<ToolState> {
        self.state.clone()
    }

    /// 把当前已注册的全部工具名设为 Agent 白名单（子会话装配点专用：子代理
    /// 继承宿主工具组合，面内全可见；父会话白名单由 Agent preset 装配期设定）。
    pub fn allow_all_registered(&self) {
        let names: Vec<String> = self
            .tools
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .map(|tool| tool.definition().name)
            .collect();
        self.state.set_agent(names);
    }

    /// 提取「工具名 → 风险等级」映射（部署侧装配 ApprovalGate 用）：注册表自动
    /// 提取，新增工具不会漏登记；审批侧据此以 `approval::decide` 判定。
    pub fn risk_map(&self) -> HashMap<String, ToolRisk> {
        self.tools
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .map(|tool| (tool.definition().name, tool.risk()))
            .collect()
    }

    /// 按调用内容分类风险（「按操作边界判定」）：取用工具覆写的
    /// [`ToolPort::classify_risk`]；未登记的工具按只读直通（执行侧兜底 unknown）。
    pub fn classify(&self, call: &ToolCall) -> ToolRisk {
        let tools = self.tools.read().unwrap_or_else(std::sync::PoisonError::into_inner);
        tools
            .iter()
            .find(|tool| tool.definition().name == call.name)
            .map(|tool| tool.classify_risk(call))
            .unwrap_or(ToolRisk::ReadOnly)
    }

    /// 当前 Agent 工具面可见工具的模型侧 schema（deferred 投影为 stub）。
    /// 每次调用求值——tool provider / 压缩快照等调用点自动获得动态工具集。
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        let agent = self.state.agent_tools();
        let tools = self.tools.read().unwrap_or_else(std::sync::PoisonError::into_inner);
        tools
            .iter()
            .filter(|tool| in_agent(tool.as_ref(), &agent))
            .map(|tool| project_definition(tool.as_ref()))
            .collect()
    }

    /// 当前 Agent 工具面内全部 deferred 工具的**完整** schema（tool_search 的搜索
    /// 范围；与 Agent 面准入边界一致——面外的 deferred 工具不参与检索）。
    pub fn deferred_definitions(&self) -> Vec<ToolDefinition> {
        let agent = self.state.agent_tools();
        let tools = self.tools.read().unwrap_or_else(std::sync::PoisonError::into_inner);
        tools
            .iter()
            .filter(|tool| tool.defer_loading() && in_agent(tool.as_ref(), &agent))
            .map(|tool| tool.definition())
            .collect()
    }

    /// 解锁一个 deferred 工具（tool_search 命中后调用；会话级，一次长期有效）。
    pub fn unlock(&self, name: &str) {
        self.state.unlock(name);
    }

    /// 执行一次调用：未知 / 面外 / 未解锁 / 取消 / 超时 / 端口错误均合成
    /// is_error 结果（不中断 step）。
    pub async fn execute(&self, call: &ToolCall, cancel: &CancellationToken, timeout: Duration) -> ToolResult {
        // 读锁只用于查找（guard 非 Send，禁止跨 await）：克隆 Arc 后立即放锁。
        let found = {
            let tools = self.tools.read().unwrap_or_else(std::sync::PoisonError::into_inner);
            tools.iter().find(|tool| tool.definition().name == call.name).cloned()
        };
        let Some(tool) = found else {
            return ToolResult::error(call, format!("unknown tool: {}", call.name));
        };
        let agent = self.state.agent_tools();
        if !in_agent(tool.as_ref(), &agent) {
            return ToolResult::error(call, format!("tool \"{}\" not in current agent tool set", call.name));
        }
        if tool.defer_loading() && !self.state.is_unlocked(&call.name) {
            return ToolResult::error(
                call,
                format!("deferred tool \"{}\" requires tool_search first", call.name),
            );
        }
        let call = call.clone();
        let cancel = cancel.clone();
        let run = tool.execute(call.clone(), cancel.clone());
        // 阻断型工具不参与统一 tool_timeout（交互工具省略 timeoutMs 即
        // 无限阻塞，等待取消或宿主回填）；其余工具保留取消/超时治理。
        let outcome = if tool.blocking() {
            tokio::select! {
                _ = cancel.cancelled() => Err("tool call aborted".to_string()),
                result = run => result,
            }
        } else {
            tokio::select! {
                _ = cancel.cancelled() => Err("tool call aborted".to_string()),
                result = run => result,
                _ = tokio::time::sleep(timeout) => Err(format!("tool call timed out after {}s", timeout.as_secs())),
            }
        };
        match outcome {
            Ok(blocks) => ToolResult { tool_call_id: call.id.clone(), name: call.name.clone(), is_error: false, content: blocks },
            Err(message) => ToolResult::error(&call, message),
        }
    }
}

/// 工具是否在当前 Agent 工具名集合内（白名单；空集合 = 全不可见）。
fn in_agent(tool: &dyn ToolPort, agent: &HashSet<String>) -> bool {
    agent.contains(&tool.definition().name)
}

/// 模型侧 schema 投影：deferred 工具下发 stub（原名 + 提示后缀 + 空 object schema，
/// 保证 OpenAI 线协议合法）；普通工具原样。
fn project_definition(tool: &dyn ToolPort) -> ToolDefinition {
    let mut definition = tool.definition();
    if tool.defer_loading() {
        definition.description =
            format!("{} [deferred: call tool_search to load the full schema]", definition.description);
        definition.parameters = json!({ "type": "object", "properties": {} });
    }
    definition
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "office_tests.rs"]
mod office_tests;
