//! 内核循环（洋葱的芯）：turn/step 状态机驱动 + 每边界事件发射 + 瀑布分发。
//!
//! 核心循环契约：
//! - step = 一次模型请求 + 其工具调用；turn = 零或多个 step；
//! - claim 即移除（reject 保持移除 = 持久取消）；
//! - 首个 step 空消息 = completed 空轮（不开 step、不发 StepStart）；
//! - 请求错误同 step 重装配重试，不重复提交 user（首轮尝试输入保持不变）；
//! - turn-stopping 串行无决策：listener 窗口内 steer，内核之后复查 next-step；
//! - 取消：提交前取消 system/user 均不提交；流中取消落定 AssistantAttempt；
//!   工具取消合成错误 ToolResult（维持 tool_use / tool_result 配对不变式）。

use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::Duration;

use futures_util::StreamExt;
use tokio_util::sync::CancellationToken;

use crate::config::Config;
use crate::compaction::estimate::{estimate_message, estimate_overhead};
use crate::compaction::prune::prune_view;
use crate::context::{Context, RequestDraft};
use crate::inbox::Inbox;
use crate::listener::{
    ClaimSource, ClaimedInput, Listener, ListenerCtx, PreStepDecision, RequestError, RetryDecision,
    StepRoute, ToolAdmission,
};
use crate::llm::{AssistantTurn, LlmPort, LlmStreamItem};
use crate::model_target::ModelTargetRef;
use crate::session::{LiveEvent, SessionEvent, SessionLog, StreamChunk};
use crate::spill::SpillStore;
use crate::state::{Machine, StateError};
use crate::system_prompt::{self, SystemPrompt};
use crate::tools::ToolRegistry;
use crate::types::{
    AgentMessage, MessageSource, StopReason, StreamDelta, ToolCall, ToolDefinition, ToolResult,
};
use crate::waterfall;
use crate::workspace::{Workspace, WorkspaceChanges};

pub struct TurnReport {
    pub turn: u32,
    pub steps: u32,
    pub reason: TurnEndReason,
    /// 本轮各步 usage 累计（轮次用量与上下文占比的展示数据源）。
    pub usage: crate::types::Usage,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TurnEndReason {
    Completed,
    Rejected(String),
    Cancelled,
    MaxTokens,
}

impl TurnReport {
    pub fn reason_label(&self) -> String {
        self.reason.label()
    }
}

impl TurnEndReason {
    fn label(&self) -> String {
        match self {
            Self::Completed => "completed".to_string(),
            Self::Rejected(reason) => format!("rejected: {reason}"),
            Self::Cancelled => "cancelled".to_string(),
            Self::MaxTokens => "max-tokens".to_string(),
        }
    }
}

/// 内核：持有状态机、上下文投影、inbox、端口与 listener 快照。
pub struct Kernel {
    config: Config,
    /// 提示词注册表：开轮装配段 / 上下文 / 工具。
    prompts: Arc<SystemPrompt>,
    context: Context,
    session: Arc<SessionLog>,
    inbox: Arc<Inbox>,
    machine: Machine,
    llm: Arc<dyn LlmPort>,
    /// 运行期模型目标引用（/model 切换；prompt-assembly 边界快照 current→assembled）。
    model_target: Arc<ModelTargetRef>,
    tools: ToolRegistry,
    cancel: CancellationToken,
    listeners: Vec<Arc<dyn Listener>>,
    turn_count: u32,
    /// 本轮装配的 system 文本（首步提交一次；空文本不提交）。
    pending_system: Option<String>,
    /// 本轮装配的工具表（规范排序后覆盖每步请求草稿）。
    pending_tools: Option<Vec<ToolDefinition>>,
    /// 工作空间指令（None = 未启用；Some = 已发现待注入）。
    workspace: Option<Workspace>,
    /// 上一步请求实际使用的模型（modelSwitchNotice 判定：模型变化时注入提示）。
    /// `dispatch_request(&self)` 内赋值，用内部可变（Mutex 保持 Kernel: Sync）。
    last_used_model: StdMutex<Option<String>>,
    /// 通用 spill 工件库（None = 未接线；未接线时工具结果原文直通）。
    spill: Option<Arc<SpillStore>>,
}

impl Kernel {
    /// 构造签名一次绑定全部协作者（config + 能力端口 +
    /// 内核状态一次注入）；正式实现 P1 AgentDriver 将收敛参数面。
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: Config,
        prompts: Arc<SystemPrompt>,
        context: Context,
        session: Arc<SessionLog>,
        inbox: Arc<Inbox>,
        machine: Machine,
        llm: Arc<dyn LlmPort>,
        model_target: Arc<ModelTargetRef>,
        tools: ToolRegistry,
        cancel: CancellationToken,
        workspace: Option<Workspace>,
    ) -> Self {
        Self {
            config,
            prompts,
            context,
            session,
            inbox,
            machine,
            llm,
            model_target,
            tools,
            cancel,
            listeners: Vec::new(),
            turn_count: 0,
            pending_system: None,
            pending_tools: None,
            workspace,
            last_used_model: StdMutex::new(None),
            spill: None,
        }
    }

