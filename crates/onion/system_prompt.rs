//! 系统提示词注册表：段 / 运行时上下文 / 变量 / 工具序。
//!
//! - 段（PromptSection）：按 `order` 升序、同 order 按名称排序拼接为 system 文本；
//! - 运行时上下文（PromptContext）：独立于段的动态事实，渲染为 user 角色快照
//!   （简化为开轮装配一次，不在轮中逐步刷新）；
//! - 变量：`{{name}}` 严格插值——未知 / 未赋值 / 格式错误引用使组装失败
//!   （格式错误的提示词比明确失败更糟）；替换值不再扫描；孤立的 `{{`
//!   （其后无 `}}`）视为字面文本；
//! - 工具序：显式 `toolOrder` + `<unlisted-tools>` rest 标记，未列出工具按
//!   字典序插入该位置；
//! - complete 段：组装后恢复为唯一 system 段（上下文 / 工具 / 变量保留）。
//!
//! 单 agent 教学示例，仅全局层、无作用域遮蔽与组装瀑布
//! （瀑布扩展点见 listener 的 request 边界）。

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Arc;

use crate::config::Config;
use crate::types::ToolDefinition;

/// 集中分配的段位置（教学子集；外部贡献可用任意值）。
pub mod section_orders {
    /// 固定 harness 身份（第一方开场白）。
    pub const IDENTITY: i64 = -1000;
    /// 部署 persona 前缀（首方指导之前；同名段是替换点）。
    pub const PERSONA_PREFIX: i64 = 0;
    /// Agent 行为指令（所选 Agent 的 `system_prompt`，紧跟 persona 之后、
    /// plan/工具策略之前）。
    pub const AGENT_DIRECTIVE: i64 = 100;
    /// plan 模式策略分节（order 500；active 时输出、
    /// inactive 空串消失）。
    pub const PLAN_POLICY: i64 = 500;
    /// 团队策略（agent-team）。
    pub const TEAM_POLICY: i64 = 600;
    /// 工具指导类段落（工具文档段的代表位）。
    pub const TOOL_GUIDE: i64 = 1000;
    /// shell 工具策略（bash）。
    pub const TOOL_BASH: i64 = 1000;
    /// shell 工具策略（pwsh）。
    pub const TOOL_PWSH: i64 = 1010;
    /// read 工具策略。
    pub const TOOL_READ: i64 = 1100;
    /// write 工具策略。
    pub const TOOL_WRITE: i64 = 1200;
    /// edit 工具策略。
    pub const TOOL_EDIT: i64 = 1300;
    /// glob 工具策略。
    pub const TOOL_GLOB: i64 = 1400;
    /// grep 工具策略。
    pub const TOOL_GREP: i64 = 1500;
    /// 后台任务控制面（job_list / job_output / job_kill）。
    pub const TOOL_JOBS: i64 = 1600;
    /// 终端策略（terminal_* 6 件）。
    pub const TOOL_PTY: i64 = 1700;
    /// web 搜索策略。
    pub const TOOL_WEB_SEARCH: i64 = 2000;
    /// web 抓取策略。
    pub const TOOL_WEB_FETCH: i64 = 2100;
    /// LSP 导航策略（lsp）。
    pub const TOOL_LSP: i64 = 2200;
    /// 会话历史查询（session_* 5 件）。
    pub const TOOL_SESSION_QUERY: i64 = 2300;
    /// goal 体系策略（get_goal / create_goal / update_goal）。
    pub const TOOL_GOAL: i64 = 2400;
    /// 记忆管理策略（SaveMemory / RecallMemory / memory_search 等 7 件；onion
    /// 自研能力，不依赖 ava-\*，蓝本为 ava-harness::memory）。
    pub const MEMORY_POLICY: i64 = 2500;
    /// MCP 资源策略（mcp_resource 3 件）。
    pub const MCP_RESOURCE_POLICY: i64 = 3100;
    /// ralph 策略。
    pub const TOOL_RALPH: i64 = 2700;
    /// workflow 编排策略（声明式 JSON 程序）。
    pub const TOOL_WORKFLOW: i64 = 2750;
    /// subagent 策略。
    pub const TOOL_SUBAGENT: i64 = 2800;
    /// todo 清单策略（todo_write）。
    pub const TODO_POLICY: i64 = 6000;
    /// 交付物声明策略（present）。
    pub const PRESENT_POLICY: i64 = 8500;
    /// 技能加载策略（skill）。
    pub const SKILL_POLICY: i64 = 9000;
    /// workspace 依赖策略（load_workspace_dependencies）。
    pub const WORKSPACE_DEPS_POLICY: i64 = 9300;
    /// 工作目录策略。
    pub const WORKING_DIR_POLICY: i64 = 9500;
    /// git worktree 策略（create_worktree）。
    pub const WORKTREE_POLICY: i64 = 9600;
    /// 提醒策略（schedule_* 4 件）。
    pub const SCHEDULE_POLICY: i64 = 9800;
    /// 部署 persona 后缀（携带环境信息，首方指导之后）。
    pub const PERSONA_SUFFIX: i64 = 10200;
}

