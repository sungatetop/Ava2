//! Runtime 门面：对外 run / end / with_listener / 事件订阅 / inbox 句柄。
//! 宿主经它投递消息、取消会话、订阅 live 事件。

use std::path::Path;
use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use arc_swap::ArcSwap;

use crate::config::Config;
use crate::context::Context;
use crate::inbox::Inbox;
use crate::kernel::{Kernel, TurnReport};
use crate::listener::{EventSink, Listener, ListenerCtx};
use crate::llm::LlmPort;
use crate::model_target::ModelTargetRef;
use crate::session::{LiveEvent, SessionLog};
use crate::spill::SpillStore;
use crate::state::{Machine, StateError};
use crate::system_prompt::SystemPrompt;
use crate::tools::ToolRegistry;
use crate::types::AgentMessage;

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// 同步扇出到全部 listener（异常被包含，不阻断发射方）。
fn fanout_contained(listeners: &[Arc<dyn Listener>], ctx: &ListenerCtx, event: &LiveEvent) {
    for listener in listeners {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            listener.on_event(ctx, event);
        }));
    }
}

pub struct Runtime {
    kernel: tokio::sync::Mutex<Kernel>,
    /// listener 注册表：with_listener 追加；运行开始时内核冻结快照，
    /// 事件 sink 在发射点动态读取（二者同源，不存在注册表漂移）。
    registry: Arc<Mutex<Vec<Arc<dyn Listener>>>>,
    inbox: Arc<Inbox>,
    session: Arc<SessionLog>,
    /// 当前轮取消令牌：每轮开轮前换新（取消按活动而非按会话）。
    cancel: Arc<ArcSwap<CancellationToken>>,
}

impl Runtime {
    /// 落盘日志（`.onion-demo/` 下 JSONL）。
    pub fn new(
        config: Config,
        prompts: SystemPrompt,
        context: Context,
        log_dir: &Path,
        llm: Arc<dyn LlmPort>,
        tools: ToolRegistry,
    ) -> std::io::Result<Self> {
        let session = Arc::new(SessionLog::open(log_dir)?);
        Self::assemble(config, prompts, context, session, llm, tools)
    }

    /// 内存态日志（测试用）。
    pub fn in_memory(
        config: Config,
        prompts: SystemPrompt,
        context: Context,
        llm: Arc<dyn LlmPort>,
        tools: ToolRegistry,
    ) -> Self {
        let session = Arc::new(SessionLog::in_memory());
        match Self::assemble(config, prompts, context, session, llm, tools) {
            Ok(runtime) => runtime,
            // assemble 只在 SessionLog::open 失败时返回 Err；in_memory 不会失败。
            Err(error) => panic!("in_memory runtime assemble failed: {error}"),
        }
    }

    /// 恢复既有会话：从 `<log_dir>/<session_id>.jsonl` 回放 durable 事件重建
    /// 内存投影（`SessionLog::open_with_replay`），其余装配同 `new`。
    /// 文件不存在时等价新建空会话。
    pub fn resume(
        config: Config,
        prompts: SystemPrompt,
        context: Context,
        log_dir: &Path,
        session_id: &str,
        llm: Arc<dyn LlmPort>,
        tools: ToolRegistry,
    ) -> std::io::Result<Self> {
        let session = Arc::new(SessionLog::open_with_replay(log_dir, session_id)?);
        Self::assemble(config, prompts, context, session, llm, tools)
    }

    fn assemble(
        config: Config,
        mut prompts: SystemPrompt,
        context: Context,
        session: Arc<SessionLog>,
        llm: Arc<dyn LlmPort>,
        tools: ToolRegistry,
    ) -> std::io::Result<Self> {
        let bus = session.bus().clone();
        let inbox = Arc::new(Inbox::new(bus.clone()));
        let registry: Arc<Mutex<Vec<Arc<dyn Listener>>>> = Arc::new(Mutex::new(Vec::new()));
        let mut machine = Machine::new(bus);
        Self::wire_sink(&registry, &inbox, &session, &mut machine);
        let cancel = Arc::new(ArcSwap::from_pointee(CancellationToken::new()));
        // 工作空间指令（config.workspace_enabled 且未显式关闭时，从 cwd 向上发现 AGENTS.md）。
        let workspace = if config.workspace_enabled {
            crate::workspace::Workspace::discover(&crate::workspace::workspace_cwd(), config.workspace_max_bytes).ok()
        } else {
            None
        };
        // 工具 schema provider：每次装配求值（装配期手动接线注册）。
        let registry_for_prompts = tools.clone();
        prompts.add_tool_provider(Arc::new(move || registry_for_prompts.definitions()));
        // 运行期模型目标：缺省从 config 静态三元组构造（assemble 期可经
        // `set_model_target` 置换为与 OpenAiSseLlm 共享的实例）。
        let model_target = Arc::new(ModelTargetRef::new(crate::model_target::ModelTarget {
            model: config.model.clone(),
            base_url: config.base_url.clone(),
            api_key: config.api_key.clone(),
            effort: None,
        }));
        let kernel = Kernel::new(
            config,
            Arc::new(prompts),
            context,
            session.clone(),
            inbox.clone(),
            machine,
            llm,
            model_target,
            tools,
            cancel.load_full().as_ref().clone(),
            workspace,
        );
        Ok(Self {
            kernel: tokio::sync::Mutex::new(kernel),
            registry,
            inbox,
            session,
            cancel,
        })
    }

