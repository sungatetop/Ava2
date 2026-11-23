//! 5 个演示 listener：纯观测 / inject 不唤醒 / 观测+steer / 瀑布拦截+洋葱包裹可视化 / 工具审批。

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, RwLock};

use serde_json::Value;
use tokio::sync::{mpsc, oneshot};

use crate::approval::{
    decide_full, session_grant, ApprovalConfig, ApprovalDecision, ApprovalOutcome, Decision,
    ToolRisk,
};
use crate::listener::{
    ClaimedInput, Listener, ListenerCtx, PreStepDecision, ToolAdmission,
};
use crate::session::{LiveEvent, SessionEvent};
use crate::types::ToolCall;
use crate::waterfall::{BoxFuture, Next};

/// 纯观测：彩色紧凑打印全部 live 事件（终端直观看到洋葱穿透次序）。
///
/// 输出一律走 **stderr**：诊断回显不得污染 stdout——stdio 协议前端
/// （`onion-server --stdio acp|sdk`）与嵌入宿主都把 stdout 当作协议流。
pub struct EchoObserver;

impl Listener for EchoObserver {
    fn on_event(&self, _ctx: &ListenerCtx, event: &LiveEvent) {
        match event {
            LiveEvent::StateChanged { from, to } => {
                eprintln!("    \x1b[90m[state]\x1b[0m {from} -> {to}");
            }
            LiveEvent::Durable(ev) => match ev {
                SessionEvent::SessionStarted { session_id, model } => {
                    eprintln!("    \x1b[36m[durable]\x1b[0m session_started {session_id} model={model}");
                }
                SessionEvent::TurnStart { turn } => eprintln!("    \x1b[36m[durable]\x1b[0m turn_start t{turn}"),
                SessionEvent::SystemMessage { turn, step, .. } => {
                    eprintln!("    \x1b[36m[durable]\x1b[0m system_message t{turn}.s{step}");
                }
                SessionEvent::UserMessage { turn, step, message } => {
                    let text = message.text_content();
                    let text = if text.chars().count() > 60 { format!("{}…", text.chars().take(60).collect::<String>()) } else { text };
                    eprintln!("    \x1b[36m[durable]\x1b[0m user_message t{turn}.s{step} \"{text}\"");
                }
                SessionEvent::AssistantMessage { turn, step, usage, .. } => {
                    eprintln!(
                        "    \x1b[36m[durable]\x1b[0m assistant_message t{turn}.s{step} in={} out={}",
                        usage.input_tokens, usage.output_tokens
                    );
                }
                SessionEvent::AssistantAttempt { turn, step, error, cancelled } => {
                    eprintln!("    \x1b[31m[durable]\x1b[0m assistant_attempt t{turn}.s{step} cancelled={cancelled} {error}");
                }
                SessionEvent::ToolCall { turn, step, call } => {
                    eprintln!("    \x1b[36m[durable]\x1b[0m tool_call t{turn}.s{step} {}({})", call.name, call.input);
                }
                SessionEvent::ToolResult { turn, step, result } => {
                    let mark = if result.is_error { "\x1b[31mERR\x1b[0m" } else { "ok" };
                    eprintln!("    \x1b[36m[durable]\x1b[0m tool_result t{turn}.s{step} {} [{mark}]", result.name);
                }
                SessionEvent::StepStart { turn, step } => eprintln!("    \x1b[36m[durable]\x1b[0m step_start t{turn}.s{step}"),
                SessionEvent::StepEnd { turn, step } => eprintln!("    \x1b[36m[durable]\x1b[0m step_end t{turn}.s{step}"),
                SessionEvent::TurnEnd { turn, reason } => {
                    eprintln!("    \x1b[36m[durable]\x1b[0m turn_end t{turn} reason={reason}");
                }
                SessionEvent::SessionEnd { reason } => eprintln!("    \x1b[36m[durable]\x1b[0m session_end {reason}"),
                SessionEvent::CompactionStart { compaction_id, trigger, .. } => {
                    eprintln!("    \x1b[36m[durable]\x1b[0m compaction_start {compaction_id} trigger={trigger}");
                }
                SessionEvent::CompactionSummary { compaction_id, shadowed_count, shadowed_tokens, .. } => {
                    eprintln!(
                        "    \x1b[36m[durable]\x1b[0m compaction_summary {compaction_id} shadowed={shadowed_count} tokens={shadowed_tokens}"
                    );
                }
                SessionEvent::CompactionEnd { compaction_id, error, .. } => {
                    eprintln!("    \x1b[36m[durable]\x1b[0m compaction_end {compaction_id} error={error:?}");
                }
                SessionEvent::CompactionPruned { pruned, saved_tokens } => {
                    eprintln!("    \x1b[36m[durable]\x1b[0m compaction_pruned pruned={pruned} saved={saved_tokens}");
                }
                SessionEvent::HookInvoked { point, handler_id, matcher } => {
                    eprintln!(
                        "    \x1b[35m[durable]\x1b[0m hook_invoked {point} #{handler_id} matcher={matcher:?}"
                    );
                }
                SessionEvent::HookResult { point, handler_id, decision, exit_code, stderr_summary, duration_ms } => {
                    eprintln!(
                        "    \x1b[35m[durable]\x1b[0m hook_result {point} #{handler_id} decision={decision} exit={exit_code:?} {duration_ms}ms stderr={stderr_summary:?}"
                    );
                }
                SessionEvent::WorkspaceInstructions { files, bytes, .. } => {
                    eprintln!(
                        "    \x1b[36m[durable]\x1b[0m workspace_instructions {} files {} bytes",
                        files.len(),
                        bytes
                    );
                }
                SessionEvent::WorkspaceChanged { added, removed, changed } => {
                    eprintln!(
                        "    \x1b[36m[durable]\x1b[0m workspace_changed +{} -{} ~{}",
                        added.len(),
                        removed.len(),
                        changed.len()
                    );
                }
                SessionEvent::GoalChange { operation, rounds_started, .. } => {
                    eprintln!(
                        "    \x1b[36m[durable]\x1b[0m goal_change {operation} rounds_started={rounds_started}"
                    );
                }
                SessionEvent::PlanMode { active } => {
                    eprintln!("    \x1b[36m[durable]\x1b[0m plan_mode active={active}");
                }
                SessionEvent::PlanWanted { active } => {
                    eprintln!("    \x1b[36m[durable]\x1b[0m plan_wanted wanted={active}");
                }
                SessionEvent::PlanRunDone { .. } => {
                    eprintln!("    \x1b[36m[durable]\x1b[0m plan_run_done");
                }
                SessionEvent::ApprovalAudited { tool, decision, via } => {
                    eprintln!("    \x1b[35m[durable]\x1b[0m approval_audited {tool} {decision} via={via}");
                }
                SessionEvent::TodoWrite { todos } => {
                    eprintln!("    \x1b[36m[durable]\x1b[0m todo_write x{}", todos.len());
                }
                SessionEvent::Presented { turn, files, .. } => {
                    eprintln!("    \x1b[36m[durable]\x1b[0m presented t{turn} x{}", files.len());
                }
                SessionEvent::WorkingDir { cwd } => {
                    eprintln!("    \x1b[36m[durable]\x1b[0m working_dir {cwd}");
                }
                SessionEvent::WorktreeChange { path } => {
                    eprintln!("    \x1b[36m[durable]\x1b[0m worktree_change {path}");
                }
                SessionEvent::TeamMemberChange { team_id, member } => {
                    eprintln!("    \x1b[36m[durable]\x1b[0m team_member_change team={team_id} name={}", member.name);
                }
                SessionEvent::TeamTaskChange { team_id, task } => {
                    eprintln!("    \x1b[36m[durable]\x1b[0m team_task_change team={team_id} id={} rev={}", task.id, task.revision);
                }
                SessionEvent::ModelSelection { model, base_url, effort } => {
                    eprintln!(
                        "    \x1b[36m[durable]\x1b[0m model_selection model={model} base_url={base_url} effort={effort:?}"
                    );
                }
                SessionEvent::SessionTitle { title } => {
                    eprintln!("    \x1b[36m[durable]\x1b[0m session_title \"{title}\"");
                }
                SessionEvent::CommandRun { command_id, name, args } => {
                    eprintln!(
                        "    \x1b[36m[durable]\x1b[0m command_run {command_id} /{name} {}",
                        args.as_deref().unwrap_or("")
                    );
                }
                SessionEvent::CommandDone { command_id, kind, text, .. } => {
                    eprintln!(
                        "    \x1b[36m[durable]\x1b[0m command_done {command_id} {kind} {}",
                        text.as_deref().unwrap_or("")
                    );
                }
                SessionEvent::ApprovalPreset { preset, source } => {
                    eprintln!(
                        "    \x1b[36m[durable]\x1b[0m approval_preset {preset} source={}",
                        source.as_deref().unwrap_or("-")
                    );
                }
                SessionEvent::FeedbackRecord { text, category } => {
                    eprintln!(
                        "    \x1b[36m[durable]\x1b[0m feedback_record category={} \"{}\"",
                        category.as_deref().unwrap_or("-"),
                        text.as_deref().unwrap_or("")
                    );
                }
                SessionEvent::SubagentModelSelectionPolicy { allowed_models } => {
                    let routes = allowed_models
                        .iter()
                        .map(|r| format!("{}/{}", r.provider, r.model))
                        .collect::<Vec<_>>()
                        .join(", ");
                    eprintln!("    \x1b[36m[durable]\x1b[0m subagent_model_selection_policy [{routes}]");
                }
                SessionEvent::SubagentDescriptor { mode, provider, label, agent_model, depth, .. } => {
                    eprintln!(
                        "    \x1b[36m[durable]\x1b[0m subagent_descriptor {mode}/{provider} depth={depth} model={} label={}",
                        agent_model.as_deref().unwrap_or("-"),
                        label.as_deref().unwrap_or("-")
                    );
                }
                SessionEvent::ForkSeed { parent_session_id, parent_boundary_seq, inherited_events } => {
                    eprintln!(
                        "    \x1b[36m[durable]\x1b[0m fork_seed parent={parent_session_id} boundary={parent_boundary_seq} inherited={inherited_events}"
                    );
                }
            },
            LiveEvent::InboxInserted { partition, message } => {
                let text = message.text_content();
                let text = if text.chars().count() > 40 { format!("{}…", text.chars().take(40).collect::<String>()) } else { text };
                eprintln!("    \x1b[33m[inbox]\x1b[0m inserted {partition} \"{text}\"");
            }
            LiveEvent::InboxClaimed { partition, messages } => {
                eprintln!("    \x1b[33m[inbox]\x1b[0m claimed {partition} x{}", messages.len());
            }
            LiveEvent::AssistantStream(chunk) => match chunk {
                crate::session::StreamChunk::Start { turn, step } => {
                    eprintln!("    \x1b[35m[stream]\x1b[0m start t{turn}.s{step}");
                }
                crate::session::StreamChunk::Text(text) => eprint!("\x1b[32m{text}\x1b[0m"),
                crate::session::StreamChunk::Thinking(text) => eprint!("\x1b[90m{text}\x1b[0m"),
                crate::session::StreamChunk::ToolCallStart { index, id, name } => {
                    eprintln!("\n    \x1b[35m[stream]\x1b[0m tool_call_start #{index} {name} ({id})");
                }
                crate::session::StreamChunk::ToolCallInput { index, partial_json } => {
                    eprintln!("    \x1b[35m[stream]\x1b[0m tool_call_input #{index} {partial_json}");
                }
                crate::session::StreamChunk::End => eprintln!("\n    \x1b[35m[stream]\x1b[0m end"),
            },
            LiveEvent::SubagentStart { run_id, name, depth, .. } => {
                eprintln!("    \x1b[94m[subagent]\x1b[0m start {name} depth={depth} run={run_id}");
            }
            LiveEvent::SubagentEnd { run_id, name, stop_reason, .. } => {
                eprintln!("    \x1b[94m[subagent]\x1b[0m end {name} stop={stop_reason} run={run_id}");
            },
        }
    }
}