    /// 运行开始时冻结 listener 快照（运行期不再增删）。
    pub fn freeze_listeners(&mut self, listeners: Vec<Arc<dyn Listener>>) {
        self.listeners = listeners;
    }

    pub fn session(&self) -> Arc<SessionLog> {
        self.session.clone()
    }

    pub fn inbox(&self) -> Arc<Inbox> {
        self.inbox.clone()
    }

    pub fn machine(&self) -> &Machine {
        &self.machine
    }

    /// 运行期模型目标引用（供宿主 /model 读取 current / 持久化）。
    pub fn model_target(&self) -> Arc<ModelTargetRef> {
        self.model_target.clone()
    }

    /// 置换运行期模型目标引用（assemble 期注入共享实例；运行中不得调用）。
    pub fn set_model_target(&mut self, target: Arc<ModelTargetRef>) {
        self.model_target = target;
    }

    /// 装配期注入 spill 工件库（未接线时工具结果原文直通）。
    pub fn set_spill_store(&mut self, store: Arc<SpillStore>) {
        self.spill = Some(store);
    }

    /// 工具结果溢出保留：超阈值结果落 spill 工件并替换为头尾摘要（#14）。
    ///
    /// spill 策略在 post-execute 段生效；未接线或工件写入失败时原文
    /// 直通（策略永不丢结果），图片块原样保留在结果内容中。
    fn apply_spill_retention(&self, call: &ToolCall, result: ToolResult) -> ToolResult {
        match &self.spill {
            Some(store) => crate::spill::policy::retain_tool_result(
                store,
                self.session.session_id(),
                call,
                result,
            ),
            None => result,
        }
    }

    /// 结束会话（Runtime::end 经锁调用）。
    pub fn end_session(&mut self, reason: &str) -> Result<(), StateError> {
        self.machine.end()?;
        self.session.append(SessionEvent::SessionEnd { reason: reason.to_string() });
        Ok(())
    }

    /// 置换本轮取消令牌（`Runtime::run` 每轮开轮前调用，持 kernel 锁内执行）。
    ///
    /// 取消按活动而非按会话：上一轮被 cancel 的 token 不复用，
    /// 新一轮拿到全新 token，会话因此可继续使用。
    pub fn set_cancel(&mut self, cancel: CancellationToken) {
        self.cancel = cancel;
    }