    /// 装配同步事件 sink：SessionLog / Inbox / Machine 三处发射点共用，
    /// 在发射点同步invoke listener（异常被包含）。
    fn wire_sink(
        registry: &Arc<Mutex<Vec<Arc<dyn Listener>>>>,
        inbox: &Arc<Inbox>,
        session: &Arc<SessionLog>,
        machine: &mut Machine,
    ) {
        let sink_registry = registry.clone();
        let sink_inbox = inbox.clone();
        // Weak：sink 会存回 SessionLog（set_sink）与 Inbox，闭包若强捕获
        // Arc<SessionLog> 即自引用环——SessionLog 永不释放，写租约/文件句柄
        // 随之泄漏。发射点调用方（Runtime/SessionLog 自身）必持有强引用，
        // upgrade 失败 = 日志已释放，跳过扇出即可。
        let sink_session = Arc::downgrade(session);
        let sink: EventSink = Arc::new(move |event: &LiveEvent| {
            let listeners = lock(&sink_registry).clone();
            if listeners.is_empty() {
                return;
            }
            let Some(session) = sink_session.upgrade() else { return };
            let state = match event {
                LiveEvent::StateChanged { to, .. } => to.clone(),
                _ => String::new(),
            };
            let ctx = ListenerCtx { inbox: sink_inbox.clone(), session, state };
            fanout_contained(&listeners, &ctx, event);
        });
        session.set_sink(sink.clone());
        inbox.set_sink(sink.clone());
        machine.set_sink(sink);
    }

    /// 注册 listener（运行开始时由内核冻结快照）。
    pub fn with_listener(self, listener: Arc<dyn Listener>) -> Self {
        lock(&self.registry).push(listener);
        self
    }

    /// 追加 listener（`&self`，供已 Arc 包裹的 Runtime 在装配后动态注册；
    /// 与 `with_listener` 同源，二者不冲突）。
    pub fn add_listener(&self, listener: Arc<dyn Listener>) {
        lock(&self.registry).push(listener);
    }

    /// 驱动一轮 turn（输入为空时尝试领取 next-turn 排队消息；皆空返回 Ok(None)）。
    /// 内核互斥是刻意的串行语义：guard 伴随整轮 turn 持有（同一时刻至多一轮
    /// 在飞，单会话单飞行轮），故显式放行跨 await 持锁检查。
    ///
    /// 每轮开轮前换新取消令牌：上一轮被 `cancel` 的 token 不复用，会话因此
    /// 在 interrupt 后可继续使用（取消按活动而非按会话）。
    #[allow(clippy::await_holding_invalid_type)]
    pub async fn run(&self, input: Vec<AgentMessage>) -> Result<Option<TurnReport>, StateError> {
        let mut kernel = self.kernel.lock().await;
        let fresh = CancellationToken::new();
        self.cancel.store(Arc::new(fresh.clone()));
        kernel.set_cancel(fresh);
        let listeners = lock(&self.registry).clone();
        kernel.freeze_listeners(listeners);
        kernel.run(input).await
    }

    /// 结束会话：取消进行中工作，状态机置 Ended，发 durable SessionEnd。
    pub async fn end(&self, reason: &str) -> Result<(), StateError> {
        self.cancel.load().cancel();
        let mut kernel = self.kernel.lock().await;
        kernel.end_session(reason)
    }

    pub fn inbox(&self) -> Arc<Inbox> {
        self.inbox.clone()
    }

    pub fn session(&self) -> Arc<SessionLog> {
        self.session.clone()
    }

    /// live 事件订阅句柄。
    pub fn subscribe(&self) -> broadcast::Receiver<LiveEvent> {
        self.session.bus().subscribe()
    }

    /// 当前轮取消令牌（interrupt 用；每轮开轮前换新）。
    pub fn cancel_handle(&self) -> CancellationToken {
        self.cancel.load_full().as_ref().clone()
    }

    /// 置换运行期模型目标（assemble 期注入与 OpenAiSseLlm 共享的实例）。
    pub async fn set_model_target(&self, target: Arc<ModelTargetRef>) {
        self.kernel.lock().await.set_model_target(target);
    }

    /// 同步变体：供同步装配路径（`assemble_runtime`）在 Kernel 尚未被并发
    /// 共享时安全置换。用 `try_lock` 而非 `blocking_lock`——装配发生在
    /// `#[tokio::main]` 的驱动线程上，属于 runtime 上下文，`blocking_lock`
    /// 会 panic（Cannot block the current thread from within a runtime）；
    /// `try_lock` 无阻塞、任何上下文安全，装配期锁必空闲，被占即时序破坏
    /// （fail-loud 返回错误）。
    pub fn set_model_target_sync(&self, target: Arc<ModelTargetRef>) -> std::io::Result<()> {
        let mut kernel = self
            .kernel
            .try_lock()
            .map_err(|_| std::io::Error::other("kernel 锁被并发持有：装配期不应存在其它持有者"))?;
        kernel.set_model_target(target);
        Ok(())
    }

    /// 装配期注入 spill 工件库（同步锁版本；镜像 [`Self::set_model_target_sync`]）。
    pub fn set_spill_store_sync(&self, store: Arc<SpillStore>) -> std::io::Result<()> {
        let mut kernel = self
            .kernel
            .try_lock()
            .map_err(|_| std::io::Error::other("kernel 锁被并发持有：装配期不应存在其它持有者"))?;
        kernel.set_spill_store(store);
        Ok(())
    }

    /// 运行期模型目标引用（宿主 /model 读取 / 持久化）。
    pub async fn model_target(&self) -> Arc<ModelTargetRef> {
        self.kernel.lock().await.model_target()
    }
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