/// 集中分配的运行时上下文位置。
pub mod context_orders {
    /// 当前工作目录（进程相关事实不进缓存 system prompt，走快照）。
    pub const WORKING_DIRECTORY: i64 = 100;
    /// subagent 派生深度。
    pub const SUBAGENT_DELEGATION: i64 = 120;
    /// 会话时钟（onion 演示用动态事实）。
    pub const RUNTIME_CLOCK: i64 = 150;
}

/// 工具序 rest 标记：未列出的工具按字典序插入该位置（恰出现一次）。
pub const TOOL_ORDER_REST: &str = "<unlisted-tools>";

/// 变量名规则：`^[a-z][a-z0-9_]*$`。
fn valid_variable_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// 段 / 上下文文本：静态字面量，或每次装配求值的动态 provider。
#[derive(Clone)]
pub enum SectionText {
    Static(String),
    Dynamic(Arc<dyn Fn() -> String + Send + Sync>),
}

impl SectionText {
    fn resolve(&self) -> String {
        match self {
            Self::Static(text) => text.clone(),
            Self::Dynamic(provider) => provider(),
        }
    }
}

impl fmt::Debug for SectionText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Static(text) => f.debug_tuple("Static").field(text).finish(),
            Self::Dynamic(_) => f.debug_tuple("Dynamic").field(&"<provider>").finish(),
        }
    }
}

/// 一条系统提示词段（注册表输入；同名重复注册被拒绝）。
#[derive(Clone, Debug)]
pub struct PromptSection {
    pub name: String,
    /// 升序拼接；同 order 按名称排序。
    pub order: i64,
    pub text: SectionText,
    /// 是否插值 `{{variable}}`（false 保留字面文本，缺省 true）。
    pub interpolate: bool,
    /// 视为完整提示词：组装后成为唯一段（上下文 / 工具 / 变量保留）。
    pub complete: bool,
}

/// 一条动态运行时上下文（渲染为 user 快照，与段分离；同名重复注册被拒绝）。
#[derive(Clone, Debug)]
pub struct PromptContext {
    pub name: String,
    /// 升序拼接。
    pub order: i64,
    pub text: SectionText,
    pub interpolate: bool,
    /// 可选运行时上下文被抑制时仍保留。
    pub required: bool,
}

/// 已解析（未插值）的一条段。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssembledSection {
    pub name: String,
    pub text: String,
    pub interpolate: bool,
}

/// 已解析的一条动态上下文。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssembledContext {
    pub name: String,
    pub text: String,
    pub interpolate: bool,
}

/// 组装产物：段与上下文未插值，工具已规范排序，变量已求值。
#[derive(Clone, Debug)]
pub struct PromptAssembly {
    pub sections: Vec<AssembledSection>,
    pub contexts: Vec<AssembledContext>,
    pub tools: Vec<ToolDefinition>,
    pub variables: HashMap<String, Option<String>>,
}

