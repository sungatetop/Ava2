//! subagent 工具面：6 工具注册（工具即 API）。
//!
//! 核心语义：
//! - subagent / subagent_fork：创建 subagent 会话（depth=1，固定一层）；
//! - send_message：向活跃 subagent 发消息（steer，next-step + 唤醒）；
//! - interrupt_agent：取消 subagent（cancel token）；
//! - list_agents：列出活跃 subagent（id | depth | status 表格）；
//! - list_subagent_models：列出子代理可用 LLM 路由（provider → models → efforts）。
//!
//! 接线时序：工具构造早于父会话 Runtime 装配（inbox / SessionLog 装配后才
//! 存在），经 OnceLock 槽位延迟接线（`wire_parent`，宿主装配完成后调用）。

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::ToolPort;
use crate::types::{ContentBlock, ToolCall, ToolDefinition};
use crate::waterfall::BoxFuture;

use crate::subagent::runtime::SubagentRuntime;
use crate::subagent::types::SubagentActivation;
use crate::inbox::Inbox;
use crate::session::SessionLog;

/// 工具族共享句柄：Provider / 父会话接线槽 / 活跃 subagent 表 / 模型目录。
#[derive(Clone)]
struct SubagentHandles {
    runtime: Arc<SubagentRuntime>,
    parent_inbox: Arc<OnceLock<Arc<Inbox>>>,
    parent_log: Arc<OnceLock<Arc<SessionLog>>>,
    agents: Arc<Mutex<HashMap<String, SubagentActivation>>>,
    /// 子代理可用模型目录（`list_subagent_models` 数据源；providers.json / 内嵌表）。
    /// 目录只决定「哪些路由存在」；**是否允许**由会话白名单决定（见 `policy`）。
    catalog: crate::provider_catalog::ProviderCatalog,
    /// 父会话当前路由（无白名单时显式选择只允许它，
    /// `assertAllowedModelSelection` 的 `parentOptions` 语义）。
    parent_route: crate::subagent::model_selection::AllowedModelRoute,
    /// 会话级路由白名单（durable `subagent/model-selection-policy` 投影；
    /// None = 未记录 → 显式选择只允许父路由）。
    ///
    /// 装配期日志尚未建立，故经 `wire_parent` 延迟投影（`OnceLock`）。
    policy: Arc<OnceLock<Option<Vec<crate::subagent::model_selection::AllowedModelRoute>>>>,
    /// 子代理可选 Agent 预设白名单（可缺省）。
    ///
    /// 空 = 不允许模型选择预设（`agent_preset` 不注入 schema，传了即报错）；
    /// 非空 = 注入 `agent_preset` enum。省略该参数本身 = 继承父会话预设。
    preset_choices: Vec<String>,
}

impl SubagentHandles {
    /// 创建 subagent 会话（depth=1，固定一层——工具面不含 subagent 工具，
    /// subagent 会话无本工具族，天然不递归）。
    ///
    /// `name` 校验在此收口：小写字母开头，仅小写字母/数字/连字符，
    /// ≤32 字符，`lead` 保留给根代理，活跃表内唯一。
    fn activate(
        &self,
        prompt: String,
        fork: bool,
        options: crate::subagent::runtime::DelegationOptions,
    ) -> Result<crate::subagent::types::SubagentActivation, String> {
        if let Some(name) = &options.name {
            validate_agent_name(name, &lock(&self.agents))?;
        }
        let inbox = self
            .parent_inbox
            .get()
            .cloned()
            .ok_or_else(|| "subagent tools not wired (missing parent inbox)".to_string())?;
        let activation = if fork {
            let log = self
                .parent_log
                .get()
                .cloned()
                .ok_or_else(|| "subagent tools not wired (missing parent log)".to_string())?;
            self.runtime.fork_delegated(&log, prompt, 1, Some(inbox), options)
        } else {
            self.runtime.spawn_delegated(prompt, 1, Some(inbox), options)
        };
        activation.map(|activation| {
            lock(&self.agents).insert(activation.id.clone(), activation.clone());
            activation
        })
        .map_err(|error| error.to_string())
    }

    /// 按 id 或 name 定位活跃 subagent（name 为模型可读标识）。
    fn find_agent(&self, key: &str) -> Option<crate::subagent::types::SubagentActivation> {
        let agents = lock(&self.agents);
        agents
            .get(key)
            .cloned()
            .or_else(|| agents.values().find(|a| a.name == key).cloned())
    }
}

