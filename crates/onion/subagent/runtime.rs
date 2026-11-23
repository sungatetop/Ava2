//! subagent Runtime：进程单例 + Provider seam + 驱动循环。
//!
//! - Provider seam（宿主注入如何创建 subagent 会话）；
//! - spawn = 全新上下文；fork = 继承父会话前缀投影；
//! - 深度限制（`depth > max_depth` → `SubagentDepthError`；冷恢复不复活 subagent）；
//! - 驱动循环：subagent 会话同样走 Runtime::assemble 装配线 + Kernel 驱动，
//!   settle 后 relay/notice 回父会话 inbox。

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::config::Config;
use crate::context::Context;
use crate::inbox::Inbox;
use crate::runtime::Runtime;
use crate::session::SessionLog;
use crate::system_prompt::{
    PromptContext, SectionText, SystemPrompt, context_orders,
};
use crate::tools::ToolRegistry;

use super::relay::{final_message_text, notice, relay, Delivery};
use super::types::{AgentStatus, SubagentActivation, SubagentError};

/// 子代理权限范围声明。
///
/// 以「运行期上下文贡献」而非 system prompt 段注入，使父子会话共用同一套部署提示词；
/// 语义是：权限范围在派生时固定，越界操作会被自动拒绝（不是挂起等人审批），
/// 遇到拒绝不要重试，而应在回复中申明限制交父代理处理。
pub const SUBAGENT_DELEGATION_CONTEXT: &str =
    "You are a delegated subagent: your permission scope was fixed when you were started and cannot be \
     widened from inside this session — operations that require approval are rejected automatically. \
     When the task needs access beyond that scope, do not retry the denied operation; state the \
     limitation in your reply so the delegating agent can handle it.";

/// 子代理会话提示词：部署默认段 + 委派范围上下文（+ 可选 Agent 预设指令）。
///
/// `directive` 来自 `agent_preset` 选中的 Agent 定义（宿主工厂解析后传入），
/// preset persona 随预设生效：非空时注册为 `agent:directive` 段，
/// 紧跟 persona、先于工具政策。
fn child_prompts(config: &Config, directive: Option<&str>) -> SystemPrompt {
    let mut prompts = SystemPrompt::new(config);
    // 插值变量必须与父会话装配点保持同源（`assemble.rs` 注册 `model`）：
    // 宿主配置的 agent preset persona 可能写 `{{model}}`，子会话漏注册会让
    // 组装直接失败（开轮被拒 → 子代理"无输出"），而非渲染成空值。
    let model = config.model.clone();
    let _ = prompts.variable("model", Arc::new(move || Some(model.clone())));
    let _ = prompts.context(PromptContext {
        name: "subagent:delegation".to_string(),
        order: context_orders::SUBAGENT_DELEGATION,
        text: SectionText::Static(SUBAGENT_DELEGATION_CONTEXT.to_string()),
        interpolate: false,
        required: true,
    });
    if let Some(text) = directive.map(str::trim).filter(|s| !s.is_empty()) {
        let _ = prompts.section(crate::system_prompt::PromptSection {
            name: "agent:directive".to_string(),
            order: crate::system_prompt::section_orders::AGENT_DIRECTIVE,
            text: SectionText::Static(text.to_string()),
            interpolate: true,
            complete: false,
        });
    }
    prompts
}

/// subagent Provider seam：宿主注入如何创建 subagent 会话。
///
/// - `spawn`：全新上下文（输入只有 prompt）；
/// - `fork`：继承父会话 durable 日志投影出的前缀。
/// - `parent_inbox`：relay/notice 回投目标（父会话 inbox 句柄）。
pub trait SubagentProvider: Send + Sync {
    /// 全新上下文创建（spawn）。
    fn spawn(
        &self,
        prompt: String,
        depth: u32,
        parent_inbox: Option<Arc<Inbox>>,
    ) -> Result<SubagentActivation, SubagentError>;
    /// 继承父会话 completedTurnPrefix 前缀创建（fork）。
    fn fork(
        &self,
        parent: &SessionLog,
        prompt: String,
        depth: u32,
        parent_inbox: Option<Arc<Inbox>>,
    ) -> Result<SubagentActivation, SubagentError>;
}

/// 子会话组合：宿主为**每个** subagent 会话现场装配的一套产物。
///
/// 子代理 = 独立 Agent 组合：提示词、工具面（含各自的服务实例）
/// 与 listener 都由宿主工厂在子会话创建时新建，不共享父会话的会话级状态
/// （goal / plan / todo / terminal / 技能等服务必须各有一套，否则会串写父会话）。
pub struct ChildSession {
    /// 子会话提示词（部署默认段 + 工具政策 + 委派范围上下文）。
    pub prompts: SystemPrompt,
    /// 子会话工具面（每会话独立 `ToolState`，防状态串扰）。
    pub tools: ToolRegistry,
    /// 子会话附加 listener（如技能手势；宿主级 listener 走 `with_listener_factory`）。
    pub listeners: Vec<Arc<dyn crate::listener::Listener>>,
    /// Runtime 建成后的接线回调（拿到子会话 id：terminal owner / 后台作业绑定等）。
    pub wire: Option<Arc<dyn Fn(&str) + Send + Sync>>,
}