/// 组装 / 渲染失败（严格插值契约：不向模型发送格式错误的提示词）。
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PromptError {
    #[error("提示词段 \"{0}\" 已注册")]
    DuplicateSection(String),
    #[error("运行时上下文 \"{0}\" 已注册")]
    DuplicateContext(String),
    #[error("提示词变量 \"{0}\" 已注册")]
    DuplicateVariable(String),
    #[error("变量名 \"{0}\" 非法（须匹配 ^[a-z][a-z0-9_]*$）")]
    InvalidVariableName(String),
    #[error("多个 complete 段同时生效: {0:?}")]
    MultipleComplete(Vec<String>),
    #[error("{kind} \"{name}\" 中存在格式错误的变量引用 \"{snippet}…\"（引用须为完整 {{{{name}}}} 组）")]
    MalformedReference { kind: &'static str, name: String, snippet: String },
    #[error("{kind} \"{name}\" 引用了未注册变量 \"{{{{{variable}}}}}\"；已注册变量: {known:?}")]
    UnknownVariable { kind: &'static str, name: String, variable: String, known: Vec<String> },
    #[error("{kind} \"{name}\" 引用的变量 \"{{{{{variable}}}}}\" 本次装配无值")]
    UnassignedVariable { kind: &'static str, name: String, variable: String },
    #[error("toolOrder 必须包含且仅包含一个 \"<unlisted-tools>\" rest 标记")]
    ToolOrderMissingRest,
    #[error("toolOrder 重复列出 \"{0}\"")]
    ToolOrderDuplicate(String),
    #[error("工具 provider 返回了保留名 \"{0}\"（为 toolOrder 的 rest 标记保留）")]
    ReservedToolName(String),
}

type VariableProvider = Arc<dyn Fn() -> Option<String> + Send + Sync>;
type ToolProvider = Arc<dyn Fn() -> Vec<ToolDefinition> + Send + Sync>;

/// 系统提示词注册表（教学版：单全局层，无作用域遮蔽）。
pub struct SystemPrompt {
    sections: Vec<PromptSection>,
    contexts: Vec<PromptContext>,
    variables: HashMap<String, VariableProvider>,
    tool_providers: Vec<ToolProvider>,
    tool_order: Option<Vec<String>>,
    runtime_suppressed: bool,
}

impl fmt::Debug for SystemPrompt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SystemPrompt")
            .field("sections", &self.sections.iter().map(|s| &s.name).collect::<Vec<_>>())
            .field("contexts", &self.contexts.iter().map(|c| &c.name).collect::<Vec<_>>())
            .field("variables", &self.variables.keys().collect::<Vec<_>>())
            .field("tool_providers", &self.tool_providers.len())
            .field("tool_order", &self.tool_order)
            .field("runtime_suppressed", &self.runtime_suppressed)
            .finish()
    }
}

impl SystemPrompt {
    /// 按 Config 装配基线：harness 身份（可选）+ persona 前缀 / 后缀。
    /// persona 段始终注册——同名段（`deployment:persona-prefix` / `-suffix`）是替换点，
    /// 空文本段渲染时消失。
    pub fn new(config: &Config) -> Self {
        let mut prompts = Self {
            sections: Vec::new(),
            contexts: Vec::new(),
            variables: HashMap::new(),
            tool_providers: Vec::new(),
            tool_order: config.tool_order.clone(),
            runtime_suppressed: !config.include_runtime_context,
        };
        if config.include_identity {
            let _ = prompts.section(PromptSection {
                name: "harness:identity".to_string(),
                order: section_orders::IDENTITY,
                text: SectionText::Static(
                    "You are an AI agent powered by the onion harness.".to_string(),
                ),
                interpolate: false,
                complete: false,
            });
        }
        let _ = prompts.section(PromptSection {
            name: "deployment:persona-prefix".to_string(),
            order: section_orders::PERSONA_PREFIX,
            text: SectionText::Static(config.persona_prefix.clone()),
            interpolate: true,
            complete: false,
        });
        let _ = prompts.section(PromptSection {
            name: "deployment:persona-suffix".to_string(),
            order: section_orders::PERSONA_SUFFIX,
            text: SectionText::Static(config.persona_suffix.clone()),
            interpolate: true,
            complete: false,
        });
        prompts
    }

    /// 注册段：同名重复拒绝（重复段多为覆盖意图，应改用同名 persona 替换点）。
    pub fn section(&mut self, section: PromptSection) -> Result<(), PromptError> {
        if self.sections.iter().any(|s| s.name == section.name) {
            return Err(PromptError::DuplicateSection(section.name));
        }
        self.sections.push(section);
        Ok(())
    }

    /// 注册运行时上下文：同名重复拒绝。
    pub fn context(&mut self, context: PromptContext) -> Result<(), PromptError> {
        if self.contexts.iter().any(|c| c.name == context.name) {
            return Err(PromptError::DuplicateContext(context.name));
        }
        self.contexts.push(context);
        Ok(())
    }

    /// 注册提示变量（名须匹配 `^[a-z][a-z0-9_]*$`）；provider 可返回 None
    /// 表示本次装配无值——渲染引用该变量的段时失败。
    pub fn variable(&mut self, name: &str, provider: VariableProvider) -> Result<(), PromptError> {
        if !valid_variable_name(name) {
            return Err(PromptError::InvalidVariableName(name.to_string()));
        }
        if self.variables.contains_key(name) {
            return Err(PromptError::DuplicateVariable(name.to_string()));
        }
        self.variables.insert(name.to_string(), provider);
        Ok(())
    }

    /// 注册工具 schema provider：每次装配求值，贡献本装配可见的工具集合。
    pub fn add_tool_provider(&mut self, provider: ToolProvider) {
        self.tool_providers.push(provider);
    }

    /// 抑制可选运行时上下文（required 的贡献保留；单开关）。
    pub fn suppress_runtime_context(&mut self) {
        self.runtime_suppressed = true;
    }

    /// 组装：求值段 / 上下文 / 变量，收集并规范排序工具（装配第一阶段）。
    /// 返回值尚未插值；插值由 `render_prompt` / `render_context_snapshot` 完成。
    pub fn assemble(&self) -> Result<PromptAssembly, PromptError> {
        let mut variables: HashMap<String, Option<String>> = HashMap::new();
        for (name, provider) in &self.variables {
            variables.insert(name.clone(), provider());
        }
        // 段：order 升序、同 order 按名称序（Rust str 序 = 字节序）。
        let mut definitions: Vec<&PromptSection> = self.sections.iter().collect();
        definitions.sort_by(|a, b| {
            a.order.cmp(&b.order).then_with(|| compare_names(&a.name, &b.name))
        });
        let complete_names: Vec<String> =
            definitions.iter().filter(|s| s.complete).map(|s| s.name.clone()).collect();
        if complete_names.len() > 1 {
            return Err(PromptError::MultipleComplete(complete_names));
        }
        let mut sections: Vec<AssembledSection> = definitions
            .iter()
            .map(|s| AssembledSection {
                name: s.name.clone(),
                text: s.text.resolve(),
                interpolate: s.interpolate,
            })
            .collect();
        if let Some(index) = definitions.iter().position(|s| s.complete) {
            sections = vec![sections.remove(index)];
        }
        // 上下文：order 升序；抑制时仅保留 required。
        let mut contexts: Vec<&PromptContext> = self.contexts.iter().collect();
        contexts.sort_by_key(|c| c.order);
        let contexts: Vec<AssembledContext> = contexts
            .iter()
            .filter(|c| !self.runtime_suppressed || c.required)
            .map(|c| AssembledContext {
                name: c.name.clone(),
                text: c.text.resolve(),
                interpolate: c.interpolate,
            })
            .collect();
        // 工具：收集 + 保留名检查 + 规范排序。
        let mut tools = Vec::new();
        for provider in &self.tool_providers {
            tools.extend(provider());
        }
        let tools = order_tools(tools, self.tool_order.as_ref())?;
        Ok(PromptAssembly { sections, contexts, tools, variables })
    }
}

/// 名称比较（按字节序，ASCII 名称下结果确定）。
fn compare_names(a: &str, b: &str) -> Ordering {
    a.cmp(b)
}

/// 应用工具序：显式列表（rest 标记处插入未列出工具的字典序）或整体字典序。
/// 缺席的已列名直接跳过（无 knownNames 机制，教学版从简）。
fn order_tools(
    tools: Vec<ToolDefinition>,
    tool_order: Option<&Vec<String>>,
) -> Result<Vec<ToolDefinition>, PromptError> {
    for tool in &tools {
        if tool.name == TOOL_ORDER_REST {
            return Err(PromptError::ReservedToolName(tool.name.clone()));
        }
    }
    let Some(order) = tool_order else {
        let mut tools = tools;
        tools.sort_by(|a, b| compare_names(&a.name, &b.name));
        return Ok(tools);
    };
    let mut seen: HashSet<&str> = HashSet::new();
    let mut rest_count = 0usize;
    for name in order {
        if name == TOOL_ORDER_REST {
            rest_count += 1;
        }
        if !seen.insert(name.as_str()) {
            return Err(PromptError::ToolOrderDuplicate(name.clone()));
        }
    }
    if rest_count != 1 {
        return Err(PromptError::ToolOrderMissingRest);
    }
    let listed: HashSet<&str> =
        order.iter().map(String::as_str).filter(|name| *name != TOOL_ORDER_REST).collect();
    let mut rest: Vec<ToolDefinition> =
        tools.iter().filter(|tool| !listed.contains(tool.name.as_str())).cloned().collect();
    rest.sort_by(|a, b| compare_names(&a.name, &b.name));
    let mut result = Vec::new();
    for name in order {
        if name == TOOL_ORDER_REST {
            result.append(&mut rest);
        } else if let Some(tool) = tools.iter().find(|tool| tool.name == *name) {
            result.push(tool.clone());
        }
    }
    Ok(result)
}

/// 渲染 system 文本：插值、丢空段、空行连接。
pub fn render_prompt(assembly: &PromptAssembly) -> Result<String, PromptError> {
    let mut parts = Vec::new();
    for section in &assembly.sections {
        let text = if section.interpolate {
            interpolate(&section.name, "section", &section.text, &assembly.variables)?
        } else {
            section.text.clone()
        };
        if !text.is_empty() {
            parts.push(text);
        }
    }
    Ok(parts.join("\n\n"))
}

/// 已插值的上下文贡献（按序、非空）。
pub fn render_context_sections(assembly: &PromptAssembly) -> Result<Vec<AssembledContext>, PromptError> {
    let mut parts = Vec::new();
    for context in &assembly.contexts {
        let text = if context.interpolate {
            interpolate(&context.name, "context", &context.text, &assembly.variables)?
        } else {
            context.text.clone()
        };
        if !text.is_empty() {
            parts.push(AssembledContext {
                name: context.name.clone(),
                text,
                interpolate: context.interpolate,
            });
        }
    }
    Ok(parts)
}

/// 完整运行时快照文本（前缀声明本快照取代更早快照）。
pub fn render_context_snapshot(assembly: &PromptAssembly) -> Result<String, PromptError> {
    let sections = render_context_sections(assembly)?;
    if sections.is_empty() {
        return Ok(String::new());
    }
    let body =
        sections.iter().map(|section| section.text.as_str()).collect::<Vec<_>>().join("\n\n");
    Ok(format!(
        "Current runtime context. This snapshot supersedes earlier runtime-context snapshots.\n\n{body}"
    ))
}

/// 严格插值一个段 / 上下文：malformed、未知、未赋值引用
/// 均报错并归属到 owning 输入；替换值不再扫描；孤立的 `{{` 为字面文本。
fn interpolate(
    name: &str,
    kind: &'static str,
    text: &str,
    variables: &HashMap<String, Option<String>>,
) -> Result<String, PromptError> {
    let mut result = String::new();
    let mut cursor = 0usize;
    while let Some(offset) = text[cursor..].find("{{") {
        let open = cursor + offset;
        let inner = &text[open + 2..];
        let Some(relative) = inner.find("}}") else {
            // 其后无 `}}`：孤立 `{{` 视为字面文本（消费 "{{"，继续扫描）。
            result.push_str(&text[cursor..open + 2]);
            cursor = open + 2;
            continue;
        };
        let group_name = &inner[..relative];
        if !valid_variable_name(group_name) {
            let snippet: String = text[open..].chars().take(16).collect();
            return Err(PromptError::MalformedReference {
                kind,
                name: name.to_string(),
                snippet,
            });
        }
        match variables.get(group_name) {
            None => {
                let mut known: Vec<String> = variables.keys().cloned().collect();
                known.sort();
                return Err(PromptError::UnknownVariable {
                    kind,
                    name: name.to_string(),
                    variable: group_name.to_string(),
                    known,
                });
            }
            Some(None) => {
                return Err(PromptError::UnassignedVariable {
                    kind,
                    name: name.to_string(),
                    variable: group_name.to_string(),
                });
            }
            Some(Some(value)) => {
                result.push_str(&text[cursor..open]);
                result.push_str(value);
                cursor = open + 2 + relative + 2;
            }
        }
    }
    result.push_str(&text[cursor..]);
    Ok(result)
}

#[cfg(test)]
#[path = "system_prompt_tests.rs"]
mod tests;