/// 校验显式代理名（小写字母开头；仅小写字母/数字/连字符；≤32 字符；
/// `lead` 保留给根代理；活跃 subagent 间唯一）。
fn validate_agent_name(
    name: &str,
    agents: &HashMap<String, crate::subagent::types::SubagentActivation>,
) -> Result<(), String> {
    if name == "lead" {
        return Err("`lead` is reserved for the root agent".to_string());
    }
    let starts_ok = name
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_lowercase());
    if !starts_ok {
        return Err("name must start with a lower-case letter".to_string());
    }
    if name.len() > 32 {
        return Err("name must be at most 32 characters".to_string());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(
            "name may only contain lower-case letters, digits, and hyphens".to_string(),
        );
    }
    if agents.values().any(|a| a.name == name) {
        return Err(format!("subagent name \"{name}\" is already in use"));
    }
    Ok(())
}

/// 解析委派路由（modelSelection：会话白名单授权，目录只决定路由是否存在）。
///
/// - `model` requires `provider`；`reasoning_effort` requires 两者；
/// - provider / model 必须在目录内（未列出 = 不存在）；
/// - **授权**由会话白名单决定：有 `subagent/model-selection-policy` 时必须在
///   列表内；无该事件时只允许父会话当前路由（`assertAllowedModelSelection` 语义）。
/// - effort 省略 → 模型 default effort；模型未声明 reasoning → 不允许带 effort。
fn resolve_route(
    catalog: &crate::provider_catalog::ProviderCatalog,
    policy: Option<&[crate::subagent::model_selection::AllowedModelRoute]>,
    parent_route: &crate::subagent::model_selection::AllowedModelRoute,
    provider: Option<&str>,
    model: Option<&str>,
    effort: Option<&str>,
) -> Result<Option<crate::model_target::ModelTarget>, String> {
    use crate::model_target::ModelTarget;
    let (provider, model) = match (provider, model) {
        (None, None) => {
            if effort.is_some() {
                return Err("`reasoning_effort` requires `provider` and `model`".to_string());
            }
            return Ok(None);
        }
        (None, Some(_)) => return Err("`model` requires `provider`".to_string()),
        (Some(_), None) => return Err("`provider` requires `model`".to_string()),
        (Some(p), Some(m)) => (p, m),
    };
    let entry = catalog
        .providers
        .iter()
        .find(|p| p.id == provider)
        .ok_or_else(|| {
            let available = if catalog.providers.is_empty() {
                "(none)".to_string()
            } else {
                catalog
                    .providers
                    .iter()
                    .map(|p| p.id.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            format!(
                "LLM provider \"{provider}\" is not registered; available providers: {available}"
            )
        })?;
    let model_entry = entry
        .models
        .iter()
        .find(|m| m.id == model)
        .ok_or_else(|| {
            format!("child LLM route \"{provider}/{model}\" is not allowed for this Session")
        })?;
    // 授权判定：白名单优先，缺失时只允许父路由。
    crate::subagent::model_selection::assert_allowed_model_selection(
        policy,
        Some(parent_route),
        &crate::subagent::model_selection::AllowedModelRoute::new(provider, model),
    )?;
    let effort_id = match (effort, &model_entry.reasoning) {
        (Some(e), Some(meta)) => {
            if !meta.efforts.iter().any(|x| x.id == e) {
                return Err(format!(
                    "reasoning effort \"{e}\" is not advertised by {provider}/{model}"
                ));
            }
            Some(e.to_string())
        }
        (Some(_), None) => {
            return Err(format!(
                "model {provider}/{model} does not advertise reasoning efforts; omit `reasoning_effort`"
            ))
        }
        (None, Some(meta)) => meta.default_effort.clone(),
        (None, None) => None,
    };
    Ok(Some(ModelTarget {
        model: model.to_string(),
        base_url: entry.base_url.clone(),
        api_key: entry.api_key.clone(),
        effort: effort_id,
    }))
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// 委派路由预检（`preflightChildLlmRoute` 语义）：显式选择路由时，在创建
/// 子会话**之前**用一次真实请求验证该路由可用（provider 可达 + 模型被接受），
/// 避免子代理在首个请求才失败、把错误埋进子会话日志。
///
/// 只对显式路由生效：继承父路由时父会话已在用该路由，重复探测纯属浪费。
async fn preflight_route(target: &crate::model_target::ModelTarget) -> Result<(), String> {
    let url = format!("{}/chat/completions", target.base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|error| format!("child LLM route preflight failed: {error}"))?;
    let response = client
        .post(url)
        .bearer_auth(&target.api_key)
        .json(&serde_json::json!({
            "model": target.model,
            "messages": [{ "role": "user", "content": "ping" }],
            "max_tokens": 1,
        }))
        .send()
        .await
        .map_err(|error| {
            format!(
                "child LLM route \"{}\" preflight failed: {error}",
                target.model
            )
        })?;
    if response.status().is_success() {
        return Ok(());
    }
    let status = response.status();
    let detail = response.text().await.unwrap_or_default();
    let detail: String = detail.chars().take(300).collect();
    Err(format!(
        "child LLM route \"{}\" preflight failed: HTTP {status}: {detail}",
        target.model
    ))
}

/// 工具族载体：持有共享句柄，注册 5 工具进注册表。
pub struct SubagentToolSet {
    handles: SubagentHandles,
}

impl SubagentToolSet {
    /// 构造（宿主持有，装配期注册 6 工具进注册表）。
    ///
    /// `parent_route` = 父会话当前 provider/model（无白名单时显式选择只允许它）。
    /// 会话白名单在 `wire_parent` 时从 durable 日志投影（装配期日志尚未建立）。
    pub fn new(
        runtime: Arc<SubagentRuntime>,
        catalog: crate::provider_catalog::ProviderCatalog,
        parent_route: crate::subagent::model_selection::AllowedModelRoute,
        preset_choices: Vec<String>,
    ) -> Self {
        let agents: Arc<Mutex<HashMap<String, SubagentActivation>>> = Arc::new(Mutex::new(HashMap::new()));
        // settle 钩子：subagent settle 时把活跃表状态翻转为 inactive
        // （「inactive = 当前没有 turn 在执行」）。父会话的唤醒由 settle 回投
        // inbox + `parent_wake` 负责（wait_agent 等的是 inbox，不是 per-agent 通知）。
        runtime.add_settle_hook({
            let agents = agents.clone();
            Arc::new(move |id| {
                if let Some(activation) = lock(&agents).get_mut(id) {
                    activation.status = crate::subagent::types::AgentStatus::Inactive;
                }
            })
        });
        Self {
            handles: SubagentHandles {
                runtime,
                parent_inbox: Arc::new(OnceLock::new()),
                parent_log: Arc::new(OnceLock::new()),
                agents,
                catalog,
                parent_route,
                policy: Arc::new(OnceLock::new()),
                preset_choices,
            },
        }
    }

    /// 延迟接线：父会话 Runtime 装配完成后调用（inbox / SessionLog 存在后）。
    pub fn wire_parent(&self, inbox: Arc<Inbox>, log: Arc<SessionLog>) {
        // subagent 生命周期事件（subagent/start、subagent/end）经父会话 live 总线扇出。
        self.handles.runtime.set_parent_log(log.clone());
        // 会话级路由白名单：从 durable 日志投影（冷恢复同样生效）。
        let _ = self
            .handles
            .policy
            .set(crate::subagent::model_selection::policy_from_events(&log.snapshot()));
        let _ = self.handles.parent_inbox.set(inbox);
        let _ = self.handles.parent_log.set(log);
    }

    /// 取会话白名单（未接线时视为未记录 → 只允许父路由）。
    pub fn policy(&self) -> Option<Vec<crate::subagent::model_selection::AllowedModelRoute>> {
        self.handles.policy.get().cloned().flatten()
    }

    /// 是否有 subagent 在飞（goal 自动续轮与 subagent 互斥判据）。
    /// 显式暴露计数而非依赖注释保证（子代理互斥显式化）。
    ///
    /// 活跃表在 settle 后保留句柄（continuable：仍可续聊），故此处按状态而非
    /// 表项是否为空判定。
    pub fn has_active_agents(&self) -> bool {
        lock(&self.handles.agents)
            .values()
            .any(|activation| activation.status == crate::subagent::types::AgentStatus::Running)
    }

    /// 注册 6 工具进注册表。
    pub fn register_all(&self, registry: &mut crate::tools::ToolRegistry) {
        self.register_all_impl(registry, true);
    }

    /// 注册工具，可选是否包含共享名控制三件（send_message / list_agents /
    /// interrupt_agent）。
    ///
    /// agent-team 启用时（`ONION_AGENT_TEAM=1`）`include_control=false`，让位给
    /// Team 语义（同名工具互斥——`tool-subagent-control` 与
    /// `tool-agent-team` 独立包同名工具互斥）。subagent 派生（subagent /
    /// subagent_fork）与 list_subagent_models 无冲突，恒注册。
    pub fn register_all_team_compat(&self, registry: &mut crate::tools::ToolRegistry) {
        self.register_all_impl(registry, false);
    }

    fn register_all_impl(&self, registry: &mut crate::tools::ToolRegistry, include_control: bool) {
        let handles = self.handles.clone();
        registry.register(Arc::new(SubagentSpawnTool { handles: handles.clone() }));
        registry.register(Arc::new(SubagentForkTool { handles }));
        if include_control {
            let handles = self.handles.clone();
            registry.register(Arc::new(SubagentSendMessageTool { handles }));
            registry.register(Arc::new(SubagentInterruptTool { handles: self.handles.clone() }));
            registry.register(Arc::new(SubagentListTool { handles: self.handles.clone() }));
            registry.register(Arc::new(SubagentWaitTool { handles: self.handles.clone() }));
        }
        registry.register(Arc::new(ListSubagentModelsTool { handles: self.handles.clone() }));
    }
}

/// 委派入参 schema（modelSelection + name / description + agent_preset；fork 同构）。
///
/// `agent_preset` 仅在配置了预设白名单时注入（未配置白名单即
/// 不暴露该字段）；白名单为空时模型无法选择预设。
fn delegation_parameters(preset_choices: &[String]) -> serde_json::Value {
    let mut properties = json!({
        "prompt": { "type": "string", "description": "task prompt for the subagent" },
        "description": { "type": "string", "description": "A short (3-5 word) description of the delegated task, for display." },
        "name": { "type": "string", "description": "Optional short name for the subagent, unique among the subagents you create: lower-case letters, digits, and hyphens, starting with a letter, at most 32 characters; lead is reserved for the root agent. Omitted names default to agent-1, agent-2, and so on." },
        "provider": { "type": "string", "description": "Child LLM provider id; requires `model`." },
        "model": { "type": "string", "description": "Model id within that provider; requires `provider`." },
        "reasoning_effort": { "type": "string", "description": "Reasoning effort id; requires `provider` and `model`. Omit to use the selected model's default." },
        "cwd": { "type": "string", "description": "Working directory for the subagent session. Relative paths resolve against the parent's current directory and must stay inside the permission root. Omit to inherit the parent's current directory." }
    });
    if !preset_choices.is_empty() {
        properties["agent_preset"] = json!({
            "type": "string",
            "enum": preset_choices,
            "description": "Agent preset for this child (tools and persona). Omit to inherit the parent's preset."
        });
    }
    json!({
        "type": "object",
        "properties": properties,
        "required": ["prompt"]
    })
}

/// 解析委派入参：prompt 必填，name/description/provider/model/reasoning_effort/cwd/agent_preset 可选。
fn parse_delegation(
    input: &Value,
    catalog: &crate::provider_catalog::ProviderCatalog,
    policy: Option<&[crate::subagent::model_selection::AllowedModelRoute]>,
    parent_route: &crate::subagent::model_selection::AllowedModelRoute,
    preset_choices: &[String],
) -> Result<(String, crate::subagent::runtime::DelegationOptions), String> {
    let prompt = input.get("prompt").and_then(Value::as_str).unwrap_or("").to_string();
    if prompt.is_empty() {
        return Err("prompt is required".to_string());
    }
    let string_field = |key: &str| {
        input
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    let route = resolve_route(
        catalog,
        policy,
        parent_route,
        string_field("provider").as_deref(),
        string_field("model").as_deref(),
        string_field("reasoning_effort").as_deref(),
    )?;
    let preset = resolve_preset(string_field("agent_preset").as_deref(), preset_choices)?;
    let options = crate::subagent::runtime::DelegationOptions {
        name: string_field("name"),
        description: string_field("description"),
        route,
        cwd: string_field("cwd"),
        preset,
    };
    Ok((prompt, options))
}

/// 解析 `agent_preset`（预设选择解析）。
///
/// - 省略 → None（继承父会话部署默认预设）；
/// - 白名单为空（未配置 choices）→ 显式传值即报错（模型选择被禁用）；
/// - 不在白名单内 → 报错并列出可选值。
fn resolve_preset(requested: Option<&str>, choices: &[String]) -> Result<Option<String>, String> {
    let Some(id) = requested else {
        return Ok(None);
    };
    if choices.is_empty() {
        return Err(
            "agent_preset selection is disabled for this tool instance (omit agent_preset)"
                .to_string(),
        );
    }
    if !choices.iter().any(|c| c == id) {
        return Err(format!("agent_preset must be one of: {}", choices.join(", ")));
    }
    Ok(Some(id.to_string()))
}

/// subagent：全新上下文创建 subagent 会话（input {prompt, description, name, provider?, model?, reasoning_effort?}）。
struct SubagentSpawnTool {
    handles: SubagentHandles,
}

impl ToolPort for SubagentSpawnTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "subagent".to_string(),
            description: "Spawn a subagent session with a fresh context. This tool starts an independently managed subagent and immediately returns its name; the runtime automatically delivers its completion notice when it finishes. Use `send_message` to steer it while running. Omit `provider`, `model`, and `reasoning_effort` to use the parent's route.".to_string(),
            parameters: delegation_parameters(&self.handles.preset_choices),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let handles = self.handles.clone();
        Box::pin(async move {
            let (prompt, options) = parse_delegation(
                &call.input,
                &handles.catalog,
                handles.policy.get().and_then(|p| p.as_deref()),
                &handles.parent_route,
                &handles.preset_choices,
            )?;
            // 显式路由先做真实预检（路由可用性）。
            if let Some(route) = &options.route {
                preflight_route(route).await?;
            }
            let activation = handles.activate(prompt, false, options)?;
            Ok(vec![ContentBlock::Text {
                text: format!("started subagent {} ({})", activation.name, activation.id),
            }])
        })
    }
}

/// subagent_fork：继承父会话前缀投影创建 subagent 会话（input 同 subagent）。
struct SubagentForkTool {
    handles: SubagentHandles,
}

impl ToolPort for SubagentForkTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "subagent_fork".to_string(),
            description: "Spawn a subagent session inheriting the parent session's completed turns as its starting context. Returns the subagent's name immediately; a completion notice is delivered when it finishes. Note: changing the route can prevent reuse of the inherited conversation prefix.".to_string(),
            parameters: delegation_parameters(&self.handles.preset_choices),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let handles = self.handles.clone();
        Box::pin(async move {
            let (prompt, options) = parse_delegation(
                &call.input,
                &handles.catalog,
                handles.policy.get().and_then(|p| p.as_deref()),
                &handles.parent_route,
                &handles.preset_choices,
            )?;
            // 显式路由先做真实预检（路由可用性）。
            if let Some(route) = &options.route {
                preflight_route(route).await?;
            }
            let activation = handles.activate(prompt, true, options)?;
            Ok(vec![ContentBlock::Text {
                text: format!("forked subagent {} ({})", activation.name, activation.id),
            }])
        })
    }
}