/// SessionStarted 时 inject 一条 `<environment>` meta 消息（不唤醒）：
/// 落在首个被接纳的 step 的 Admitting 批次。
///
/// cwd 取会话级 [`SessionCwd`]：多会话部署（onion-server）下宿主进程 cwd 是
/// 装配目录而非任一会话的工作目录，注入进程 cwd 会让模型自报失真的工作目录
/// （「工作区切换未生效」的表象即源于此）；单会话 front door 的 session_cwd
/// 初始即进程 cwd / 装配根，语义不变。
pub struct EnvInjector {
    session_cwd: Arc<crate::session_cwd::SessionCwd>,
}

impl EnvInjector {
    /// 以会话级工作目录为环境基线（全部装配点统一入口）。
    pub fn with_session_cwd(session_cwd: Arc<crate::session_cwd::SessionCwd>) -> Self {
        Self { session_cwd }
    }
}

impl Listener for EnvInjector {
    fn on_event(&self, ctx: &ListenerCtx, event: &LiveEvent) {
        if let LiveEvent::Durable(SessionEvent::SessionStarted { .. }) = event {
            let os = std::env::consts::OS;
            let cwd = self.session_cwd.current().display().to_string();
            let when = crate::session::epoch_millis();
            let text = format!("<environment os=\"{os}\" cwd=\"{cwd}\" epoch_ms=\"{when}\"/>");
            // 带 runtime-context source 标记：系统注入不冒充真实用户输入。
            let message = crate::types::AgentMessage {
                role: crate::types::Role::User,
                blocks: vec![crate::types::ContentBlock::Text { text }],
                source: Some(crate::types::MessageSource::RuntimeContext),
            };
            match ctx.inbox.inject(message) {
                Ok(()) => eprintln!("    \x1b[34m[env]\x1b[0m injected environment baseline (next-step, no wakeup)"),
                Err(error) => eprintln!("    \x1b[34m[env]\x1b[0m inject failed: {error}"),
            }
        }
    }
}

