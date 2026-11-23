//! Agent 预设：AgentDefinition / PresentationMode / AgentCatalog。
//!
//! 一个 Agent = 工具名列表 + 系统提示词 + 呈现模式。
//! onion 内置双预设（coding / work），并支持从 YAML 配置文件读取用户自定义
//! Agent 定义，与内置预设合并为 Agent 目录（`AgentCatalog`）。
//!
//! 2026-10-20 决策：
//! - 工具面由「工作模式（chat/work）」改为「Agent 工具名集合」过滤；
//! - 自定义预设改用 YAML 格式（`agents.yml`），保持声明式风格；
//! - `tool_search`/deferred 保留代码但不使用（见 `tools/mod.rs`）。

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

/// 呈现模式：模型工具调用的呈现风格（native 为默认，ptc 面向
/// 编码场景，见 P5 落地）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PresentationMode {
    /// 原生工具调用呈现。
    #[default]
    Native,
    /// PTC（plan-tool-code）编码场景呈现（P5 落地，当前等同 native）。
    Ptc,
}

/// 一个 Agent 定义：工具名列表 + 系统提示词 + 呈现模式。
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct AgentDefinition {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub system_prompt: String,
    #[serde(default)]
    pub presentation: PresentationMode,
}

/// YAML 自定义 Agent 文件根结构（`agents.yml`）。
#[derive(Clone, Debug, Deserialize)]
pub struct AgentFile {
    #[serde(default)]
    pub agents: Vec<AgentDefinition>,
}

/// Agent 目录：id → 定义。内置双预设 + 用户自定义合并。
#[derive(Clone, Debug, Default)]
pub struct AgentCatalog {
    agents: HashMap<String, AgentDefinition>,
}

/// 创造模式提示词（Creator preset：随 preset 挂载插件开发技能与
/// 工作流指导）。`{{model}}` 由装配期插值。
pub const CREATOR_SYSTEM_PROMPT: &str = "你处于创造模式（creator，由 {{model}} 驱动）：根据用户指令开发、校验并安装 ava2 App。\
一个 ava2 App = 一个目录（manifest.json + ESM 入口），可按需组合贡献 UI 页（contributes.apps）、技能（skills）、\
MCP server（mcpServers）与 hooks（Claude Code 兼容）中的任意几类。\n\n\
工作流程：\n\
1. 技能列表里有 develop-ava2-app 时，先用 skill 工具加载它再动手；否则定位 ava2 仓库（从会话工作目录向上找 \
contracts/app-contract.json），直接读契约（contracts/app-contract.json）、类型（src/host/types.ts）与参考实现（apps/ 下按组合类型选读）。\n\
2. 新 App 源码写在 ava2 仓库 apps/<id>/ 下：目录名与 manifest.id 一致（kebab-case）；优先裸 ESM 形态\
（manifest.json + app.js，react 由宿主 import map 供给）；改过已发布 App 的代码要 bump version。\n\
3. 交付前在 ava2 仓库根目录运行 node scripts/check-app-contract.mjs，0 error 才允许安装。\n\
4. 用 pwsh 调安装 REST：端口读环境变量 ONION_SERVER_PORT（缺省 9800），\
POST http://127.0.0.1:<端口>/v1/apps/install，body {\"path\":\"<App 目录绝对路径>\",\"overwrite\":true}，\
必须 UTF-8 无 BOM 编码；装完提示用户在 http://localhost:1420/ 查看。\n\
5. 校验/安装报错要原样转述并修复重试，禁止虚构安装结果；样式一律用 var(--ava-*) / var(--cc-*) 令牌，\
禁止硬编码色值与字号。";

impl AgentCatalog {
    /// 内置双预设：coding（编码，含 terminal）/ work（办公，含
    /// schedule，无 terminal）；creator（创造模式：按契约开发并安装 App）。
    pub fn builtin() -> Self {
        let mut catalog = Self::default();
        catalog.insert(AgentDefinition {
            id: "coding".to_string(),
            name: "Coding Agent".to_string(),
            tools: coding_tools(),
            system_prompt: "You are a coding agent powered by the {{model}} model. \
                Focus on reading, editing, and writing code in the workspace."
                .to_string(),
            presentation: PresentationMode::Native,
        });
        catalog.insert(AgentDefinition {
            id: "work".to_string(),
            name: "Office Assistant".to_string(),
            tools: work_tools(),
            system_prompt: "You are an office assistant. Help with documents, scheduling, \
                web research, and everyday tasks. Shell/terminal tools are not available."
                .to_string(),
            presentation: PresentationMode::Native,
        });
        catalog.insert(AgentDefinition {
            id: "creator".to_string(),
            name: "创造模式".to_string(),
            tools: coding_tools(),
            system_prompt: CREATOR_SYSTEM_PROMPT.to_string(),
            presentation: PresentationMode::Native,
        });
        catalog
    }

    /// 从 YAML 文件加载自定义 Agent 定义（不存在则空目录，不报错）。
    /// 覆盖同 id 的内置预设（用户自定义优先）。
    pub fn load_custom(path: Option<&Path>) -> std::io::Result<Self> {
        let mut catalog = Self::default();
        let Some(path) = path else {
            return Ok(catalog);
        };
        if !path.exists() {
            return Ok(catalog);
        }
        let raw = std::fs::read_to_string(path)?;
        let file: AgentFile = serde_yaml::from_str(&raw)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        for agent in file.agents {
            catalog.insert(agent);
        }
        Ok(catalog)
    }