/// 解析寻址键（模型可读标识是 `agent` 名）。
///
/// `agent` 优先，回落到历史字段 `agent_id`（旧会话/旧提示词仍在用会话 id）；
/// 两者皆空 → None。定位仍走 [`SubagentHandles::find_agent`]（name 或 id 均可）。
fn agent_key(input: &Value) -> Option<String> {
    ["agent", "agent_id"]
        .iter()
        .filter_map(|key| input.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(str::to_string)
}

/// send_message：向活跃 subagent 发消息。
///
/// `delivery` 对应 `SubagentDelivery`：`queue`（缺省）落 next-turn 等子代理
/// 下一轮消费；`steer` 落 next-step 插到其最近一步之前。
struct SubagentSendMessageTool {
    handles: SubagentHandles,
}

impl ToolPort for SubagentSendMessageTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "send_message".to_string(),
            description: "Send a message to a running subagent. delivery=\"queue\" (default) delivers it at the subagent's next turn; delivery=\"steer\" injects it before its current step.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "agent": { "type": "string", "description": "subagent name (or session id)" },
                    "message": { "type": "string", "description": "message text to send to the subagent" },
                    "delivery": {
                        "type": "string",
                        "enum": ["queue", "steer"],
                        "description": "queue (default) = next turn; steer = before the current step"
                    }
                },
                "required": ["agent", "message"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let handles = self.handles.clone();
        Box::pin(async move {
            let agent_id = agent_key(&call.input)
                .ok_or_else(|| "agent and message are required".to_string())?;
            let message = call.input.get("message").and_then(Value::as_str).unwrap_or("").to_string();
            if message.is_empty() {
                return Err("agent and message are required".to_string());
            }
            let delivery = crate::subagent::relay::Delivery::parse(
                call.input.get("delivery").and_then(Value::as_str),
            )
            .map_err(|e| e.to_string())?;
            let known = handles
                .find_agent(&agent_id)
                .ok_or_else(|| format!("unknown subagent: {agent_id}"))?;
            // 投递经 runtime：运行中 steer 并入当前轮，已 settle 则在同一 Runtime 上
            // 重新开轮（continuable `sendMessage` 语义），冷恢复由 runtime 兜底。
            let resumed = handles
                .runtime
                .send_or_restore(&known.id, crate::types::AgentMessage::user(message), delivery)
                .map_err(|e| e.to_string())?;
            // 与 runtime 句柄表对齐（冷恢复 / 续聊后名字、深度、在跑态以 runtime 为准）。
            let refreshed = handles
                .runtime
                .child_activation(&known.id)
                .unwrap_or(known.clone());
            {
                let mut agents = lock(&handles.agents);
                if let Some(entry) = agents.get_mut(&known.id) {
                    *entry = refreshed.clone();
                } else {
                    agents.insert(refreshed.id.clone(), refreshed.clone());
                }
            }
            let action = match (resumed, delivery) {
                (true, _) => "resumed",
                (false, crate::subagent::relay::Delivery::Steer) => "steered",
                (false, crate::subagent::relay::Delivery::Queue) => "queued",
            };
            Ok(vec![ContentBlock::Text {
                text: format!("message {action} to {}", refreshed.name),
            }])
        })
    }
}