    /// 驱动一轮 turn；无输入返回 Ok(None)（不开轮）。
    pub async fn run(&mut self, input: Vec<AgentMessage>) -> Result<Option<TurnReport>, StateError> {
        self.machine.start_run()?;
        self.session.ensure_started(&self.config.model);
        // ① Claiming：外部输入 + next-turn 全量（开轮批量领取，同批投递的
        // relay + notice 合并单 turn）；皆空且无唤醒 → 不开轮。
        // 空闲唤醒开轮（有排队消息即 kick）：无在飞轮次时
        // steer 的唤醒直接开轮，next-step 消息整批成为首轮批次。
        // 开轮尝试即取走唤醒标志（取走即清）：不留陈旧标志，inject 永不触发开轮。
        let mut claimed = input;
        claimed.extend(self.inbox.claim_open());
        let wakeup = self.inbox.take_wakeup();
        if claimed.is_empty() && wakeup {
            claimed.extend(self.inbox.drain_next_step());
        }
        if claimed.is_empty() {
            self.machine.no_input()?;
            self.machine.turn_closed()?;
            return Ok(None);
        }
        self.machine.claimed()?;
        let turn = self.next_turn();
        self.log(SessionEvent::TurnStart { turn });
        let mut step_no = 0u32;
        let mut batch = claimed;
        let mut turn_ends: Option<TurnEndReason> = None;
        let mut turn_usage = crate::types::Usage::default();
        // ⑥ 开轮装配提示词（开轮一次，非每步重装）：
        // 渲染 system 文本、运行时快照并入首步批次、工具显式序覆盖请求草稿。
        // 严格插值契约：组装 / 渲染失败 → 空轮拒绝，不向模型发送坏提示词。
        let (system_text, snapshot, ordered_tools) = match assemble_prompt(&self.prompts) {
            Ok(parts) => parts,
            Err(error) => {
                self.machine.reject_step()?;
                let reason =
                    TurnEndReason::Rejected(format!("system prompt 组装失败: {error}"));
                self.log(SessionEvent::TurnEnd { turn, reason: reason.label() });
                self.machine.turn_closed()?;
                return Ok(Some(TurnReport {
                    turn,
                    steps: 0,
                    reason,
                    usage: crate::types::Usage::default(),
                }));
            }
        };
        self.pending_system = Some(system_text);
        self.pending_tools = Some(ordered_tools);
        if !snapshot.is_empty() {
            // 运行时快照带 source 标记：宿主可据此区分
            // 系统注入与真实用户输入（如 log-backed title 提取）。
            batch.insert(
                0,
                AgentMessage {
                    role: crate::types::Role::User,
                    blocks: vec![crate::types::ContentBlock::Text { text: snapshot }],
                    source: Some(MessageSource::RuntimeContext),
                },
            );
        }
        'turn: loop {
            if self.cancel.is_cancelled() {
                self.machine.abort_step()?;
                turn_ends = Some(TurnEndReason::Cancelled);
                break;
            }
            // ⑤ Admitting 前：next-step 整批领取（与工具欠请求/续跑共用边界）
            batch.extend(self.inbox.drain_next_step());
            let starts_series = step_no == 0;
            let claimed_input = ClaimedInput {
                turn,
                step: step_no + 1,
                source: if step_no == 0 { ClaimSource::TurnOpen } else { ClaimSource::NextStep },
                messages: std::mem::take(&mut batch),
                starts_request_series: starts_series,
            };
            let decision = self.dispatch_pre_step(claimed_input).await;
            let messages = match decision {
                PreStepDecision::Reject { reason } => {
                    self.machine.reject_step()?;
                    turn_ends = Some(TurnEndReason::Rejected(reason));
                    break 'turn;
                }
                PreStepDecision::Enter { messages, .. } => messages,
            };
            if messages.is_empty() && step_no == 0 {
                // 空轮：仅首轮以 completed 收轮；续跑 step 的空批次由工具上下文驱动。
                self.machine.reject_step()?;
                turn_ends.get_or_insert(TurnEndReason::Completed);
                break 'turn;
            }
            step_no += 1;
            self.machine.enter_step()?;
            self.log(SessionEvent::StepStart { turn, step: step_no });
            // ⑦ Preparing → Requesting：request 瀑布装配（listener 可改写草稿）
            let route = self.dispatch_request(&messages, starts_series).await;
            self.machine.enter_requesting()?;
            // 取消安全点：取消则 system/user 均不提交（提交点在本次检查之后）
            if self.cancel.is_cancelled() {
                self.machine.abort_step()?;
                turn_ends = Some(TurnEndReason::Cancelled);
                break 'turn;
            }
            self.commit_inputs(turn, step_no, &messages);
            // ⑨⑩⑪ Streaming（含同 step 重试）
            let outcome = self.request_with_retry(turn, step_no, route.draft).await?;
            if let Some(error) = &outcome.error {
                self.machine.abort_step()?;
                turn_ends = Some(if outcome.cancelled {
                    TurnEndReason::Cancelled
                } else {
                    TurnEndReason::Rejected(format!("请求失败: {error}"))
                });
                break 'turn;
            }
            if outcome.stop_reason == StopReason::MaxTokens {
                // max-tokens 粘性：作为轮结束理由保留
                turn_ends.get_or_insert(TurnEndReason::MaxTokens);
            }
            self.log(SessionEvent::AssistantMessage {
                turn,
                step: step_no,
                message: outcome.message.clone(),
                usage: outcome.usage,
            });
            turn_usage.input_tokens += outcome.usage.input_tokens;
            turn_usage.output_tokens += outcome.usage.output_tokens;
            turn_usage.cache_read_tokens += outcome.usage.cache_read_tokens;
            turn_usage.cache_write_tokens += outcome.usage.cache_write_tokens;
            let calls: Vec<ToolCall> = outcome.message.tool_calls();
            if !calls.is_empty() {
                self.machine.enter_executing()?;
                // F2：stop_reason == MaxTokens 时整批判失败（fail_truncated_calls）——
                // 输出被 token 上限截断，工具参数可能残缺，绝不执行，逐个落 is_error。
                if outcome.stop_reason == StopReason::MaxTokens {
                    for call in &calls {
                        self.log(SessionEvent::ToolCall { turn, step: step_no, call: call.clone() });
                        let result = ToolResult::error(
                            call,
                            format!(
                                "Tool call \"{}\" was not executed: the response hit the output \
                                 token limit, so its arguments may be truncated. Re-issue the \
                                 tool call with complete arguments.",
                                call.name
                            ),
                        );
                        self.dispatch_tool_post(call, &result).await;
                        self.log(SessionEvent::ToolResult { turn, step: step_no, result });
                    }
                } else {
                    self.execute_tools(turn, step_no, &calls).await;
                }
            }
            self.machine.enter_settling()?;
            self.log(SessionEvent::StepEnd { turn, step: step_no });
            // 续跑判定：工具欠请求 ∨ next-step 有输入
            let next_batch = self.inbox.drain_next_step();
            if !calls.is_empty() || !next_batch.is_empty() {
                batch = next_batch;
                self.machine.step_continues()?;
                continue 'turn;
            }
            // ⑮ Stopping：串行收尾瀑布（仅当 next-step 空），窗口内可 steer
            self.machine.enter_stopping()?;
            if !self.inbox.has_next_step() {
                self.dispatch_turn_stopping().await;
            }
            let injected = self.inbox.drain_next_step();
            if injected.is_empty() {
                self.machine.stop_end()?;
                break 'turn;
            }
            batch = injected;
            self.machine.stop_continue()?;
        }
        // ⑯ 闭轮：TurnEnd + 交还 Session 层
        let reason = turn_ends.unwrap_or(TurnEndReason::Completed);
        self.log(SessionEvent::TurnEnd { turn, reason: reason.label() });
        self.machine.turn_closed()?;
        Ok(Some(TurnReport { turn, steps: step_no, reason, usage: turn_usage }))
    }

    fn next_turn(&mut self) -> u32 {
        self.turn_count += 1;
        self.turn_count
    }

    /// durable 落日志 + 上下文投影同步更新。
    fn log(&mut self, event: SessionEvent) {
        self.context.apply(&event);
        self.session.append(event);
    }

    /// ⑧ 提交输入：首步发 SystemMessage（如有，渲染后非空）+ workspace 基线
    /// reminder（如有启用且已发现），每条批次消息发 UserMessage。
    fn commit_inputs(&mut self, turn: u32, step: u32, messages: &[AgentMessage]) {
        if step == 1 && self.context.system.is_none() {
            let system = self.pending_system.as_deref().filter(|text| !text.is_empty());
            if let Some(system) = system {
                self.log(SessionEvent::SystemMessage { turn, step, message: system.to_string() });
            }
        }
        if step == 1 {
            self.commit_workspace_baseline(turn, step);
        }
        for message in messages {
            self.log(SessionEvent::UserMessage { turn, step, message: message.clone() });
        }
    }

    /// workspace 基线注入（agent-instructions baseline）：step 1 渲染预算内
    /// 文件为 reminder 帧注入 user 消息；durable 侧落审计事件。SystemMessage 与
    /// workspace 基线是两个独立通道（system prompt vs user reminder），不互斥。
    fn commit_workspace_baseline(&mut self, turn: u32, step: u32) {
        let Some(workspace) = self.workspace.as_mut() else {
            return;
        };
        let baseline = workspace.render_baseline();
        if baseline.is_empty() {
            return;
        }
        // 先取齐 workspace 数据（避免与 self.log 的可变借用冲突）。
        let files = workspace.file_paths();
        let bytes = workspace.baseline_bytes();
        let version = workspace.baseline_version();
        let message = AgentMessage {
            role: crate::types::Role::User,
            blocks: vec![crate::types::ContentBlock::Text { text: baseline }],
            source: Some(MessageSource::WorkspaceBaseline { files: files.clone() }),
        };
        self.log(SessionEvent::UserMessage { turn, step, message });
        self.log(SessionEvent::WorkspaceInstructions { files, bytes, version });
    }

    /// 同 step 重试循环：错误落定 AssistantAttempt → request-error 瀑布 →
    /// Retry 且未取消且未超限 → 重装配（输入已提交，从 context 派生）重试。
    async fn request_with_retry(
        &mut self,
        turn: u32,
        step: u32,
        mut draft: RequestDraft,
    ) -> Result<AssistantTurn, StateError> {
        let mut retries = 0u32;
        loop {
            if self.cancel.is_cancelled() {
                return Ok(AssistantTurn::error(draft.model.clone(), "cancelled before request", true));
            }
            self.machine.enter_streaming()?;
            let outcome = self.stream_llm(turn, step, &draft).await;
            if let Some(message) = &outcome.error {
                self.log(SessionEvent::AssistantAttempt {
                    turn,
                    step,
                    error: message.clone(),
                    cancelled: outcome.cancelled,
                });
                let decision = self
                    .dispatch_request_error(
                        message,
                        outcome.cancelled,
                        outcome.failure.clone(),
                        retries + 1,
                    )
                    .await;
                if let RetryDecision::Retry { delay_ms } = decision {
                    if !outcome.cancelled && retries < self.config.max_retries {
                        retries += 1;
                        // 退避等待（对齐内核：先通知后等待，可被取消打断）。
                        if delay_ms > 0 && !self.cancel.is_cancelled() {
                            tokio::select! {
                                () = tokio::time::sleep(std::time::Duration::from_millis(delay_ms)) => {}
                                () = self.cancel.cancelled() => {}
                            }
                        }
                        self.machine.retry_request()?;
                        draft = self.dispatch_request(&[], false).await.draft;
                        continue;
                    }
                }
            }
            return Ok(outcome);
        }
    }

    /// LLM 流式：增量发 live AssistantStream；落定/错误均编码为 Done 进流（错误进流契约）。
    async fn stream_llm(&mut self, turn: u32, step: u32, draft: &RequestDraft) -> AssistantTurn {
        self.session.emit_live(LiveEvent::AssistantStream(StreamChunk::Start { turn, step }));
        let mut stream = self.llm.stream(draft, self.cancel.clone()).await;
        let mut settled: Option<AssistantTurn> = None;
        while let Some(item) = stream.next().await {
            match item {
                LlmStreamItem::Delta(delta) => {
                    let chunk = match delta {
                        StreamDelta::Text { text } => StreamChunk::Text(text),
                        StreamDelta::Thinking { thinking } => StreamChunk::Thinking(thinking),
                        StreamDelta::ToolCallStart { index, id, name } => {
                            StreamChunk::ToolCallStart { index, id, name }
                        }
                        StreamDelta::ToolCallInput { index, partial_json } => {
                            StreamChunk::ToolCallInput { index, partial_json }
                        }
                    };
                    self.session.emit_live(LiveEvent::AssistantStream(chunk));
                }
                LlmStreamItem::Done(outcome) => settled = Some(outcome),
            }
        }
        self.session.emit_live(LiveEvent::AssistantStream(StreamChunk::End));
        settled.unwrap_or_else(|| AssistantTurn::error(draft.model.clone(), "流中断：未收到落定消息", false))
    }

    /// ⑫⑬ 工具执行：ToolCall 先落 durable，再经准入瀑布执行；Deny/取消合成错误结果。
    /// 执行段按滚动池并发（上限 `max_parallel_tools`）：执行重叠、结果按模型顺序 commit；post 瀑布与 workspace 对账
    /// 在模型顺序 commit 点串行触发（listener 顺序确定性）。
    async fn execute_tools(&mut self, turn: u32, step: u32, calls: &[ToolCall]) {
        // 阶段一：ToolCall 事件按模型顺序先落 durable（启动前 append）。
        for call in calls {
            self.log(SessionEvent::ToolCall { turn, step, call: call.clone() });
        }

        // 阶段二：并发执行（admission 瀑布 + 工具执行），输出按 index 缓冲。
        let max_parallel = self.config.max_parallel_tools.max(1);
        let tools = self.tools.clone();
        let cancel = self.cancel.clone();
        let listeners = self.listeners.clone();
        let tool_timeout = self.config.tool_timeout;
        let mut results: Vec<Option<ToolResult>> = vec![None; calls.len()];
        let mut next = 0usize;
        let mut in_flight = futures_util::stream::FuturesUnordered::new();

        while next < calls.len() || !in_flight.is_empty() {
            // 补位：池未满且未取消即启动下一个。
            while next < calls.len() && in_flight.len() < max_parallel && !cancel.is_cancelled() {
                let call = calls[next].clone();
                let index = next;
                let tools = tools.clone();
                let cancel = cancel.clone();
                let listeners = listeners.clone();
                let ctx = self.listener_ctx();
                in_flight.push(async move {
                    (index, run_single_tool(tools, cancel, listeners, ctx, call, tool_timeout).await)
                });
                next += 1;
            }
            // 取消后不再补位（abort 停止启动，在途排空）。
            if in_flight.is_empty() {
                break;
            }
            // 任一结算即回收一个槽位（滚动）。
            if let Some((index, result)) = in_flight.next().await {
                results[index] = Some(result);
            }
        }

        // 取消后未启动的调用合成跳过结果（skipped-call 语义）。
        for (index, call) in calls.iter().enumerate() {
            if results[index].is_none() {
                results[index] =
                    Some(ToolResult::error(call, "tool call aborted before dispatch"));
            }
        }

        // 阶段三：结果按模型顺序 commit（log + post 瀑布 + workspace 对账）。
        for (call, result) in calls.iter().zip(results) {
            let result = match result {
                Some(result) => result,
                None => continue,
            };
            // 溢出保留：超大结果落 spill 工件并替换为头尾摘要；post listener
            // 观测保留版（spill 策略在 post-execute 生效）。
            let result = self.apply_spill_retention(call, result);
            // ⑬' 工具执行后旁路（PostToolUse）：串行逐 listener，纯观测/注入。
            self.dispatch_tool_post(call, &result).await;
            // workspace 触点对账：read_file/write_file 成功后 reconcile，
            // 有变更即注入 delta reminder（agent-instructions 变更同步）。
            if matches!(call.name.as_str(), "read_file" | "write_file") && !result.is_error {
                self.reconcile_workspace(turn, step).await;
            }
            self.log(SessionEvent::ToolResult { turn, step, result });
        }
    }

    /// workspace 触点对账：reconcile 后非空变更即注入 delta reminder user 消息。
    async fn reconcile_workspace(&mut self, turn: u32, step: u32) {
        let Some(workspace) = self.workspace.as_mut() else {
            return;
        };
        let changes: WorkspaceChanges = match workspace.reconcile() {
            Ok(changes) => changes,
            Err(_) => return,
        };
        if changes.is_empty() {
            return;
        }
        let delta = workspace.render_delta(&changes);
        if delta.is_empty() {
            return;
        }
        let message = AgentMessage {
            role: crate::types::Role::User,
            blocks: vec![crate::types::ContentBlock::Text { text: delta }],
            source: Some(MessageSource::WorkspaceDelta {
                files: changes.files(),
            }),
        };
        self.log(SessionEvent::UserMessage { turn, step, message });
        self.log(SessionEvent::WorkspaceChanged {
            added: changes.added,
            removed: changes.removed,
            changed: changes.changed,
        });
    }

    fn listener_ctx(&self) -> ListenerCtx {
        ListenerCtx {
            inbox: self.inbox.clone(),
            session: self.session.clone(),
            state: self.machine.snapshot(),
        }
    }

    /// prune 限制预算（offload 触发阈值）：`CompactSpec.threshold_tokens`
    /// 派生；派生失败 → offload 禁用（u64::MAX 恒不触发）。
    fn limit_tokens(&self) -> u64 {
        crate::compaction::config::resolve_compact_spec_for(&self.config, &self.config.model)
            .map(|spec| spec.threshold_tokens)
            .unwrap_or(u64::MAX)
    }

    /// prune 请求级开销估算：system + 工具 schema。
    fn overhead_tokens(&self) -> u64 {
        estimate_overhead(self.context.system.as_deref(), &self.context.tools)
    }

    async fn dispatch_pre_step(&self, input: ClaimedInput) -> PreStepDecision {
        let ctx = self.listener_ctx();
        waterfall::fold(
            &self.listeners,
            ctx,
            input,
            |input| {
                Box::pin(async move {
                    PreStepDecision::Enter {
                        messages: input.messages,
                        starts_request_series: input.starts_request_series,
                    }
                })
            },
            |listener, ctx, input, next| listener.pre_step(ctx, input, next),
        )
        .await
    }

    async fn dispatch_request(&self, pending: &[AgentMessage], starts_series: bool) -> StepRoute {
        // prompt-assembly 边界快照：current → assembled（system-prompt 装配）。
        // 运行中 /model 切换置换 current，本步快照后即锁定目标，不中断运行中 step。
        self.model_target.snapshot();
        let assembled_model = self
            .model_target
            .assembled()
            .map(|t| t.model)
            .unwrap_or_else(|| self.config.model.clone());
        // modelSwitchNotice（模型路由变化时前置注入的 user 提示）：
        // 变化时，在装配消息前注入一条 user 提示，声明上方 assistant 轮次由旧模型
        // 生成、会话以新模型继续。effort-only 变化不注入。
        let mut notice_pending: Vec<AgentMessage> = Vec::new();
        {
            let previous = self.last_used_model.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(previous) = previous.as_ref() {
                if previous != &assembled_model {
                    let notice = format!(
                        "[model changed: assistant turns above this point were generated by {previous}; \
                         the session continues with {assembled_model}]"
                    );
                    notice_pending.push(AgentMessage {
                        role: crate::types::Role::User,
                        blocks: vec![crate::types::ContentBlock::Text { text: notice }],
                        source: Some(crate::types::MessageSource::ModelSelection),
                    });
                }
            }
        }
        *self.last_used_model.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(assembled_model.clone());
        // 投影重建（listener 压缩事件回流）：listener 可能在 pre_step /
        // request_error 直写压缩事务（durable 已落 session），内核内存投影不
        // 感知。本步装配基线从 durable 日志 fold 重建——「model-visible means
        // logged」的完整闭环：session 是唯一权威，投影永远可重建。与逐条
        // apply 等价；tools 传投影初始表，随后被 pending_tools 覆盖。
        let context = Context::project(&self.session.snapshot(), self.context.tools.clone());
        let mut pending_all: Vec<AgentMessage> = Vec::with_capacity(notice_pending.len() + pending.len());
        pending_all.extend(notice_pending);
        pending_all.extend_from_slice(pending);
        let draft = context.derive_request(
            assembled_model,
            self.config.max_tokens,
            self.config.temperature,
            pending_all,
            starts_series,
        );
        // 提示词装配的工具表（规范排序）优先于投影初始表（assembly.tools 权威）。
        let mut draft = draft;
        if let Some(tools) = &self.pending_tools {
            draft.tools = tools.clone();
        }
        // 层1 图片预算（请求视图）：历史图片数量封顶——仅保留最近
        // MAX_CONTEXT_IMAGES 张，更早的替换为占位文本（state 原件不动；
        // read_image 入口已做像素/字节预算，见 media 模块）。先于 prune_view
        // 应用，使溢出估算（offload）基于封顶后的视图。
        draft.messages = crate::media::evict_old_images(&draft.messages);
        // E1 确定性裁剪段（请求装配内裁剪）：derive_request 之后、listener
        // 瀑布之前应用 prune_view——超长工具结果 head/marker/tail 裁剪恒开；
        // 溢出路径追加图片卸载（limit = threshold_tokens；offload 估算失败
        // 或 overhead 缺失时按 limit 不可得 → 仅文本裁剪）。明细非空即落
        // log-only durable 事件（投影不消费，视图重放零成本）。
        let (view, entries) = prune_view(
            &draft.messages,
            self.limit_tokens(),
            self.overhead_tokens(),
        );
        if !entries.is_empty() {
            let pre = draft
                .messages
                .iter()
                .map(estimate_message)
                .sum::<u64>();
            let post = view.iter().map(estimate_message).sum::<u64>();
            let saved_tokens = pre.saturating_sub(post);
            self.session.append(SessionEvent::CompactionPruned {
                pruned: entries.len() as u32,
                saved_tokens,
            });
            draft.messages = view;
        }
        let ctx = self.listener_ctx();
        waterfall::fold(
            &self.listeners,
            ctx,
            draft,
            |draft| Box::pin(async move { StepRoute { draft } }),
            |listener, ctx, draft, next| listener.request(ctx, draft, next),
        )
        .await
    }

    async fn dispatch_request_error(
        &self,
        message: &str,
        cancelled: bool,
        failure: Option<crate::llm::LlmFailure>,
        attempt: u32,
    ) -> RetryDecision {
        let error = RequestError { message: message.to_string(), cancelled, failure };
        let ctx = self.listener_ctx();
        waterfall::fold(
            &self.listeners,
            ctx,
            error,
            // 链尾默认决策（llm-retry normal 模式）：未分类裸错误轮
            // （测试 Fake）立即重试；有分类时按 retry 策略判定——可重试分类走
            // 退避（Retry-After 优先，指数 500ms→10s），Retry-After 超上限或
            // 不可重试分类 Abort。溢出分类由 compaction listener 的显式恢复
            // 决策承接，不进退避表。
            move |error: RequestError| {
                Box::pin(async move {
                    match &error.failure {
                        None => RetryDecision::Retry { delay_ms: 0 },
                        Some(failure) => {
                            if !crate::retry::is_retryable(failure.code) {
                                RetryDecision::Abort
                            } else {
                                match crate::retry::resolve_retry_delay(
                                    Some(failure),
                                    attempt,
                                    crate::retry::sample_random(),
                                ) {
                                    Some(delay_ms) => RetryDecision::Retry { delay_ms },
                                    None => RetryDecision::Abort,
                                }
                            }
                        }
                    }
                })
            },
            |listener, ctx, error, next| listener.request_error(ctx, error, next),
        )
        .await
    }

    async fn dispatch_tool(&self, call: ToolCall) -> ToolAdmission {
        let ctx = self.listener_ctx();
        waterfall::fold(
            &self.listeners,
            ctx,
            call,
            |call| Box::pin(async move { ToolAdmission::Allow { call } }),
            |listener, ctx, call, next| listener.tool(ctx, call, next),
        )
        .await
    }

    /// ⑮ turn-stopping：串行逐 listener（无 next、无决策），窗口内可 steer 注入。
    async fn dispatch_turn_stopping(&self) {
        let ctx = self.listener_ctx();
        for listener in &self.listeners {
            listener.turn_stopping(ctx.clone()).await;
        }
    }

    /// ⑬' 工具执行后旁路（post-execute 边界）：串行逐 listener。
    async fn dispatch_tool_post(&self, call: &ToolCall, result: &ToolResult) {
        let ctx = self.listener_ctx();
        for listener in &self.listeners {
            listener.tool_post(ctx.clone(), call, result).await;
        }
    }
}