/// ToolResult 为 error 时 steer 一条纠正消息（唤醒）：演示事件监听驱动的边界注入。
pub struct RetryNudger;

impl Listener for RetryNudger {
    fn on_event(&self, ctx: &ListenerCtx, event: &LiveEvent) {
        if let LiveEvent::Durable(SessionEvent::ToolResult { result, .. }) = event {
            if result.is_error {
                let text = format!("上次工具调用 {} 失败，请修正参数后重试", result.name);
                // 带 runtime-context source 标记：纠正提示是引擎注入，不冒充真实用户输入
                // （UI 渲染据此过滤，模型侧仍作为 user_message 进入上下文）。
                let message = crate::types::AgentMessage {
                    role: crate::types::Role::User,
                    blocks: vec![crate::types::ContentBlock::Text { text }],
                    source: Some(crate::types::MessageSource::RuntimeContext),
                };
                match ctx.inbox.steer(message) {
                    Ok(()) => eprintln!("    \x1b[34m[nudge]\x1b[0m steered retry hint (next-step, wakeup)"),
                    Err(error) => eprintln!("    \x1b[34m[nudge]\x1b[0m steer failed: {error}"),
                }
            }
        }
    }
}

/// 瀑布拦截 + 洋葱包裹可视化：消息含 "BLOCK" 时不调 next 直接 reject（短路）；
/// 否则打印「→ 外层进入」/「← 外层退出」包裹 next()。
pub struct Gatekeeper {
    pub entered: Arc<Mutex<u32>>,
    pub exited: Arc<Mutex<u32>>,
}