/// interrupt_agent：取消 subagent（cancel token）。
struct SubagentInterruptTool {
    handles: SubagentHandles,
}

impl ToolPort for SubagentInterruptTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "interrupt_agent".to_string(),
            description: "Interrupt a running subagent session. Input: {\"agent\": string}.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "agent": { "type": "string", "description": "subagent name (or session id)" }
                },
                "required": ["agent"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let handles = self.handles.clone();
        Box::pin(async move {
            let agent_id = agent_key(&call.input)
                .ok_or_else(|| "agent is required".to_string())?;
            let activation = handles
                .find_agent(&agent_id)
                .ok_or_else(|| format!("unknown subagent: {agent_id}"))?;
            // 取消**当前轮**而非销毁子会话：句柄保留，之后仍可 send_message 续聊
            // （continuable：interrupt 只打断本轮）。
            handles
                .runtime
                .interrupt(&activation.id)
                .map_err(|e| e.to_string())?;
            Ok(vec![ContentBlock::Text { text: format!("subagent {} interrupted", activation.name) }])
        })
    }
}

/// wait_agent：等待父会话 inbox 出现新输入。
///
/// 语义（本工具据此实现，不再按单个 subagent 的 settle 等待）：
/// - 无目标参数：监听父会话 inbox，一旦有排队输入即返回 `{"timedOut":false}`——
///   子代理回报、完成通知、用户插话都走同一条 inbox 通道，唤醒来源天然全覆盖；
/// - 不消费输入：排队项留给 step 边界整批领取（`timedOut:false` 只表示「有输入
///   待下个 step 喂给模型」）；
/// - `timeout_ms` 非整数或越界（10s..=3600s）→ 报错，不做夹取（严格校验）；
/// - 取消（interrupt / 会话销毁）→ `Err`（与 `exec.signal` 一致）。
struct SubagentWaitTool {
    handles: SubagentHandles,
}