impl ChildSession {
    /// 最小组合：默认提示词（含委派范围上下文）+ 给定工具面，无附加 listener / 接线。
    pub fn new(config: &Config, tools: ToolRegistry) -> Self {
        Self::with_directive(config, tools, None)
    }

    /// 带 Agent 预设指令的组合（`agent_preset` 选中预设的 system_prompt）。
    pub fn with_directive(
        config: &Config,
        tools: ToolRegistry,
        directive: Option<&str>,
    ) -> Self {
        Self {
            prompts: child_prompts(config, directive),
            tools,
            listeners: Vec::new(),
            wire: None,
        }
    }
}

/// subagent Runtime：进程单例，持有装配材料。
///
/// - 深度计数（depth 由 tools/subagent.rs 传入）；
/// - 冷恢复不复活：Provider 不参与回放，冷恢复出的会话没有 Provider——
///   本实现中 Provider 仅存于父会话的工具面（由宿主在装配期注入），
///   不写进 durable 日志，回放侧（main.rs 回放器 / 测试）不注入 Provider，
///   天然满足「不复活」；
/// - 驱动循环：subagent 会话同样走 `Runtime::assemble` 装配线，
///   settle 后 relay / notice 回父会话 inbox。
pub struct SubagentRuntime {
    /// 深度上限（构造期注入，不扩 Config）。
    max_depth: u32,
    /// subagent 会话装配模板 Config（宿主注入，不读环境）。
    config: Config,
    llm: Arc<dyn crate::llm::LlmPort>,
    /// 子会话组合工厂（每个子会话现场装配一套 prompts / tools / listeners）。
    ///
    /// 入参：
    /// - `cwd`：委派指定的工作目录（`DelegationOptions.cwd` 原样透传，由宿主按
    ///   权限根解析校验）；`None` = 继承父会话当前目录。
    /// - `preset`：`agent_preset` 解析出的 Agent id（`None` = 继承父会话预设）。
    ///   宿主据此选定预设的工具面与 persona。
    ///
    /// 解析失败返回错误串。
    factory: Arc<dyn Fn(Option<&str>, Option<&str>) -> Result<ChildSession, String> + Send + Sync>,
    /// subagent 会话落盘日志目录（None = 内存态，测试用）。
    log_dir: Option<std::path::PathBuf>,
    /// 在飞 subagent 计数（create 递增、settle 递减；goal 续轮互斥判据源）。
    inflight: Arc<AtomicUsize>,
    /// subagent 会话 listener 工厂（宿主注入；None = 无 listener，工具调用不设准入审批）。
    ///
    /// 每个 subagent 会话各自新建 listener 集合（会话级授权表不跨会话），
    /// 但共享宿主的审批请求通道，裁决仍汇入同一入口。
    listener_factory: Option<Arc<dyn Fn() -> Vec<Arc<dyn crate::listener::Listener>> + Send + Sync>>,
    /// 默认代理名计数（省略 name 时默认 `agent-1`、`agent-2` …）。
    name_counter: AtomicUsize,
    /// settle 钩子（宿主注册：settle 时以 subagent 会话 id 回调，如活跃表状态翻转）。
    settle_hooks: Arc<std::sync::Mutex<Vec<Arc<dyn Fn(&str) + Send + Sync>>>>,
    /// 父会话唤醒回调（宿主注入）：settle 回投 inbox 后触发，负责在父会话空闲时
    /// 立刻起新一轮消费 relay（AgentManager 式唤醒语义）。
    ///
    /// 不注入时行为退化为「relay 滞留至下一次用户输入」。
    parent_wake: Arc<std::sync::Mutex<Option<Arc<dyn Fn() + Send + Sync>>>>,
    /// 父会话日志（宿主注入）：subagent 生命周期事件（start / end）的发车载体。
    ///
    /// 生命周期事件由父会话 carrier 发出：事件瞬时、不落盘，
    /// 前端据此即时渲染子代理卡片而非轮询 list_agents。未注入时不发事件。
    parent_log: Arc<std::sync::Mutex<Option<Arc<SessionLog>>>>,
    /// 父会话「是否空闲」探测器（宿主注入）：settle 投递据此选择 next-turn / next-step
    /// （父空闲 → queue，运行中 → steer）。
    parent_idle: Arc<std::sync::Mutex<Option<Arc<dyn Fn() -> bool + Send + Sync>>>>,
    /// 子会话在线句柄表（id → 运行时 + 回投目标 + 是否在跑）。
    ///
    /// 保留已 settle 的句柄，使 [`SubagentRuntime::send`] 能在同一 Runtime 上重新开轮
    /// （续聊）；会话 ended 后移除
    /// （不可续聊）。进程重启后本表为空，由 [`SubagentRuntime::restore`] 按日志冷恢复。
    children: Arc<std::sync::Mutex<HashMap<String, ChildEntry>>>,
}