impl Gatekeeper {
    pub fn new() -> Self {
        Self { entered: Arc::new(Mutex::new(0)), exited: Arc::new(Mutex::new(0)) }
    }
}

impl Default for Gatekeeper {
    fn default() -> Self {
        Self::new()
    }
}

impl Listener for Gatekeeper {
    fn pre_step(&self, ctx: ListenerCtx, input: ClaimedInput, next: Next<ClaimedInput, PreStepDecision>) -> BoxFuture<PreStepDecision> {
        let blocked = input.messages.iter().any(|m| m.text_content().contains("BLOCK"));
        if blocked {
            eprintln!("    \x1b[31m[gate]\x1b[0m BLOCK detected at t{}.s{} → reject（短路，不调 next）", input.turn, input.step);
            let _ = ctx;
            return Box::pin(async { PreStepDecision::Reject { reason: "message blocked by gatekeeper".to_string() } });
        }
        *self.entered.lock().unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
        let exited = self.exited.clone();
        eprintln!("    \x1b[34m[gate]\x1b[0m → 外层进入 pre_step t{}.s{}（{} 条消息）", input.turn, input.step, input.messages.len());
        Box::pin(async move {
            let decision = next(input).await;
            *exited.lock().unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
            let summary = match &decision {
                PreStepDecision::Enter { messages, .. } => format!("enter({} msgs)", messages.len()),
                PreStepDecision::Reject { reason } => format!("reject({reason})"),
            };
            eprintln!("    \x1b[34m[gate]\x1b[0m ← 外层退出 pre_step → {summary}");
            decision
        })
    }
}