/// `wait_agent` 的超时边界（10s..=3600s，越界即报错不夹取）。
const WAIT_TIMEOUT_MIN_MS: u64 = 10_000;
const WAIT_TIMEOUT_MAX_MS: u64 = 3_600_000;
/// 省略 `timeout_ms` 时的默认等待时长（默认 30s）。
const WAIT_TIMEOUT_DEFAULT_MS: u64 = 30_000;

/// 解析 `wait_agent` 入参（严格校验）：
/// 不接受声明外字段（`additionalProperties: false`）、`timeout_ms` 必须是
/// 10s..=3600s 的整数（越界报错，不夹取），省略时取 30s。
fn parse_wait_args(input: &Value) -> Result<u64, String> {
    if let Some(object) = input.as_object() {
        for key in object.keys() {
            if key != "timeout_ms" {
                return Err(format!(
                    "unexpected argument `{key}` (allowed: timeout_ms); wait_agent waits for any new input"
                ));
            }
        }
    }
    match input.get("timeout_ms") {
        None | Some(Value::Null) => Ok(WAIT_TIMEOUT_DEFAULT_MS),
        Some(value) => match value.as_u64() {
            Some(ms) if (WAIT_TIMEOUT_MIN_MS..=WAIT_TIMEOUT_MAX_MS).contains(&ms) => Ok(ms),
            _ => Err(format!(
                "timeout_ms must be an integer within {WAIT_TIMEOUT_MIN_MS}..={WAIT_TIMEOUT_MAX_MS}"
            )),
        },
    }
}