/// 一个子会话的在线句柄（续聊 / interrupt / 冷恢复的定位依据）。
struct ChildEntry {
    runtime: Arc<Runtime>,
    /// 结果回投目标（父会话 inbox；`None` = 顶层，无回投）。
    parent_inbox: Option<Arc<Inbox>>,
    /// 生命周期事件（`subagent/start`、`subagent/end`）的发射目标：父会话日志。
    notify: Option<Arc<SessionLog>>,
    /// 模型可读代理名（续聊时随驱动任务透传，保证 settle / notice 归属一致）。
    name: String,
    /// 相对深度（派生时记录；冷恢复补登活跃表时回填）。
    depth: u32,
    /// 是否有驱动任务在跑（settle 后翻转，`send` 据此决定是否重新开轮）。
    running: bool,
}

/// 委派选项（委派工具的 `name` / `description` 与 modelSelection 路由）。
///
/// - `name`：模型可读代理名（省略时默认 `agent-N`）；
/// - `description`：任务简述（展示用，可省略）；
/// - `route`：子代理 LLM 路由覆盖（provider 目录解析出的 ModelTarget；None =
///   继承宿主路由）。目录即允许路由白名单（`ModelSelectionPolicy.routes`）；
/// - `cwd`：子会话初始工作目录（相对路径基于父会话当前目录解析，须落在权限根内；
///   None = 继承父会话当前目录）；
/// - `preset`：子会话 Agent 预设 id（工具面 + persona）。None = 继承父会话预设
///   （省略即沿用部署默认预设）。
#[derive(Clone, Debug, Default)]
pub struct DelegationOptions {
    pub name: Option<String>,
    pub description: Option<String>,
    pub route: Option<crate::model_target::ModelTarget>,
    pub cwd: Option<String>,
    pub preset: Option<String>,
}