/// 工具执行审批请求（listener → main）：调用摘要 + 一次性裁决通道。
/// 答复 `AllowedOnce` / `AllowedForSession` = 放行执行；`Rejected` / 通道关闭（reply
/// 被 drop，如会话退出前）= 拒绝（fail-closed）。
pub struct ApprovalRequest {
    pub name: String,
    pub input: Value,
    /// 来源标记：`None` = 准入 gate 首次审批；`Some("sandbox-retry")` = OS 沙箱
    /// 拒绝后的越界执行申请（前端据此区分文案）。
    pub reason: Option<String>,
    pub reply: oneshot::Sender<ApprovalDecision>,
}

/// 在飞审批的共享裁决（同一授权键的并发调用复用一次人工裁决，避免弹卡风暴）。
#[derive(Default)]
struct Inflight {
    decision: Mutex<Option<OperatorReply>>,
    notify: tokio::sync::Notify,
}

/// 人工裁决结果（含通道异常的 fail-closed 分支）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperatorReply {
    /// 正常裁决。
    Decided(ApprovalDecision),
    /// 审批通道关闭（main 侧接收端已 drop）。
    Closed,
    /// 答复丢失（reply 发送端被 drop，如会话退出前清理）。
    Dropped,
}

/// 向人工发起一次审批；通道关闭 / 答复丢失均按 fail-closed 拒绝处理。
async fn ask_operator(
    requests: &mpsc::UnboundedSender<ApprovalRequest>,
    name: &str,
    input: &Value,
    reason: Option<String>,
) -> OperatorReply {
    let (reply, receiver) = oneshot::channel();
    let request = ApprovalRequest { name: name.to_string(), input: input.clone(), reason, reply };
    if requests.send(request).is_err() {
        return OperatorReply::Closed;
    }
    eprintln!("    \x1b[33m[approve]\x1b[0m 挂起等待人工复决 {name} ...");
    receiver.await.map_or(OperatorReply::Dropped, OperatorReply::Decided)
}

/// 落一条审批审计（`decision` 只能取 [`ApprovalOutcome`] 闭集字面值）。
fn audit(session: &Arc<crate::session::SessionLog>, tool: &str, outcome: ApprovalOutcome, via: &str) {
    session.append(SessionEvent::ApprovalAudited {
        tool: tool.to_string(),
        decision: outcome.as_str().to_string(),
        via: via.to_string(),
    });
}

/// 工具审批 gate（tool 准入瀑布；安全模型
/// 第二层，见 docs/design/onion-tools-design.md §6）。判定内核为「沙箱允许集 ×
/// 审批策略」两维模型 + 工具限制（[`crate::approval::decide_full`]）：
/// `deny` 规则强制拒绝、`allow` 规则强制放行；沙箱允许集内直通（`workspace-write`
/// 下工作区写入免审批）；允许集外的升级操作按 `Ask` 挂起等待 `/approve` 或
/// `/deny`（或交 `ApprovalReviewer` seam 自动裁决）、按 `Never` 直接拒绝
/// （fail-closed）。取消/退出路径无需感知 cancel——main 在会话结束前 drop 挂起
/// 答复，等待端 recv Err → fail-closed `Rejected`，配对不变式由既有 Rejected →
/// 合成错误 ToolResult 通道维持。
pub struct ApprovalGate {
    config: Arc<RwLock<ApprovalConfig>>,
    classify: Arc<dyn Fn(&ToolCall) -> ToolRisk + Send + Sync>,
    requests: mpsc::UnboundedSender<ApprovalRequest>,
    /// 会话级授权表（键见 [`crate::approval::session_grant`]；不落盘，随会话销毁）。
    grants: Arc<Mutex<HashSet<String>>>,
    /// 在飞审批去重表：同键共享一次人工裁决。
    inflight: Arc<Mutex<HashMap<String, Arc<Inflight>>>>,
}