impl ToolPort for SubagentWaitTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "wait_agent".to_string(),
            description: "Wait for input at your next step, including agent messages, completion notifications, and user steering. Returns immediately if new input is pending. `timedOut: false` means new input is pending. Pending input is delivered to the model after tool execution finishes. Input: {\"timeout_ms\"?: number}.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "timeout_ms": { "type": "integer", "description": "Wait duration in milliseconds, from 10000 through 3600000. Defaults to 30000." }
                },
                "required": []
            }),
        }
    }

    fn execute(&self, call: ToolCall, cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let handles = self.handles.clone();
        Box::pin(async move {
            let timeout_ms = parse_wait_args(&call.input)?;
            // 存活守卫：父会话 inbox / 日志未接线时无法等待，直接结构化失败而非静默超时。
            let inbox = handles
                .parent_inbox
                .get()
                .cloned()
                .ok_or_else(|| "wait_agent unavailable: subagent tools not wired (missing parent inbox)".to_string())?;
            let log = handles
                .parent_log
                .get()
                .cloned()
                .ok_or_else(|| "wait_agent unavailable: subagent tools not wired (missing parent log)".to_string())?;
            // 已有排队输入：立即返回（不消费——留给 step 边界整批领取）。
            if inbox.has_pending() {
                return Ok(vec![ContentBlock::Text { text: json!({ "timedOut": false }).to_string() }]);
            }
            // 先订阅再复查 pending：插入不会落在两者之间（订阅-复查同序）。
            let mut events = log.bus().subscribe();
            if inbox.has_pending() {
                return Ok(vec![ContentBlock::Text { text: json!({ "timedOut": false }).to_string() }]);
            }
            let wait = async {
                loop {
                    match events.recv().await {
                        Ok(crate::session::LiveEvent::InboxInserted { .. }) => return,
                        // 滞后与无关事件：继续等（滞后只丢历史，不丢「有输入」这一事实）。
                        Ok(_) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
                    }
                }
            };
            let timed_out = tokio::select! {
                () = wait => false,
                () = tokio::time::sleep(std::time::Duration::from_millis(timeout_ms)) => true,
                () = cancel.cancelled() => return Err("wait_agent aborted".to_string()),
            };
            Ok(vec![ContentBlock::Text { text: json!({ "timedOut": timed_out }).to_string() }])
        })
    }
}