/// 组装 + 渲染两阶段：system 文本、运行时快照、规范排序的工具表。
/// 任何失败即拒绝开轮——不向模型发送格式错误的提示词。
fn assemble_prompt(
    prompts: &SystemPrompt,
) -> Result<(String, String, Vec<ToolDefinition>), String> {
    let assembly = prompts.assemble().map_err(|error| error.to_string())?;
    let system = system_prompt::render_prompt(&assembly).map_err(|error| error.to_string())?;
    let snapshot =
        system_prompt::render_context_snapshot(&assembly).map_err(|error| error.to_string())?;
    Ok((system, snapshot, assembly.tools))
}

/// 单次工具调用的并发安全执行单元（不借 `&mut self`）：admission 瀑布
/// （所有 listener 依次过闸）→ 取消复查 → 注册表执行。可在滚动池中 spawn。
async fn run_single_tool(
    tools: ToolRegistry,
    cancel: CancellationToken,
    listeners: Vec<Arc<dyn Listener>>,
    ctx: ListenerCtx,
    call: ToolCall,
    tool_timeout: Duration,
) -> ToolResult {
    let admission = waterfall::fold(
        &listeners,
        ctx,
        call,
        |call| Box::pin(async move { ToolAdmission::Allow { call } }),
        |listener, ctx, call, next| listener.tool(ctx, call, next),
    )
    .await;
    match admission {
        ToolAdmission::Allow { call } => {
            if cancel.is_cancelled() {
                ToolResult::error(&call, "tool call aborted before dispatch")
            } else {
                tools.execute(&call, &cancel, tool_timeout).await
            }
        }
        ToolAdmission::Deny { call, reason } => ToolResult::error(&call, format!("denied: {reason}")),
    }
}

#[cfg(test)]
#[path = "kernel_tests.rs"]
mod tests;