impl ApprovalGate {
    /// 声明式装配（部署侧安全配置，不进工具定义）：共享审批配置（预设 + 裁决者，
    /// 可经 [`Self::config`] 句柄运行时切换）+ 风险分类器。分类器按调用内容判定
    /// 风险（`ToolRegistry::classify`，按操作边界判定——如 shell 读/写
    /// 命令区分）；未登记的工具按只读直通（registry 执行侧兜底 unknown tool 错误）。
    pub fn new(
        config: Arc<RwLock<ApprovalConfig>>,
        classify: Arc<dyn Fn(&ToolCall) -> ToolRisk + Send + Sync>,
        requests: mpsc::UnboundedSender<ApprovalRequest>,
    ) -> Self {
        Self {
            config,
            classify,
            requests,
            grants: Arc::new(Mutex::new(HashSet::new())),
            inflight: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 共享审批请求发送端（main 消费 `approval_rx`；Phase E-2 沙箱拒绝升级复用）。
    pub fn requests(&self) -> mpsc::UnboundedSender<ApprovalRequest> {
        self.requests.clone()
    }

    /// 共享审批配置句柄（main 的 `/permission` 切换入口）。
    pub fn config(&self) -> Arc<RwLock<ApprovalConfig>> {
        self.config.clone()
    }
}

impl Listener for ApprovalGate {
    fn tool(
        &self,
        ctx: ListenerCtx,
        call: ToolCall,
        next: Next<ToolCall, ToolAdmission>,
    ) -> BoxFuture<ToolAdmission> {
        let config = self.config.read().unwrap_or_else(std::sync::PoisonError::into_inner);
        let preset = config.preset.clone();
        let reviewer = config.reviewer.clone();
        drop(config);
        let risk = (self.classify)(&call);
        match decide_full(&call.name, risk, &preset) {
            Decision::Allow => {
                // 审计（D1）：允许集直通或 allow 规则放行。
                let via = if preset.restriction.is_allowed(&call.name) { "restriction" } else { "sandbox" };
                audit(&ctx.session, &call.name, ApprovalOutcome::AllowedOnce, via);
                return next(call);
            }
            Decision::Deny => {
                // 审计（D1）：deny 规则或 never 策略拒绝。
                let via = if preset.restriction.is_denied(&call.name) { "restriction" } else { "policy" };
                audit(&ctx.session, &call.name, ApprovalOutcome::Rejected, via);
                return Box::pin(async move {
                    ToolAdmission::Deny { call, reason: "denied by approval policy (never)".to_string() }
                });
            }
            Decision::Escalate => {}
        }
        // 会话级授权（本会话允许）：同键调用直通，审计标记 `operator-session`。
        let grant = session_grant(&call.name, &call.input);
        if let Some(grant) = &grant {
            let granted = self.grants.lock().unwrap_or_else(std::sync::PoisonError::into_inner).contains(grant);
            if granted {
                audit(&ctx.session, &call.name, ApprovalOutcome::AllowedOnce, "operator-session");
                return next(call);
            }
        }
        // 审批必须发生在打开的轮次内：审计对
        // 必须被 durable 日志的提交/回放边界包住，轮次之间的裸审批在重载时是
        // 崩溃尾巴垃圾。tool 瀑布只能返回准入结论，故 fail-closed
        // 拒绝且不落审计事件（保持「无事件」语义）。
        if !crate::approval::turn_open(&ctx.session.snapshot()) {
            eprintln!(
                "    \x1b[31m[approve]\x1b[0m {} → 轮次外审批请求，fail-closed 拒绝（不落审计）",
                call.name
            );
            return Box::pin(async move {
                ToolAdmission::Deny {
                    call,
                    reason: "approval requested outside an open turn: the approval audit pair must be turn-enclosed"
                        .to_string(),
                }
            });
        }
        // 升级操作：有 reviewer 则自动裁决（auto 预设），无则挂起人工复决。
        if let Some(reviewer) = reviewer {
            let session = ctx.session;
            return Box::pin(async move {
                let decision = reviewer.review(call.clone()).await;
                let allowed = decision.is_allowed();
                audit(&session, &call.name, decision.outcome(), "reviewer");
                if allowed {
                    ToolAdmission::Allow { call }
                } else {
                    ToolAdmission::Deny { call, reason: "rejected by reviewer".to_string() }
                }
            });
        }
        let requests = self.requests.clone();
        let session = ctx.session;
        let grants = self.grants.clone();
        let inflight = self.inflight.clone();
        // 去重键：可授权调用按授权键（覆盖同族重复），其余按完整输入（精确匹配）。
        let key = grant.clone().unwrap_or_else(|| format!("{}:{}", call.name, call.input));
        Box::pin(async move {
            // 在飞去重：同键已有在飞请求时复用其裁决（并发/循环下只弹一张卡）。
            let (entry, leader) = {
                let mut map = inflight.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                match map.get(&key).cloned() {
                    Some(entry) => (entry, false),
                    None => {
                        let entry = Arc::new(Inflight::default());
                        map.insert(key.clone(), entry.clone());
                        (entry, true)
                    }
                }
            };
            let decision = if leader {
                let decision = ask_operator(&requests, &call.name, &call.input, None).await;
                *entry.decision.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(decision);
                entry.notify.notify_waiters();
                inflight.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(&key);
                decision
            } else {
                loop {
                    let notified = entry.notify.notified();
                    if let Some(decision) = *entry.decision.lock().unwrap_or_else(std::sync::PoisonError::into_inner) {
                        break decision;
                    }
                    notified.await;
                }
            };
            match decision {
                OperatorReply::Closed => {
                    // 无审批通道可用（main 侧接收端已 drop）：fail-closed `unavailable`。
                    audit(&session, &call.name, ApprovalOutcome::Unavailable, "fail-closed");
                    ToolAdmission::Deny { call, reason: "approval channel closed".to_string() }
                }
                OperatorReply::Dropped => {
                    // 请求被撤回（答复端在会话中断/退出前被 drop）：`cancelled`。
                    eprintln!("    \x1b[31m[approve]\x1b[0m {} → 答复丢失，fail-closed 拒绝", call.name);
                    audit(&session, &call.name, ApprovalOutcome::Cancelled, "interrupted");
                    ToolAdmission::Deny { call, reason: "approval cancelled (fail-closed)".to_string() }
                }
                OperatorReply::Decided(ApprovalDecision::Rejected) => {
                    eprintln!("    \x1b[31m[approve]\x1b[0m {} → rejected by operator", call.name);
                    audit(&session, &call.name, ApprovalOutcome::Rejected, "operator");
                    ToolAdmission::Deny { call, reason: "rejected by operator".to_string() }
                }
                OperatorReply::Decided(ApprovalDecision::AllowedForSession) if grant.is_some() => {
                    let grant = grant.unwrap_or_default();
                    eprintln!("    \x1b[32m[approve]\x1b[0m {} → allowed-for-session（{grant}）", call.name);
                    grants.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(grant);
                    audit(&session, &call.name, ApprovalOutcome::AllowedOnce, "operator-session");
                    ToolAdmission::Allow { call }
                }
                OperatorReply::Decided(_) => {
                    // `allowed-once`，或破坏性命令的「本会话允许」（降级为一次性放行）。
                    eprintln!("    \x1b[32m[approve]\x1b[0m {} → allowed-once（放行执行）", call.name);
                    audit(&session, &call.name, ApprovalOutcome::AllowedOnce, "operator");
                    ToolAdmission::Allow { call }
                }
            }
        })
    }
}

#[cfg(test)]
#[path = "listeners_tests.rs"]
mod tests;