/// list_agents：列出活跃 subagent（`name [status] — description` 表格）。
/// 渲染形态沿用 `list-agents.ts`。
struct SubagentListTool {
    handles: SubagentHandles,
}

impl ToolPort for SubagentListTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "list_agents".to_string(),
            description: "List your subagent tree: the root agent (`lead`) and every active continuable descendant. \
Each row is `name [status]`, optionally followed by ` — description`. \
running means it is working; inactive means no turn is executing; it does not indicate task completion, success, or failure. \
While you remain active, you will be notified when a direct child finishes; there is no need to keep checking its status. \
Use send_message to continue the conversation. Input: {}."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    fn execute(&self, _call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let handles = self.handles.clone();
        Box::pin(async move {
            let agents = lock(&handles.agents);
            // 根代理恒在首行（root 先于所有后代；本工具总在某轮内执行，
            // 故根代理此刻必为 running）。
            let mut rows = vec![format!(
                "{} [{}] — root agent",
                crate::subagent::types::ROOT_AGENT_NAME,
                crate::subagent::types::AgentStatus::Running.label()
            )];
            // 稳定顺序：HashMap 迭代无序，按 name 排序后再渲染。
            let mut entries: Vec<&SubagentActivation> = agents.values().collect();
            entries.sort_by(|a, b| a.name.cmp(&b.name));
            rows.extend(entries.into_iter().map(|activation| {
                let description = activation
                    .description
                    .as_deref()
                    .map(|d| format!(" — {d}"))
                    .unwrap_or_default();
                format!("{} [{}]{description}", activation.name, activation.status.label())
            }));
            Ok(vec![ContentBlock::Text { text: rows.join("\n") }])
        })
    }
}

/// list_subagent_models：列出子代理可用 LLM 路由。
///
/// 语义（目录即允许路由白名单）：
/// - 无参 → 列出所有 provider（`id — name`）；
/// - `provider` → 列出该 provider 的 models（`provider/model — name[: description]`）；
/// - `provider` + `model` → 检查精确模型 + 列出其 reasoning efforts（default 标注）。
///
/// 只读、非 deferred——发现不改当前 Agent 路由（"without changing
/// the current Agent"）。
struct ListSubagentModelsTool {
    handles: SubagentHandles,
}

impl ListSubagentModelsTool {
    /// 渲染一条 model 行（`provider/model — name[: description]`）。
    fn model_line(provider: &str, model: &crate::provider_catalog::ModelEntry) -> String {
        match &model.description {
            Some(desc) => format!("{provider}/{} — {}: {}", model.id, model.name, desc),
            None => format!("{provider}/{} — {}", model.id, model.name),
        }
    }

    /// 渲染一个 provider 的 efforts 块（尾部 `Reasoning efforts:` 段）。
    fn efforts_block(reasoning: Option<&crate::provider_catalog::ReasoningMeta>) -> String {
        let Some(reasoning) = reasoning else {
            return "(no advertised reasoning efforts)".to_string();
        };
        if reasoning.efforts.is_empty() {
            return "(no advertised reasoning efforts)".to_string();
        }
        let default = reasoning.default_effort.as_deref();
        let lines: Vec<String> = reasoning
            .efforts
            .iter()
            .map(|e| {
                let marker = if default == Some(e.id.as_str()) { " (default)" } else { "" };
                match &e.description {
                    Some(desc) => format!("{}{} — {}: {}", e.id, marker, e.name, desc),
                    None => format!("{}{} — {}", e.id, marker, e.name),
                }
            })
            .collect();
        lines.join("\n")
    }