impl SubagentRuntime {
    /// 进程单例。
    pub fn new(
        max_depth: u32,
        config: Config,
        llm: Arc<dyn crate::llm::LlmPort>,
        factory: Arc<dyn Fn(Option<&str>, Option<&str>) -> Result<ChildSession, String> + Send + Sync>,
        log_dir: Option<std::path::PathBuf>,
    ) -> Self {
        Self {
            max_depth,
            config,
            llm,
            factory,
            log_dir,
            inflight: Arc::new(AtomicUsize::new(0)),
            listener_factory: None,
            name_counter: AtomicUsize::new(0),
            settle_hooks: Arc::new(std::sync::Mutex::new(Vec::new())),
            parent_wake: Arc::new(std::sync::Mutex::new(None)),
            parent_log: Arc::new(std::sync::Mutex::new(None)),
            parent_idle: Arc::new(std::sync::Mutex::new(None)),
            children: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    /// 注入父会话「是否空闲」探测器（宿主装配期调用一次）。
    ///
    /// settle 投递按它选择分区（父空闲 → queue，运行中 → steer）：
    /// 空闲时落 next-turn 排队，运行中落 next-step 插队。未注入时视为**运行中**
    /// （保守选择：steer 保证父代理当前步骤立刻看到结果，不会滞留到下一轮）。
    pub fn set_parent_idle(&self, idle: Arc<dyn Fn() -> bool + Send + Sync>) {
        *self
            .parent_idle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(idle);
    }

    /// 取「父会话是否空闲」（未注入 → false = 视作运行中）。
    fn parent_idle(&self) -> bool {
        self.parent_idle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .is_some_and(|probe| probe())
    }

    /// 注入父会话唤醒回调（宿主装配期调用一次）。
    ///
    /// settle 回投 inbox 后调用，让父会话在 idle 状态下立即起新一轮消费 relay；
    /// 宿主实现应保证幂等（重复唤起由生成侧闸门丢弃）。
    pub fn set_parent_wake(&self, wake: Arc<dyn Fn() + Send + Sync>) {
        *self
            .parent_wake
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(wake);
    }

    /// 取「父会话是否空闲」探测器克隆（驱动任务持有，settle 时刻再求值）。
    fn parent_idle_probe(&self) -> Option<Arc<dyn Fn() -> bool + Send + Sync>> {
        self.parent_idle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// 取当前唤醒回调克隆（驱动任务持有，避免生命周期耦合）。
    fn parent_wake(&self) -> Option<Arc<dyn Fn() + Send + Sync>> {
        self.parent_wake
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// 注入父会话日志（宿主装配期调用一次）：subagent 生命周期事件的发车载体。
    ///
    /// 生命周期事件**不落盘**（瞬时 observe-only），只经父会话 live 总线扇出，
    /// 与 `subagent/start` / `subagent/end` 事件同语义。
    pub fn set_parent_log(&self, log: Arc<SessionLog>) {
        *self
            .parent_log
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(log);
    }

    /// 取父会话日志克隆（驱动任务持有）。
    fn parent_log(&self) -> Option<Arc<SessionLog>> {
        self.parent_log
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// 注入 subagent 会话 listener 工厂（宿主装配期调用）。
    ///
    /// subagent 由模型自主派生，其工具调用必须与父会话接受同样的准入审批：
    /// 由宿主注入 `ApprovalGate` 等 listener，保证 shell / 越界写入不绕过审批。
    pub fn with_listener_factory(
        mut self,
        factory: Arc<dyn Fn() -> Vec<Arc<dyn crate::listener::Listener>> + Send + Sync>,
    ) -> Self {
        self.listener_factory = Some(factory);
        self
    }

    /// 为新建的 subagent Runtime 挂载宿主注入的 listener。
    fn wire_listeners(&self, runtime: &Runtime) {
        if let Some(factory) = &self.listener_factory {
            for listener in factory() {
                runtime.add_listener(listener);
            }
        }
    }

    /// 当前在飞 subagent 计数（>0 表示有 subagent 运行中，goal 续轮互斥）。
    pub fn inflight(&self) -> usize {
        self.inflight.load(Ordering::SeqCst)
    }

    /// 注册 settle 钩子（settle 后以 subagent 会话 id 回调；宿主用于同步
    /// 活跃表状态等展示侧事实，不影响运行时驱动）。
    pub fn add_settle_hook(&self, hook: Arc<dyn Fn(&str) + Send + Sync>) {
        self.settle_hooks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(hook);
    }

    /// 深度限制检查：`depth > max_depth` → `SubagentDepthError`。
    fn check_depth(&self, depth: u32) -> Result<u32, SubagentError> {
        if depth > self.max_depth {
            return Err(SubagentError::Depth { depth, max: self.max_depth });
        }
        Ok(depth)
    }

    /// spawn：全新 SessionLog + Runtime 装配，驱动一轮 turn，settle 后 relay 回父会话。
    pub fn spawn(
        &self,
        prompt: String,
        depth: u32,
        parent_inbox: Option<Arc<Inbox>>,
    ) -> Result<SubagentActivation, SubagentError> {
        self.spawn_delegated(prompt, depth, parent_inbox, DelegationOptions::default())
    }

    /// 带委派选项的 spawn（modelSelection / name / description）。
    pub fn spawn_delegated(
        &self,
        prompt: String,
        depth: u32,
        parent_inbox: Option<Arc<Inbox>>,
        options: DelegationOptions,
    ) -> Result<SubagentActivation, SubagentError> {
        let depth = self.check_depth(depth)?;
        self.create(prompt, None, depth, parent_inbox, options)
    }

    /// ralph 固定循环专用：全新上下文驱动一轮 turn 并等待 settle，返回最终
    /// assistant 文本（每轮 fresh structured child 语义）。
    ///
    /// 与 `spawn` 不同：本方法同步等待子代理完成（不是 fire-and-forget），
    /// 供 ralph 工具逐轮捕获有界结构化报告后再开启下一轮。
    pub async fn run_round(&self, prompt: String, depth: u32) -> Result<String, SubagentError> {
        self.check_depth(depth)?;
        let config = self.config.clone();
        // ralph 每轮 fresh child：不指定 cwd / preset（继承父会话）。
        let child = (self.factory)(None, None).map_err(SubagentError::Factory)?;
        // 工具面进请求上下文（Kernel 每步投影以构造时的 context.tools 为准，
        // 缺省 Context 无工具——对齐宿主装配点 `tools: tools.definitions()`）。
        let context = Context { system: None, messages: Vec::new(), tools: child.tools.definitions() };
        let runtime = match &self.log_dir {
            Some(dir) => Runtime::new(config, child.prompts, context, dir, self.llm.clone(), child.tools)?,
            None => Runtime::in_memory(config, child.prompts, context, self.llm.clone(), child.tools),
        };
        // 宿主注入的 listener（工具准入审批等）与父会话同源，子代理不得绕过。
        self.wire_listeners(&runtime);
        for listener in child.listeners {
            runtime.add_listener(listener);
        }
        if let Some(wire) = &child.wire {
            wire(runtime.session().session_id());
        }
        self.inflight.fetch_add(1, Ordering::SeqCst);
        let _ = runtime.run(vec![crate::types::AgentMessage::user(prompt)]).await;
        self.inflight.fetch_sub(1, Ordering::SeqCst);
        // 从 durable 日志投影出最终 assistant 消息（「model-visible means logged」）。
        let final_message = runtime
            .session()
            .snapshot()
            .iter()
            .rev()
            .find_map(|event| match event {
                crate::session::SessionEvent::AssistantMessage { message, .. } => {
                    Some(message.clone())
                }
                _ => None,
            });
        Ok(final_message
            .map(|m| {
                m.blocks
                    .iter()
                    .filter_map(|b| match b {
                        crate::types::ContentBlock::Text { text } => Some(text.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default())
    }

    /// fork：父会话 durable 日志投影出前缀，作为 subagent 初始 context。
    pub fn fork(
        &self,
        parent: &SessionLog,
        prompt: String,
        depth: u32,
        parent_inbox: Option<Arc<Inbox>>,
    ) -> Result<SubagentActivation, SubagentError> {
        self.fork_delegated(parent, prompt, depth, parent_inbox, DelegationOptions::default())
    }

    /// 带委派选项的 fork（modelSelection / name / description）。
    pub fn fork_delegated(
        &self,
        parent: &SessionLog,
        prompt: String,
        depth: u32,
        parent_inbox: Option<Arc<Inbox>>,
        options: DelegationOptions,
    ) -> Result<SubagentActivation, SubagentError> {
        let depth = self.check_depth(depth)?;
        self.create(prompt, Some(parent), depth, parent_inbox, options)
    }

    /// spawn / fork 共同实现：装配 subagent 会话 + 驱动循环（drive_subagent）。
    fn create(
        &self,
        prompt: String,
        parent_log: Option<&SessionLog>,
        depth: u32,
        parent_inbox: Option<Arc<Inbox>>,
        options: DelegationOptions,
    ) -> Result<SubagentActivation, SubagentError> {
        let config = self.config.clone();
        // 委派方式（`delegationMode`）：由**传入的**父日志决定，
        // 与下面注入的父会话通知目标（`self.parent_log()`）无关，故先取值。
        let delegation_mode = if parent_log.is_some() { "fork" } else { "spawn" };
        // 委派 cwd / preset 透传给宿主工厂（宿主按权限根解析 cwd、按预设装配
        // 工具面与 persona；None = 各自继承父会话）。
        let child = (self.factory)(options.cwd.as_deref(), options.preset.as_deref())
            .map_err(SubagentError::Factory)?;
        let child_tools = child.tools.definitions();
        let context = match parent_log {
            // fork：父会话 durable 日志投影出前缀（completedTurnPrefix 语义），
            // 工具面 = 子会话组合的 definitions（Kernel 每步投影依据，缺省为空即无工具）。
            Some(parent) => Context::project(&parent.snapshot(), child_tools),
            // spawn：全新上下文（输入只有 prompt），工具面同上。
            None => Context { system: None, messages: Vec::new(), tools: child_tools },
        };
        // 路由覆盖（modelSelection）：Some 时为子会话构造独立 LLM 目标
        // （静态 ModelTargetRef），不影响父会话 /model 切换；None = 共享宿主 LLM。
        // 描述符字段先快照（route / options 随后被消费）。
        let descriptor_model =
            options.route.as_ref().map(|t| t.model.clone()).unwrap_or_else(|| config.model.clone());
        let descriptor_effort = options.route.as_ref().and_then(|t| t.effort.clone());
        let descriptor_cwd = options.cwd.clone();
        let descriptor_label = options.description.clone();
        let llm: Arc<dyn crate::llm::LlmPort> = match options.route {
            Some(target) => Arc::new(
                crate::llm::OpenAiSseLlm::new(target.base_url.clone(), target.api_key.clone())
                    .with_target(Arc::new(crate::model_target::ModelTargetRef::new(target))),
            ),
            None => self.llm.clone(),
        };
        let runtime = match &self.log_dir {
            Some(dir) => Runtime::new(config, child.prompts, context, dir, llm, child.tools)?,
            None => Runtime::in_memory(config, child.prompts, context, llm, child.tools),
        };
        // 宿主注入的 listener（工具准入审批等）与父会话同源，子代理不得绕过。
        self.wire_listeners(&runtime);
        for listener in child.listeners {
            runtime.add_listener(listener);
        }
        if let Some(wire) = &child.wire {
            wire(runtime.session().session_id());
        }
        // 驱动循环：tokio::spawn 驱动 Runtime::run 循环直至 settle → relay + notice。
        let agent_id = runtime.session().session_id().to_string();
        // runId：一次运行的唯一标识，start / end 共用。
        let run_id = format!("{agent_id}-{}", crate::session::epoch_millis());
        let name = options.name.unwrap_or_else(|| {
            format!("agent-{}", self.name_counter.fetch_add(1, Ordering::SeqCst) + 1)
        });
        let activation = SubagentActivation {
            id: agent_id.clone(),
            name: name.clone(),
            description: options.description,
            depth,
            status: AgentStatus::Running,
            cancel: runtime.cancel_handle(),
            inbox: Some(runtime.inbox()),
        };
        let parent_log = self.parent_log();
        let parent_session_id = parent_log
            .as_ref()
            .map(|log| log.session_id().to_string())
            .unwrap_or_default();
        // 子会话继承父会话的模型选择白名单（子会话
        // 从父会话投影复制 policy；父会话无 policy 时子会话也不写）。
        if let Some(parent) = &parent_log {
            let inherited = crate::subagent::model_selection::policy_from_events(&parent.snapshot());
            if let Err(error) = crate::subagent::model_selection::seed_policy(
                &runtime.session(),
                inherited.as_deref(),
            ) {
                return Err(SubagentError::Invalid(error));
            }
        }
        // subagent/descriptor：durable 描述符写入**子会话**日志（首个请求之前，
        // 首个 turn 内 append 一次）。log-only：
        // 不进模型上下文，但冷恢复后可据此枚举子代理元信息。
        runtime.session().append(crate::session::SessionEvent::SubagentDescriptor {
            version: crate::session::SUBAGENT_DESCRIPTOR_VERSION,
            mode: "continuable".to_string(),
            provider: "session".to_string(),
            name: Some(name.clone()),
            label: descriptor_label,
            agent_model: Some(descriptor_model),
            agent_reasoning_effort: descriptor_effort,
            cwd: descriptor_cwd,
            parent_session_id: parent_session_id.clone(),
            depth,
            delegation_mode: Some(delegation_mode.to_string()),
        });
        // 委派审批预设（`appendDelegatedPolicyOverrides` 语义）：父会话记录过
        // 预设时，把该预设以 `source: 'delegation'` 播种进**子会话自己的**日志，
        // 使子会话的有效审批配置可由其日志单独重建（冷恢复不再依赖父进程内存）。
        // 子会话与父会话共享同一审批通道（`listener_factory`），因此播种值即父
        // 会话此刻的有效预设；父会话未记录预设时不写（走装配期默认）。
        if let Some(parent) = &parent_log {
            if let Some(preset) = crate::approval::preset_from_events(&parent.snapshot()) {
                runtime.session().append(crate::session::SessionEvent::ApprovalPreset {
                    preset,
                    source: Some(crate::approval::DELEGATION_SOURCE.to_string()),
                });
            }
        }
        // subagent/start：瞬时生命周期事件（父会话载体发出，不落盘）。
        if let Some(log) = &parent_log {
            log.emit_live(crate::session::LiveEvent::SubagentStart {
                run_id: run_id.clone(),
                agent_id: agent_id.clone(),
                name: name.clone(),
                depth,
                parent_session_id: parent_session_id.clone(),
            });
        }
        let identity = drive_identity(
            run_id,
            agent_id.clone(),
            name.clone(),
            depth,
            parent_session_id,
            parent_log.clone(),
        );
        // 登记在线句柄：settle 后仍保留，`send` 可在同一 Runtime 上续聊。
        let runtime = Arc::new(runtime);
        lock(&self.children).insert(
            agent_id,
            ChildEntry {
                runtime: runtime.clone(),
                parent_inbox: parent_inbox.clone(),
                notify: parent_log,
                name,
                depth,
                running: true,
            },
        );
        self.start_drive(
            runtime,
            vec![crate::types::AgentMessage::user(prompt)],
            parent_inbox,
            identity,
        );
        Ok(activation)
    }

    /// 在当前 Runtime 上驱动一轮（初次派生与续聊共用）。
    ///
    /// `input` 为空 = 续聊：本轮输入由子会话 inbox 的 next-step 批次提供
    /// （`Kernel::run` 的 claim 语义），因此 `send` 只需先 steer 再开轮。
    fn start_drive(
        &self,
        runtime: Arc<Runtime>,
        input: Vec<crate::types::AgentMessage>,
        parent_inbox: Option<Arc<Inbox>>,
        identity: DriveIdentity,
    ) {
        // 在飞计数递增（settle 时递减，见 drive_subagent）。
        self.inflight.fetch_add(1, Ordering::SeqCst);
        drive_subagent(
            runtime,
            input,
            parent_inbox,
            self.inflight.clone(),
            self.settle_hooks.clone(),
            self.parent_wake(),
            self.parent_idle_probe(),
            self.children.clone(),
            identity,
        );
    }

    /// 向子会话投递消息（按 `delivery` 入队 + 必要时重新开轮）。
    ///
    /// sendMessage 语义：
    /// - 运行中 → 按 `delivery` 并入队列（`steer` 当前步 / `queue` 下一轮），
    ///   不打断，返回 `Ok(false)`；
    /// - 已 settle → 在**同一个** Runtime 上重新开轮（续聊），返回 `Ok(true)`；
    /// - 未知 id 或会话已 ended（句柄已移除）→ [`SubagentError::Unknown`]。
    pub fn send(
        &self,
        agent_id: &str,
        message: crate::types::AgentMessage,
        delivery: Delivery,
    ) -> Result<bool, SubagentError> {
        let (runtime, parent_inbox, notify, name, depth, resume) = {
            let mut children = lock(&self.children);
            let entry = children
                .get_mut(agent_id)
                .ok_or_else(|| SubagentError::Unknown(agent_id.to_string()))?;
            let resume = !entry.running;
            entry.running = true;
            (
                entry.runtime.clone(),
                entry.parent_inbox.clone(),
                entry.notify.clone(),
                entry.name.clone(),
                entry.depth,
                resume,
            )
        };
        let pushed = match delivery {
            Delivery::Queue => runtime.inbox().followup(message),
            Delivery::Steer => runtime.inbox().steer(message),
        };
        if let Err(error) = pushed {
            // 投递失败（如分区满）：回滚 running 标记，避免子代理被误判为在跑而再也无法续聊。
            if let Some(entry) = lock(&self.children).get_mut(agent_id) {
                entry.running = !resume;
            }
            return Err(SubagentError::Inbox(error));
        }
        if resume {
            // 空输入 + inbox 唤醒：Kernel 以 next-turn / next-step 批次开轮。
            let run_id = format!("{agent_id}-{}", crate::session::epoch_millis());
            let identity = drive_identity(
                run_id,
                agent_id.to_string(),
                name,
                depth,
                notify
                    .as_ref()
                    .map(|log| log.session_id().to_string())
                    .unwrap_or_default(),
                notify,
            );
            self.start_drive(runtime, Vec::new(), parent_inbox, identity);
        }
        Ok(resume)
    }

    /// 冷恢复：按 id 从持久化日志重建已 settle 的子会话。
    ///
    /// 复用**同一个** session id（历史由 [`Runtime::resume`] 回放，续聊时模型记得
    /// 之前做过什么），只重建组合（提示词 / 工具面 / listener）。权限策略不重新捕获：
    /// 子会话日志里已有的准入事件即权威（策略在派生时固定）。
    ///
    /// 前置条件（不满足即 [`SubagentError::NotResumable`]）：落盘模式、日志可读、
    /// 且存在 `mode = continuable` 的 [`crate::session::SessionEvent::SubagentDescriptor`]。
    pub fn restore(&self, agent_id: &str) -> Result<(), SubagentError> {
        if lock(&self.children).contains_key(agent_id) {
            return Ok(());
        }
        let Some(dir) = &self.log_dir else {
            return Err(SubagentError::NotResumable(agent_id.to_string()));
        };
        // 只读回放（非写者）：真正的写者句柄由下方 Runtime::resume 打开并持有
        // 跨进程写租约；restore 自身不再抢同一会话的第二个写句柄。
        let log = SessionLog::replay_only(dir, agent_id)
            .map_err(|_| SubagentError::NotResumable(agent_id.to_string()))?;
        let snapshot = log.snapshot();
        // descriptor 只在自己的事件里找（fork 继承前缀由 kernel 投影，不参与判定）。
        let descriptor = snapshot.iter().rev().find_map(|event| match event {
            crate::session::SessionEvent::SubagentDescriptor { mode, name, depth, .. }
                if mode == "continuable" =>
            {
                Some((name.clone(), *depth))
            }
            _ => None,
        });
        let Some((name, depth)) = descriptor else {
            return Err(SubagentError::NotResumable(agent_id.to_string()));
        };
        let child = (self.factory)(None, None).map_err(SubagentError::Factory)?;
        let tools = child.tools;
        let context = Context::project(&snapshot, tools.definitions());
        let runtime = Runtime::resume(
            self.config.clone(),
            child.prompts,
            context,
            dir,
            agent_id,
            self.llm.clone(),
            tools,
        )?;
        // 宿主 listener 与父会话同源；子会话自己的 listener / 接线照常挂回。
        self.wire_listeners(&runtime);
        for listener in child.listeners {
            runtime.add_listener(listener);
        }
        if let Some(wire) = &child.wire {
            wire(agent_id);
        }
        let runtime = Arc::new(runtime);
        lock(&self.children).insert(
            agent_id.to_string(),
            ChildEntry {
                runtime,
                parent_inbox: None,
                notify: self.parent_log(),
                name: name.unwrap_or_else(|| agent_id.to_string()),
                depth,
                running: false,
            },
        );
        Ok(())
    }

    /// 投递消息，必要时先冷恢复（在线句柄命中 → 投递；
    /// 未命中 → restore → 投递）。
    pub fn send_or_restore(
        &self,
        agent_id: &str,
        message: crate::types::AgentMessage,
        delivery: Delivery,
    ) -> Result<bool, SubagentError> {
        if !lock(&self.children).contains_key(agent_id) {
            self.restore(agent_id)?;
        }
        self.send(agent_id, message, delivery)
    }

    /// 子会话代理名（冷恢复后补登活跃表 / 展示用）。
    pub fn child_name(&self, agent_id: &str) -> Option<String> {
        lock(&self.children).get(agent_id).map(|entry| entry.name.clone())
    }

    /// 子会话当前轮的取消令牌（interrupt 与展示侧共用；每轮开轮前换新）。
    pub fn cancel_handle(&self, agent_id: &str) -> Option<tokio_util::sync::CancellationToken> {
        lock(&self.children)
            .get(agent_id)
            .map(|entry| entry.runtime.cancel_handle())
    }

    /// 子会话激活句柄快照（宿主活跃表据此与 runtime 句柄表对齐；未知 id → None）。
    pub fn child_activation(&self, agent_id: &str) -> Option<SubagentActivation> {
        let children = lock(&self.children);
        let entry = children.get(agent_id)?;
        Some(SubagentActivation {
            id: agent_id.to_string(),
            name: entry.name.clone(),
            description: None,
            depth: entry.depth,
            status: if entry.running { AgentStatus::Running } else { AgentStatus::Inactive },
            cancel: entry.runtime.cancel_handle(),
            inbox: Some(entry.runtime.inbox()),
        })
    }

    /// 取消子会话当前轮（interrupt）。
    ///
    /// 取运行时的**当前轮**令牌（每轮开轮前换新），因此续聊之后仍能取消最新一轮；
    /// inbox 保留、句柄保留，cancel 后可继续 `send` 续聊。
    pub fn interrupt(&self, agent_id: &str) -> Result<(), SubagentError> {
        let runtime = {
            let children = lock(&self.children);
            children
                .get(agent_id)
                .map(|entry| entry.runtime.clone())
                .ok_or_else(|| SubagentError::Unknown(agent_id.to_string()))?
        };
        runtime.cancel_handle().cancel();
        Ok(())
    }
}

fn lock<T>(mutex: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// prepare 阶段捕获的运行标识（drive_subagent 结束后据此发 `subagent/end`）。
struct DriveIdentity {
    run_id: String,
    agent_id: String,
    name: String,
    depth: u32,
    parent_session_id: String,
    parent_log: Option<Arc<SessionLog>>,
}

fn drive_identity(
    run_id: String,
    agent_id: String,
    name: String,
    depth: u32,
    parent_session_id: String,
    parent_log: Option<Arc<SessionLog>>,
) -> DriveIdentity {
    DriveIdentity { run_id, agent_id, name, depth, parent_session_id, parent_log }
}

/// `TurnEndReason` → `stopReason` 词汇（completed / aborted / error / refusal / max-tokens）。
fn stop_reason_label(reason: &crate::kernel::TurnEndReason) -> &'static str {
    match reason {
        crate::kernel::TurnEndReason::Completed => "completed",
        crate::kernel::TurnEndReason::Cancelled => "aborted",
        crate::kernel::TurnEndReason::MaxTokens => "max-tokens",
        crate::kernel::TurnEndReason::Rejected(_) => "refusal",
    }
}

/// 驱动 subagent 会话：tokio::spawn 后台驱动。
///
/// - settle：Runtime::run 返回 TurnReport（Completed）→ relay 最终 assistant
///   消息 + notice settle 事实回父会话 inbox（None = 顶层，不回投）；
/// - 取消 / 错误：仅 notice（空消息，投影时被跳过），父会话只收到 settle 事实。
///
/// `input` 为空 = 续聊（输入由子会话 inbox 的 next-step 批次提供）。
/// settle 后翻转句柄表的 `running`（会话已 ended 则移除句柄，不再可续聊）。
fn drive_subagent(
    runtime: Arc<Runtime>,
    input: Vec<crate::types::AgentMessage>,
    parent_inbox: Option<Arc<Inbox>>,
    inflight: Arc<AtomicUsize>,
    settle_hooks: Arc<std::sync::Mutex<Vec<Arc<dyn Fn(&str) + Send + Sync>>>>,
    parent_wake: Option<Arc<dyn Fn() + Send + Sync>>,
    // 父会话空闲探测（在 settle 时刻求值：委派瞬间父代理必然在运行中，
    // 用陈旧值会把回投错误地打成 next-step）。
    parent_idle: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
    children: Arc<std::sync::Mutex<HashMap<String, ChildEntry>>>,
    identity: DriveIdentity,
) {
    tokio::spawn(async move {
        let report = runtime.run(input).await;
        // settle：在飞计数递减（无论 settle/取消/错误，subagent 均不再运行）。
        inflight.fetch_sub(1, Ordering::SeqCst);
        let agent_id = runtime.session().session_id().to_string();
        // settle：句柄表状态翻转（会话已 ended → 移除，之后 send 报未知）。
        let ended = runtime
            .session()
            .snapshot()
            .iter()
            .any(|event| matches!(event, crate::session::SessionEvent::SessionEnd { .. }));
        {
            let mut children = lock(&children);
            if ended {
                children.remove(&agent_id);
            } else if let Some(entry) = children.get_mut(&agent_id) {
                entry.running = false;
            }
        }
        // settle：宿主钩子（活跃表状态翻转等展示侧事实同步）。
        {
            let hooks = settle_hooks.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            for hook in hooks.iter() {
                hook(&agent_id);
            }
        }
        // 最终 assistant 消息投影（「model-visible means logged」）：既用于 relay，
        // 也用于 subagent/end 的摘要字段。
        let final_message = runtime.session().snapshot().iter().rev().find_map(|event| match event {
            crate::session::SessionEvent::AssistantMessage { message, .. } => Some(message.clone()),
            _ => None,
        });
        let final_text = final_message.as_ref().map(final_message_text);
        // 结束原因只算一次：live `subagent/end` 与 settle notice 共用同一词汇。
        let stop_reason = match &report {
            Ok(Some(report)) => stop_reason_label(&report.reason).to_string(),
            Ok(None) => "completed".to_string(),
            Err(_) => "error".to_string(),
        };
        // subagent/end：瞬时生命周期事件，与 start 以 run_id 配对（不落盘）。
        if let Some(log) = &identity.parent_log {
            log.emit_live(crate::session::LiveEvent::SubagentEnd {
                run_id: identity.run_id.clone(),
                agent_id: identity.agent_id.clone(),
                name: identity.name.clone(),
                depth: identity.depth,
                parent_session_id: identity.parent_session_id.clone(),
                stop_reason: stop_reason.clone(),
                last_assistant_message: final_text,
            });
        }
        // settle：settle 事实通知（notice）+ 结果投递（relay）回父会话 inbox。
        if let Some(inbox) = parent_inbox {
            // 投递分区（父空闲 → queue，运行中 → steer）：
            // 父代理空闲 → next-turn 排在下一轮；运行中 → next-step 插到最近一步前。
            let delivery = Delivery::for_parent_idle(
                parent_idle.as_ref().is_some_and(|probe| probe()),
            );
            let closing_delivered = final_message.is_some();
            if let Some(message) = final_message {
                let _ = relay(&inbox, &agent_id, message, delivery);
            }
            // notice：一行 settle 账目（含 stopReason / summary），relay 已投递收尾
            // 消息时不再重复其内容（omitClosingText 语义）。
            let _ = notice(
                &inbox,
                &agent_id,
                &identity.name,
                &stop_reason,
                true,
                closing_delivered,
                delivery,
            );
            // 父会话唤醒：relay/notice 均已入队后触发，父会话 idle 时立即起新一轮消费，
            // 不再滞留到下一次用户输入（AgentManager 式唤醒语义）。
            if let Some(wake) = &parent_wake {
                wake();
            }
        }
    });
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