    /// 合并内置 + 自定义（自定义覆盖内置同 id），返回完整目录。
    pub fn merged(custom_path: Option<&Path>) -> std::io::Result<Self> {
        let mut catalog = Self::builtin();
        let custom = Self::load_custom(custom_path)?;
        for (id, agent) in custom.agents {
            catalog.agents.insert(id, agent);
        }
        Ok(catalog)
    }

    fn insert(&mut self, agent: AgentDefinition) {
        self.agents.insert(agent.id.clone(), agent);
    }

    /// 按 id 取 Agent 定义（缺省回退 `coding`）。
    pub fn get(&self, id: &str) -> Option<&AgentDefinition> {
        self.agents.get(id)
    }

    /// 全部 Agent id（有序，/agent 选择器数据源）。
    pub fn ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.agents.keys().cloned().collect();
        ids.sort();
        ids
    }
}

/// coding 预设工具名列表（编码场景，含 terminal_*）。
pub fn coding_tools() -> Vec<String> {
    vec![
        "read".into(),
        "read_image".into(),
        "write".into(),
        "edit".into(),
        "str_replace_editor".into(),
        "apply_patch".into(),
        "office_convert".into(),
        "office_recalculate".into(),
        "office_render".into(),
        "glob".into(),
        "grep".into(),
        "bash".into(),
        "pwsh".into(),
        "job_list".into(),
        "job_output".into(),
        "job_kill".into(),
        "workflow".into(),
        "todo_write".into(),
        "SaveMemory".into(),
        "UpdateMemory".into(),
        "RecallMemory".into(),
        "ReadMemory".into(),
        "DeleteMemory".into(),
        "ListMemories".into(),
        "memory_search".into(),
        "web_search".into(),
        "web_fetch".into(),
        "session_search".into(),
        "session_event_search".into(),
        "session_trace".into(),
        "session_event_trace".into(),
        "session_event_read".into(),
        "present".into(),
        "skill".into(),
        "working_directory".into(),
        "create_worktree".into(),
        "load_workspace_dependencies".into(),
        "lsp".into(),
        "a2a_call".into(),
        "a2a_send".into(),
        "a2a_approve".into(),
        "terminal_open".into(),
        "terminal_send".into(),
        "terminal_read".into(),
        "terminal_signal".into(),
        "terminal_close".into(),
        "terminal_list".into(),
        "schedule_create".into(),
        "schedule_list".into(),
        "schedule_update".into(),
        "schedule_delete".into(),
        "list_mcp_resources".into(),
        "list_mcp_resource_templates".into(),
        "read_mcp_resource".into(),
        "get_goal".into(),
        "create_goal".into(),
        "update_goal".into(),
        "exit_plan_mode".into(),
        "ask_user_question".into(),
        "subagent".into(),
        "subagent_fork".into(),
        "send_message".into(),
        "interrupt_agent".into(),
        "list_agents".into(),
        "list_subagent_models".into(),
        // agent-team 启用（`ONION_AGENT_TEAM=1`）时 team 工具接管 `send_message` /
        // `list_agents` / `interrupt_agent` 三个共享名，并新增以下 6 件；未启用时
        // 这些名字未注册，白名单中的冗余项无副作用。
        "spawn_teammate".into(),
        "wait_agent".into(),
        "team_task_create".into(),
        "team_task_list".into(),
        "team_task_get".into(),
        "team_task_update".into(),
        "ralph".into(),
    ]
}

/// work 预设工具名列表（办公场景，含 schedule_*，无 terminal / bash / pwsh）。
pub fn work_tools() -> Vec<String> {
    vec![
        "read".into(),
        "read_image".into(),
        "write".into(),
        "edit".into(),
        "str_replace_editor".into(),
        "apply_patch".into(),
        "office_convert".into(),
        "office_recalculate".into(),
        "office_render".into(),
        "glob".into(),
        "grep".into(),
        "job_list".into(),
        "job_output".into(),
        "job_kill".into(),
        "workflow".into(),
        "todo_write".into(),
        "SaveMemory".into(),
        "UpdateMemory".into(),
        "RecallMemory".into(),
        "ReadMemory".into(),
        "DeleteMemory".into(),
        "ListMemories".into(),
        "memory_search".into(),
        "web_search".into(),
        "web_fetch".into(),
        "session_search".into(),
        "session_event_search".into(),
        "session_trace".into(),
        "session_event_trace".into(),
        "session_event_read".into(),
        "present".into(),
        "skill".into(),
        "working_directory".into(),
        "load_workspace_dependencies".into(),
        "list_mcp_resources".into(),
        "list_mcp_resource_templates".into(),
        "read_mcp_resource".into(),
        "get_goal".into(),
        "create_goal".into(),
        "update_goal".into(),
        "exit_plan_mode".into(),
        "ask_user_question".into(),
        "subagent".into(),
        "subagent_fork".into(),
        "send_message".into(),
        "interrupt_agent".into(),
        "list_agents".into(),
        "list_subagent_models".into(),
        "a2a_call".into(),
        "a2a_send".into(),
        "a2a_approve".into(),
        "spawn_teammate".into(),
        "wait_agent".into(),
        "team_task_create".into(),
        "team_task_list".into(),
        "team_task_get".into(),
        "team_task_update".into(),
        "ralph".into(),
        "schedule_create".into(),
        "schedule_list".into(),
        "schedule_update".into(),
        "schedule_delete".into(),
    ]
}

#[cfg(test)]
#[path = "preset_tests.rs"]
mod tests;