    /// 无 provider：列出**被授权**的 provider（`id — name`）；空目录返回占位。
    ///
    /// 无白名单时只列父路由所属 provider。
    fn list_providers(
        catalog: &crate::provider_catalog::ProviderCatalog,
        policy: Option<&[crate::subagent::model_selection::AllowedModelRoute]>,
        parent_route: &crate::subagent::model_selection::AllowedModelRoute,
    ) -> String {
        let routes: Vec<crate::subagent::model_selection::AllowedModelRoute> = match policy {
            Some(routes) => routes.to_vec(),
            None => vec![parent_route.clone()],
        };
        let providers: Vec<&crate::provider_catalog::ProviderEntry> = catalog
            .providers
            .iter()
            .filter(|p| routes.iter().any(|r| r.provider == p.id))
            .collect();
        if providers.is_empty() {
            return "(no LLM providers)".to_string();
        }
        providers
            .iter()
            .map(|p| format!("{} — {}", p.id, p.name))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 定位 provider；目录外视为未注册（provider 诊断）。
    fn find_provider<'a>(
        catalog: &'a crate::provider_catalog::ProviderCatalog,
        provider: &str,
    ) -> Result<&'a crate::provider_catalog::ProviderEntry, String> {
        if let Some(entry) = catalog.providers.iter().find(|p| p.id == provider) {
            return Ok(entry);
        }
        let available = catalog
            .providers
            .iter()
            .map(|p| p.id.clone())
            .collect::<Vec<_>>()
            .join(", ");
        let available = if available.is_empty() { "(none)".to_string() } else { available };
        Err(format!(
            "LLM provider \"{provider}\" is not registered; available providers: {available}"
        ))
    }

    /// 有 provider 无 model：列出该 provider 下**被授权**的 models；空则占位。
    fn list_models(
        entry: &crate::provider_catalog::ProviderEntry,
        policy: Option<&[crate::subagent::model_selection::AllowedModelRoute]>,
        parent_route: &crate::subagent::model_selection::AllowedModelRoute,
    ) -> String {
        let routes: Vec<crate::subagent::model_selection::AllowedModelRoute> = match policy {
            Some(routes) => routes.to_vec(),
            None => vec![parent_route.clone()],
        };
        let models: Vec<&crate::provider_catalog::ModelEntry> = entry
            .models
            .iter()
            .filter(|m| routes.iter().any(|r| r.provider == entry.id && r.model == m.id))
            .collect();
        if models.is_empty() {
            return format!("(no advertised models for {})", entry.id);
        }
        models
            .iter()
            .map(|m| Self::model_line(&entry.id, m))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// provider + model：检查精确模型 + 列出 efforts；目录外模型视为不允许。
    fn inspect_model(
        entry: &crate::provider_catalog::ProviderEntry,
        model: &str,
    ) -> Result<String, String> {
        let Some(model_entry) = entry.models.iter().find(|m| m.id == model) else {
            return Err(format!(
                "child LLM route \"{}/{}\" is not allowed for this Session",
                entry.id, model
            ));
        };
        let line = Self::model_line(&entry.id, model_entry);
        let efforts = Self::efforts_block(model_entry.reasoning.as_ref());
        Ok(format!("{line}\nReasoning efforts:\n{efforts}"))
    }
}

impl ToolPort for ListSubagentModelsTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "list_subagent_models".to_string(),
            description: "Discover LLM routes for subagents without changing the current Agent. Call with no arguments to list registered providers, with `provider` to list its advertised models, or with `provider` and `model` to inspect that exact model and its reasoning efforts. Catalog membership is advisory: an adapter may accept an unlisted model id. Use the returned ids with a delegation tool's `provider`, `model`, and `reasoning_effort` fields.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "provider": { "type": "string", "description": "Registered LLM provider id. Omit to list providers." },
                    "model": { "type": "string", "description": "Exact model id to inspect. Requires provider; omit to list that provider's advertised models." }
                },
                "required": []
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let handles = self.handles.clone();
        let provider = call.input.get("provider").and_then(Value::as_str).map(str::to_string);
        let model = call.input.get("model").and_then(Value::as_str).map(str::to_string);
        Box::pin(async move {
            // 校验：model 需要 provider（「`model` requires `provider`」）。
            if model.is_some() && provider.is_none() {
                return Err("`model` requires `provider`".to_string());
            }
            if let Some(p) = &provider {
                if p.is_empty() {
                    return Err("`provider` must be non-empty".to_string());
                }
            }
            if let Some(m) = &model {
                if m.is_empty() {
                    return Err("`model` must be non-empty".to_string());
                }
            }
            let catalog = &handles.catalog;
            let policy = handles.policy.get().and_then(|p| p.as_deref());
            let parent_route = &handles.parent_route;
            let Some(provider) = provider else {
                return Ok(vec![ContentBlock::Text {
                    text: Self::list_providers(catalog, policy, parent_route),
                }]);
            };
            let entry = Self::find_provider(catalog, &provider)?;
            let Some(model) = model else {
                return Ok(vec![ContentBlock::Text {
                    text: Self::list_models(entry, policy, parent_route),
                }]);
            };
            Ok(vec![ContentBlock::Text { text: Self::inspect_model(entry, &model)? }])
        })
    }
}

#[cfg(test)]
#[path = "subagent_tests.rs"]
mod tests;
