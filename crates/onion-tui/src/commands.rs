//! commands.rs：斜杠命令分发（沿用 onion-cli 命令面，设计文档 §7.2）。
//!
//! TUI 侧分发要点：反馈不 `println!`（全屏画面被污染），统一收集到 `Vec<String>`
//! 由宿主渲染到反馈区；plan 切换不依赖 `onion::plan::commands::run_plan_command`
//! （内部 println! 直出终端），自实现无打印版本（同一投影语义：persist
//! wanted + busy 挂 pending / 空闲立即生效）。
//!
//! 命令面（对齐 onion-cli main.rs 头注）：
//! - 普通文本：闲时直达开轮，忙时 followup 排队（一条排队消息 = 一轮）
//! - `/steer <msg>` / `/inject <msg>` / `/followup <msg>` / `/compact`
//! - `/agent [id]` / `/permission [preset]` / `/approve` / `/deny`
//! - `/answer <id> <value>` / `/goal [objective|pause|resume|complete|clear]`
//! - `/plan [off|<msg>]` / `/new` / `/resume [id]` / `/exit`
//! - `/status`（Phase B 接入，见 design §10）

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::collections::VecDeque;

use onion::assemble::{build_approval_config, Assembled};
use onion::approval::{ApprovalConfig, ApprovalDecision};
use onion::ask_user::types::{AskUserAnswer, AskUserAnswerItem, AskUserQuestion};
use onion::goal::driver::GoalRoundDriver;
use onion::goal::service::GoalService;
use onion::plan::service::{PlanMode, PlanVerdict, SetResult};
use onion::plan::store::append_plan_wanted;
use onion::llm::OpenAiSseLlm;
use onion::runtime::Runtime;
use onion::types::AgentMessage;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use crate::renderer::{Line, Style};
use crossterm::style::Color;

/// 在飞 ask_user 问询状态（timed 超时后转为 `AnswerToPending` 注入 inbox）。
pub struct PendingAskUser {
    /// 一次性答复句柄（前台已超时则 send 失败）。
    pub reply: oneshot::Sender<AskUserAnswer>,
    /// 原始问题全集（迟到答复注入时随附）。
    pub asked: Vec<AskUserQuestion>,
    /// 尚未作答的问题。
    pub questions: Vec<AskUserQuestion>,
    /// 已作答项。
    pub answered: Vec<AskUserAnswerItem>,
    /// 发起该调用的 tool call id（迟到注入锚点）。
    pub call_id: String,
}

/// 命令分发后宿主需执行的动作（开轮 / 退出）。
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    /// 无驱动动作：宿主只重绘反馈区。
    Continue,
    /// 需要开轮（main 消费：busy=false 时 spawn_turn；busy=true 时排队，
    /// 由 turn 循环收尾后自动续轮）。
    Kick,
    /// 优雅退出（SessionEnd 语义，main 消费：restore + 结束）。
    Quit,
    /// 打开 /model 选择器 overlay（main 消费：picker.open + 拦截按键）。
    OpenModelPicker,
    /// 打开 /provider 表单 overlay（main 消费：form.open + 拦截按键）。
    OpenProviderForm(crate::provider_form::FormKind),
}

/// `/resume` 无参列表最多呈现的会话条数（反馈区是定高的一次性渲染带，
/// 多于此的行会被 `FeedbackRing` 淘汰，故显式截断避免"列表被静默吃掉"）。
const RESUME_LIST_LIMIT: usize = 6;

/// 命令分发上下文：装配产物 + 待答复一次性句柄。
/// 生命周期同装配（会话切换 = 整体重建）。
pub struct Commands {
    pub runtime: Arc<Runtime>,
    pub tools_handle: onion::tools::ToolRegistry,
    pub goal_service: Arc<GoalService>,
    pub goal_driver: Arc<GoalRoundDriver>,
    pub plan_mode: Arc<PlanMode>,
    pub approval_config: Arc<std::sync::RwLock<ApprovalConfig>>,
    pub hooks_bridge: Option<Arc<onion::hooks_claude::ClaudeCodeHooksListener>>,
    pub hook_cancel: Option<CancellationToken>,
    pub subagent_runtime: Arc<onion::subagent::runtime::SubagentRuntime>,
    pub compaction_listener: Arc<onion::compaction::listener::CompactionListener>,
    pub compact_tx: mpsc::UnboundedSender<()>,
    pub busy: Arc<AtomicBool>,
    /// 审批 reviewer LLM（/permission 重建 ApprovalConfig 需要）。
    pub reviewer_llm: Arc<OpenAiSseLlm>,
    /// 审批 reviewer 模型名。
    pub reviewer_model: String,
    /// 运行期模型目标引用（/model 切换：swap_current + 写回 settings）。
    pub model_target: Arc<onion::model_target::ModelTargetRef>,
    /// 模型目录（/model 选择器数据源）。
    pub provider_catalog: onion::provider_catalog::ProviderCatalog,
    /// 上下文预算（/status 快照用，Config 透传）。
    pub context_window_tokens: u64,
    /// 当前 Agent id（/agent 查询与切换）。
    pub active_agent: String,
    /// 完整 Agent 目录（/agent 选择器：id 列表 + 解析）。
    pub agent_catalog: onion::preset::AgentCatalog,
    /// 会话切换目标（/new /resume 预检后由 main 消费重建装配）。
    pub switch: Option<Switch>,
    /// 在飞审批的一次性答复句柄（同一时刻至多一个，内核串行保证）；
    /// 会话退出前 drop → 等待端 recv Err → fail-closed 拒绝。
    pub pending_approval: Option<oneshot::Sender<ApprovalDecision>>,
    /// 在飞 plan-review 的一次性答复句柄（与审批共用 /approve /deny）。
    pub pending_plan_review: Option<oneshot::Sender<PlanVerdict>>,
    /// 在飞 ask_user 问询（timed 超时后答复改为注入 inbox）。
    pub pending_ask_user: Option<PendingAskUser>,
}

/// 宿主通道（main 循环 select! 消费）：待答复请求的接收端。
///
/// 独立于 Commands：select! 分支需与 `commands.dispatch()` 并发借用，
/// 放同一结构体内会造成可变借用冲突。
pub struct HostChannels {
    /// 工具审批请求（内核挂起等待人工复决）。
    pub approval_rx: mpsc::UnboundedReceiver<onion::listeners::ApprovalRequest>,
    /// plan-review 请求（exit_plan_mode 提交审批）。
    pub plan_review_rx: mpsc::UnboundedReceiver<onion::plan::service::PlanReviewRequest>,
    /// ask_user 问询（阻塞工具等待人工答复）。
    pub ask_user_rx: mpsc::UnboundedReceiver<onion::ask_user::service::AskUserRequest>,
}

/// 会话切换目标（/new /resume 预检通过后置位，main 消费重建装配）。
#[derive(Debug, PartialEq, Eq)]
pub enum Switch {
    New,
    Resume(String),
}

/// 命令反馈行（TUI 渲染到反馈区，非 durable）。
pub type Feedback = Vec<String>;

impl Commands {
    /// 从装配产物构建（字段 move，rx 留给宿主循环 select! 消费）。
    pub fn new(a: Assembled) -> (Self, HostChannels) {
        let channels = HostChannels {
            approval_rx: a.approval_rx,
            plan_review_rx: a.plan_review_rx,
            ask_user_rx: a.ask_user_rx,
        };
        let commands = Self {
            runtime: a.runtime,
            tools_handle: a.tools_handle,
            goal_service: a.goal_service,
            goal_driver: a.goal_driver,
            plan_mode: a.plan_mode,
            approval_config: a.approval_config,
            hooks_bridge: a.hooks_bridge,
            hook_cancel: a.hook_cancel,
            subagent_runtime: a.subagent_runtime,
            compaction_listener: a.compaction_listener,
            compact_tx: a.compact_tx,
            busy: a.busy,
            reviewer_llm: a.reviewer_llm,
            reviewer_model: a.reviewer_model,
            model_target: a.model_target,
            provider_catalog: a.provider_catalog,
            context_window_tokens: a.context_window_tokens,
            active_agent: a.active_agent,
            agent_catalog: a.agent_catalog,
            switch: None,
            pending_approval: None,
            pending_plan_review: None,
            pending_ask_user: None,
        };
        (commands, channels)
    }

    /// 分发一行输入：斜杠命令或普通消息。
    /// 反馈行进 Feedback（宿主渲染），驱动动作经 Action 返回。
    pub async fn dispatch(&mut self, line: &str) -> (Feedback, Action) {
        let mut fb = Feedback::new();
        let action = self.route(line, &mut fb).await;
        (fb, action)
    }

    /// 是否还有待答复的一次性句柄（宿主据此收起问题面板）。
    pub fn has_pending(&self) -> bool {
        self.pending_approval.is_some()
            || self.pending_plan_review.is_some()
            || self.pending_ask_user.is_some()
    }

    async fn route(&mut self, line: &str, fb: &mut Feedback) -> Action {
        if line == "/exit" || line == "exit" || line == "/quit" {
            return Action::Quit;
        }
        if line == "/new" {
            return self.cmd_new(fb);
        }
        if line == "/resume" || line.starts_with("/resume ") {
            return self.cmd_resume(line, fb);
        }
        if let Some(rest) = line.strip_prefix("/agent ") {
            return self.cmd_agent(rest, fb);
        }
        if line == "/agent" {
            return self.cmd_agent("", fb);
        }
        if line == "/permission" || line.starts_with("/permission ") {
            return self.cmd_permission(line, fb);
        }
        if line == "/approve" || line == "/deny" {
            return self.cmd_approve_deny(line, fb);
        }
        if let Some(rest) = line.strip_prefix("/answer ") {
            return self.cmd_answer(rest, fb);
        }
        if let Some(rest) = line.strip_prefix("/steer ") {
            match self.runtime.inbox().steer(AgentMessage::user(rest)) {
                Ok(()) => {
                    let hint = if self.busy.load(Ordering::SeqCst) {
                        "已 steer，下一个 step 领取"
                    } else {
                        "空闲：已 steer，kick 将自动开轮"
                    };
                    fb.push(format!("（{hint}）"));
                }
                Err(error) => fb.push(format!("（steer 失败：{error}）")),
            }
            return Action::Kick;
        }
        if let Some(rest) = line.strip_prefix("/inject ") {
            match self.runtime.inbox().inject(AgentMessage::user(rest)) {
                Ok(()) => fb.push("（已 inject，不唤醒）".to_string()),
                Err(error) => fb.push(format!("（inject 失败：{error}）")),
            }
            return Action::Continue;
        }
        if let Some(rest) = line.strip_prefix("/followup ") {
            return self.cmd_queue(rest, fb);
        }
        if line == "/compact" {
            return self.cmd_compact(fb).await;
        }
        if line == "/goal" || line.starts_with("/goal ") {
            return self.cmd_goal(line, fb);
        }
        if line == "/plan" || line.starts_with("/plan ") {
            return self.cmd_plan(line, fb);
        }
        if line == "/status" {
            return self.cmd_status(fb);
        }
        if line == "/model" || line.starts_with("/model ") {
            return self.cmd_model(line, fb);
        }
        if line == "/provider" || line.starts_with("/provider ") {
            return self.cmd_provider(line, fb);
        }
        // 普通消息：统一经 inbox 投递 + 唤醒（followup），
        // 由 main 消费 Action::Kick 开轮（忙时排队，轮收尾后自动续轮）。
        // 排队成功的提示不打印（transcript 会展示该消息，画面清爽）。
        match self.runtime.inbox().followup(AgentMessage::user(line)) {
            Ok(()) => {}
            Err(error) => fb.push(format!("（投递失败：{error}）")),
        }
        Action::Kick
    }

    /// /new：busy/subagent 预检 → drop pendings → end → Switch::New。
    fn cmd_new(&mut self, fb: &mut Feedback) -> Action {
        if self.busy.load(Ordering::SeqCst) {
            fb.push("（轮进行中，暂不能切换会话；/exit 可退出）".to_string());
            return Action::Continue;
        }
        if self.subagent_runtime.inflight() > 0 {
            fb.push("（有 subagent 在飞，暂不能切换会话）".to_string());
            return Action::Continue;
        }
        // 挂起审批随会话丢弃 → 等待端 recv Err → fail-closed。
        drop(self.pending_approval.take());
        drop(self.pending_plan_review.take());
        drop(self.pending_ask_user.take());
        self.switch = Some(Switch::New);
        // runtime.end 由 main 消费 Switch 时统一执行（对齐 onion-cli：
        // 预检通过 → end → break → 重建装配）。
        Action::Continue
    }

    /// /resume [id]：预检（非当前会话 + 日志存在 + 无在飞）→ Switch::Resume。
    /// 无参列出历史会话（最近活动倒序）。
    fn cmd_resume(&mut self, line: &str, fb: &mut Feedback) -> Action {
        let sessions_dir = onion::paths::sessions_dir(
            &onion::paths::data_home(),
            &std::env::current_dir().unwrap_or_default(),
        );
        let Some(id) = line.strip_prefix("/resume ").map(str::trim).filter(|s| !s.is_empty()) else {
            // 无参：列出历史会话（最近活动倒序，与 onion-cli 同源）。
            let summaries = onion::session::list_sessions(&sessions_dir);
            if summaries.is_empty() {
                fb.push("（暂无历史会话）".to_string());
                return Action::Continue;
            }
            fb.push("历史会话（最近优先）· /resume <id> 恢复：".to_string());
            for s in summaries.iter().take(RESUME_LIST_LIMIT) {
                fb.push(format!(
                    "  {} · {} 事件 · goal={} · {}",
                    s.id, s.events, s.goal_phase, s.title
                ));
            }
            return Action::Continue;
        };
        if id == self.runtime.session().session_id() {
            fb.push(format!("（{id} 已是当前会话）"));
            return Action::Continue;
        }
        if self.busy.load(Ordering::SeqCst) || self.subagent_runtime.inflight() > 0 {
            fb.push("（轮进行中或有 subagent 在飞，暂不能切换会话）".to_string());
            return Action::Continue;
        }
        if !onion::paths::session_log_file(&onion::paths::session_dir(&sessions_dir, id))
            .exists()
        {
            fb.push(format!("（会话不存在：{id}）"));
            return Action::Continue;
        }
        drop(self.pending_approval.take());
        drop(self.pending_plan_review.take());
        drop(self.pending_ask_user.take());
        self.switch = Some(Switch::Resume(id.to_string()));
        Action::Continue
    }

    /// /agent [<id>]：无参列出当前 Agent 与可选 id；带 id 执行侧立即生效。
    fn cmd_agent(&mut self, rest: &str, fb: &mut Feedback) -> Action {
        let id = rest.trim();
        if id.is_empty() {
            fb.push(format!(
                "（当前 Agent：{}；可选：{}）",
                self.active_agent,
                self.agent_catalog.ids().join(" | ")
            ));
            return Action::Continue;
        }
        let Some(agent) = self.agent_catalog.get(id).cloned() else {
            fb.push(format!(
                "（未知 Agent {id}，可选：{}）",
                self.agent_catalog.ids().join(" | ")
            ));
            return Action::Continue;
        };
        self.tools_handle.state().set_agent(agent.tools);
        self.active_agent = id.to_string();
        if let Some(bridge) = &self.hooks_bridge {
            let bridge = bridge.clone();
            let id = id.to_string();
            tokio::spawn(async move {
                bridge.emit_config_change("agent", &id).await;
            });
        }
        fb.push(format!(
            "（已切换 Agent → {id}：执行侧立即生效，装配侧下一轮生效）"
        ));
        Action::Continue
    }

    /// /permission [preset]：审批预设切换（空参列出当前 + 可选）。
    fn cmd_permission(&mut self, line: &str, fb: &mut Feedback) -> Action {
        if line == "/permission" {
            let current = self
                .approval_config
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let name = current.preset.name.clone();
            drop(current);
            fb.push(format!(
                "（当前审批预设：{name}；可选：{}）",
                ApprovalConfig::PRESET_NAMES.join(" | ")
            ));
            return Action::Continue;
        }
        let Some(rest) = line.strip_prefix("/permission ") else {
            return Action::Continue;
        };
        let preset_name = rest.trim();
        if !ApprovalConfig::PRESET_NAMES.contains(&preset_name) {
            fb.push(format!(
                "（未知预设 {preset_name}，可选：{}）",
                ApprovalConfig::PRESET_NAMES.join(" | ")
            ));
            return Action::Continue;
        }
        let mut guard = self
            .approval_config
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = build_approval_config(preset_name, &self.reviewer_llm, &self.reviewer_model.clone());
        drop(guard);
        fb.push(format!("（已切换审批预设 → {preset_name}：沙箱允许集与审批策略即时生效）"));
        Action::Continue
    }

    /// /approve /deny：plan-review 优先（与工具审批共用命令通道），
    /// 工具审批次之。
    fn cmd_approve_deny(&mut self, line: &str, fb: &mut Feedback) -> Action {
        if let Some(reply) = self.pending_plan_review.take() {
            let verdict = if line == "/approve" {
                PlanVerdict::Approve
            } else {
                PlanVerdict::KeepPlanning
            };
            let _ = reply.send(verdict);
            return Action::Continue;
        }
        match self.pending_approval.take() {
            Some(reply) => {
                let decision = if line == "/approve" {
                    ApprovalDecision::AllowedOnce
                } else {
                    ApprovalDecision::Rejected
                };
                let _ = reply.send(decision);
            }
            None => fb.push("（当前无待审批请求）".to_string()),
        }
        Action::Continue
    }

    /// /answer <id> <value>：ask_user 问询答复（多问题一次只答一个，
    /// 其余保持待答；全部答完合并回填宿主）。
    fn cmd_answer(&mut self, rest: &str, fb: &mut Feedback) -> Action {
        let Some(mut pending) = self.pending_ask_user.take() else {
            fb.push("（当前无待答问询）".to_string());
            return Action::Continue;
        };
        let mut questions = std::mem::take(&mut pending.questions);
        let mut answered = std::mem::take(&mut pending.answered);
        let rest = rest.trim();
        // 单问题快捷答复：/answer <编号> 或 /answer <标签/文本>（省略问题 id）。
        // 仅当 rest 不是任何待答问题的 id 时，将其整段作为唯一问题的值。
        let single_question = questions.len() == 1;
        let single_id = single_question
            .then(|| questions[0].id.clone())
            .filter(|id| id != rest);
        let (qid, value) = match rest.split_once(char::is_whitespace) {
            Some((id, value)) => (id.trim().to_string(), value.trim().to_string()),
            None => match single_id {
                Some(id) => (id, rest.to_string()),
                None => (rest.to_string(), String::new()),
            },
        };
        let Some(question) = questions.iter().find(|q| q.id == qid) else {
            fb.push(format!("（无此问题 id：{qid}）"));
            pending.questions = questions;
            pending.answered = answered;
            self.pending_ask_user = Some(pending);
            return Action::Continue;
        };
        if value.is_empty() {
            fb.push("（用法：/answer <id> <选项编号/标签或自由文本>，或单问题直接 /answer <编号/文本>）".to_string());
            pending.questions = questions;
            pending.answered = answered;
            self.pending_ask_user = Some(pending);
            return Action::Continue;
        }
        let selected = if let Some(options) = question.options.as_ref().filter(|o| !o.is_empty()) {
            // 有选项：答复可用选项编号（1-based）或完整标签；多选逗号分隔。
            let labels: Vec<&str> = value.split(',').map(str::trim).collect();
            let unknown: Vec<&str> = labels
                .iter()
                .filter(|l| {
                    // 编号：解析为 1..=len 视为合法；否则须匹配某个标签。
                    match l.parse::<usize>() {
                        Ok(n) => n < 1 || n > options.len(),
                        Err(_) => !options.iter().any(|o| o.label == **l),
                    }
                })
                .copied()
                .collect();
            if !unknown.is_empty() {
                fb.push(format!("（未知选项：{}）", unknown.join(", ")));
                pending.questions = questions;
                pending.answered = answered;
                self.pending_ask_user = Some(pending);
                return Action::Continue;
            }
            labels
                .into_iter()
                .map(|l| {
                    // 编号 → 对应标签（回填引擎的 selected 语义为标签）。
                    match l.parse::<usize>() {
                        Ok(n) => options[n - 1].label.clone(),
                        Err(_) => l.to_string(),
                    }
                })
                .collect()
        } else {
            // 无选项：整段作为自定义答复。
            vec![value]
        };
        answered.push(AskUserAnswerItem { id: qid.to_string(), selected, custom: None });
        questions.retain(|q| q.id != qid);
        if questions.is_empty() {
            let answer = AskUserAnswer { answers: answered };
            if pending.reply.send(answer.clone()).is_err() {
                // timed 前台等待已逾限：迟到答复以 answer_to_pending_question
                // 注入 inbox。
                let message = onion::ask_user::answer_to_pending_message(
                    &pending.call_id,
                    &pending.asked,
                    &answer,
                );
                match self.runtime.inbox().steer(message) {
                    Ok(()) => fb.push("（问询已超时，答复已作为迟到答复注入会话）".to_string()),
                    Err(error) => fb.push(format!("（迟到答复注入失败：{error}）")),
                }
            } else {
                fb.push("（问询已答复）".to_string());
            }
        } else {
            fb.push(format!("（已记录答复，剩余 {} 个问题待答）", questions.len()));
            pending.questions = questions;
            pending.answered = answered;
            self.pending_ask_user = Some(pending);
        }
        Action::Continue
    }

    /// /followup <msg>：显式排队成轮（成功不打印提示，失败才反馈）。
    fn cmd_queue(&mut self, rest: &str, fb: &mut Feedback) -> Action {
        match self.runtime.inbox().followup(AgentMessage::user(rest)) {
            Ok(()) => {}
            Err(error) => fb.push(format!("（followup 失败：{error}）")),
        }
        Action::Kick
    }

    /// /compact：忙时请求压缩（下一 step 准入前执行）；
    /// 空闲时直接执行压缩事务（串行 await，避免与开轮并发破坏 bracket 原子性）。
    async fn cmd_compact(&mut self, fb: &mut Feedback) -> Action {
        if self.busy.load(Ordering::SeqCst) {
            match self.compact_tx.send(()) {
                Ok(()) => fb.push("（已请求压缩：下一个 step 准入前执行）".to_string()),
                Err(_) => fb.push("（compaction listener 未在运行）".to_string()),
            }
            return Action::Continue;
        }
        let session = self.runtime.session().clone();
        self.compaction_listener.compact_now(&session).await;
        Action::Continue
    }

    /// /goal [objective|pause|resume|complete|clear]：goal 命令分发。
    fn cmd_goal(&mut self, line: &str, fb: &mut Feedback) -> Action {
        let rest = line.strip_prefix("/goal ").map(str::trim).unwrap_or("");
        if rest.is_empty() {
            match self.goal_service.view() {
                Some(view) => {
                    fb.push(format!(
                        "当前目标：{}（{} · round {}/{}）",
                        view.snapshot.objective,
                        view.snapshot.phase.label(),
                        view.rounds_started,
                        view.snapshot.max_goal_rounds
                    ));
                    if let Some(reason) = &view.snapshot.blocked_reason {
                        fb.push(format!("  阻塞：{} — {}", reason.code, reason.message));
                    }
                }
                None => fb.push("（当前无目标）".to_string()),
            }
            return Action::Continue;
        }
        match rest {
            "edit" => {
                fb.push("（用法：/goal edit <objective>）".to_string());
                Action::Continue
            }
            r if r.starts_with("edit ") => {
                let objective = r[5..].trim();
                if objective.is_empty() {
                    fb.push("（用法：/goal edit <objective>）".to_string());
                    return Action::Continue;
                }
                let Some(view) = self.goal_service.view() else {
                    fb.push("（当前无目标，请用 /goal <objective> 创建）".to_string());
                    return Action::Continue;
                };
                // 当前 complete 视为替换（重新 create）；否则 edit 修订 objective。
                if view.snapshot.phase.label() == "complete" {
                    match self.goal_service.create(objective.to_string(), view.snapshot.max_goal_rounds) {
                        Ok(snapshot) => {
                            fb.push(format!(
                                "（目标已替换：{} · round 0/{}）",
                                snapshot.objective, snapshot.max_goal_rounds
                            ));
                            return self.start_goal_round(fb);
                        }
                        Err(e) => fb.push(format!("（替换失败：{e}）")),
                    }
                } else {
                    let reference = view.snapshot.ref_of();
                    match self.goal_service.edit(reference, Some(objective.to_string()), None) {
                        Ok(snapshot) => fb.push(format!("（目标已编辑：{}）", snapshot.objective)),
                        Err(e) => fb.push(format!("（编辑失败：{e}）")),
                    }
                }
                Action::Continue
            }
            "pause" => {
                if let Some(reference) = self.goal_service.view().map(|v| v.snapshot.ref_of()) {
                    match self.goal_service.pause(reference) {
                        Ok(_) => fb.push("（目标已暂停）".to_string()),
                        Err(e) => fb.push(format!("（暂停失败：{e}）")),
                    }
                } else {
                    fb.push("（当前无目标）".to_string());
                }
                Action::Continue
            }
            "resume" => {
                if let Some(reference) = self.goal_service.view().map(|v| v.snapshot.ref_of()) {
                    match self.goal_service.resume(reference) {
                        Ok(_) => {
                            fb.push("（目标已恢复）".to_string());
                            return self.start_goal_round(fb);
                        }
                        Err(e) => fb.push(format!("（恢复失败：{e}）")),
                    }
                } else {
                    fb.push("（当前无目标）".to_string());
                }
                Action::Continue
            }
            "complete" => {
                if let Some(reference) = self.goal_service.view().map(|v| v.snapshot.ref_of()) {
                    match self.goal_service.complete(reference) {
                        Ok(_) => fb.push("（目标已完成）".to_string()),
                        Err(e) => fb.push(format!("（完成失败：{e}）")),
                    }
                } else {
                    fb.push("（当前无目标）".to_string());
                }
                Action::Continue
            }
            "clear" => {
                if let Some(reference) = self.goal_service.view().map(|v| v.snapshot.ref_of()) {
                    match self.goal_service.clear(reference) {
                        Ok(_) => fb.push("（目标已清除）".to_string()),
                        Err(e) => fb.push(format!("（清除失败：{e}）")),
                    }
                } else {
                    fb.push("（当前无目标）".to_string());
                }
                Action::Continue
            }
            _ => {
                // 视为创建/替换目标：objective = rest，max_goal_rounds 默认 256。
                match self.goal_service.create(rest.to_string(), 256) {
                    Ok(snapshot) => {
                        fb.push(format!(
                            "（目标已创建：{} · round 0/{}）",
                            snapshot.objective, snapshot.max_goal_rounds
                        ));
                        return self.start_goal_round(fb);
                    }
                    Err(e) => fb.push(format!("（创建失败：{e}）")),
                }
                Action::Continue
            }
        }
    }

    /// goal 轮启动（create / edit 替换 / resume 共用）。goal 首轮消息 followup
    /// 进 inbox（带 GoalRound source 锚点 + continuation prompt），由 kick 空轮
    /// 消费；CAS 预留（record_round）在 followup 前建立，由 kernel pre-step
    /// 边界校验。
    fn start_goal_round(&mut self, fb: &mut Feedback) -> Action {
        if self.busy.load(Ordering::SeqCst) {
            // 轮进行中设置/恢复目标：turn 循环边界的 should_continue 检查会接管
            // （此时目标已 armed），无需立即拉起。
            fb.push("（当前轮进行中，结束后自动续跑 goal 轮）".to_string());
            return Action::Continue;
        }
        self.goal_driver.service_enter();
        let Some(message) = self.goal_driver.continuation_input() else {
            self.goal_driver.service_exit();
            fb.push("（无法启动 goal 轮：轮预算耗尽或目标相位非 active）".to_string());
            return Action::Continue;
        };
        // 进入 inbox 前 CAS 预留 round；失败说明目标被并发变更，放弃本轮。
        if self.goal_driver.record_round().is_err() {
            self.goal_driver.service_exit();
            fb.push("（目标已被并发变更，放弃启动本轮）".to_string());
            return Action::Continue;
        }
        // followup 进 inbox + 唤醒；kick 分支会以空轮消费。
        if self.runtime.inbox().followup(message).is_err() {
            self.goal_driver.service_exit();
            fb.push("（goal 消息入 inbox 失败）".to_string());
            return Action::Continue;
        }
        fb.push("（goal 轮已入队，kick 将自动开轮）".to_string());
        Action::Kick
    }

    /// /status：durable 事件快照折叠（terminal-only，设计文档 §10）。
    fn cmd_status(&mut self, fb: &mut Feedback) -> Action {
        let events = self.runtime.session().snapshot();
        let snap = crate::status::StatusSnapshot::from_events(&events, self.context_window_tokens);
        for line in snap.render() {
            fb.push(line);
        }
        Action::Continue
    }

    /// /model [id]：带参直接切换目标（写回 settings + append ModelSelection 事件）；
    /// 无参打开选择器 overlay。
    fn cmd_model(&mut self, line: &str, fb: &mut Feedback) -> Action {
        let rest = line.strip_prefix("/model ").map(str::trim).filter(|s| !s.is_empty());
        let Some(model) = rest else {
            fb.push("（/model 选择器已打开）".to_string());
            return Action::OpenModelPicker;
        };
        // 带参：目录校验 + 置换 current + 写回 settings + append durable 事件。
        let Some(target) = self.provider_catalog.target_for(model) else {
            fb.push(format!("（模型目录中无 {model}，列出可用模型后重试）"));
            return Action::Continue;
        };
        let label = format!("{} @ {}", target.model, target.base_url);
        self.model_target.swap_current(target.clone());
        // 写回 settings.json（失败仅 warn，不影响本会话）。
        let persisted = onion::settings::PersistedModelTarget {
            model: target.model.clone(),
            base_url: target.base_url.clone(),
            effort: target.effort.clone(),
        };
        if let Err(error) =
            onion::settings::SettingsFile::save_model_target(&onion::paths::data_home(), &persisted)
        {
            fb.push(format!("（模型写回 settings 失败，仅本次会话生效：{error}）"));
        }
        // append durable ModelSelection 事件（log-only）。
        self.runtime
            .session()
            .append(onion::session::SessionEvent::ModelSelection {
                model: target.model.clone(),
                base_url: target.base_url.clone(),
                effort: target.effort,
            });
        fb.push(format!("（已切换模型目标 → {label}，下一个请求生效）"));
        Action::Continue
    }

    /// /provider [provider|model]：打开新增供应商/模型表单 overlay。
    /// 无参默认打开供应商表单；`/provider model [provider_id]` 打开模型表单。
    fn cmd_provider(&mut self, line: &str, fb: &mut Feedback) -> Action {
        let rest = line.strip_prefix("/provider ").map(str::trim).unwrap_or("");
        if rest.starts_with("model") {
            // 可选 provider_id 预填（缺省用当前模型所属 provider 或留空）。
            let provider_id = rest
                .strip_prefix("model")
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToString::to_string)
                .unwrap_or_default();
            fb.push(format!(
                "（/provider 模型表单已打开{}）",
                if provider_id.is_empty() {
                    "".to_string()
                } else {
                    format!("，供应商 {provider_id}")
                }
            ));
            return Action::OpenProviderForm(crate::provider_form::FormKind::Model);
        }
        fb.push("（/provider 供应商表单已打开）".to_string());
        Action::OpenProviderForm(crate::provider_form::FormKind::Provider)
    }

    /// /plan [off|<msg>]：plan 模式切换（无打印版本，对齐 onion::plan::commands
    /// 同一投影语义）。
    fn cmd_plan(&mut self, line: &str, fb: &mut Feedback) -> Action {
        let Some(command) = onion::plan::commands::parse_plan_command(line) else {
            return Action::Continue;
        };
        match command {
            onion::plan::commands::PlanCommand::Enter { message } => {
                self.apply_plan_switch(true, fb);
                // 附带消息：followup 进 inbox。
                let mut kicked = false;
                if let Some(msg) = message.filter(|m| !m.is_empty()) {
                    match self.runtime.inbox().followup(AgentMessage::user(msg)) {
                        Ok(()) => kicked = true,
                        Err(error) => {
                            fb.push(format!("（plan 附带消息入 inbox 失败：{error}）"));
                        }
                    }
                }
                if kicked {
                    return Action::Kick;
                }
            }
            onion::plan::commands::PlanCommand::Exit => {
                self.apply_plan_switch(false, fb);
            }
        }
        Action::Continue
    }

    /// 应用 plan 模式切换（busy 挂 pending / 空闲立即生效）+ 持久化 wanted。
    /// 语义对齐 onion::plan::commands::apply_switch，反馈收集到 fb。
    fn apply_plan_switch(&mut self, active: bool, fb: &mut Feedback) {
        if let Some(session) = self.plan_mode.session_arc() {
            append_plan_wanted(session, active);
        }
        if self.busy.load(Ordering::SeqCst) {
            match self.plan_mode.set(active) {
                SetResult::Queued => fb.push("（已在轮内切换 plan 模式，将在下一 step 边界生效）".to_string()),
                SetResult::Noop => fb.push("（plan 模式未变化）".to_string()),
                SetResult::Committed => {
                    // 理论不可达（busy 时 set 应返回 Queued）；防御性反馈。
                    fb.push("（plan 切换已提交）".to_string());
                }
            }
        } else if active {
            self.plan_mode.enter();
            fb.push("（已进入 plan 模式：产出计划后用 exit_plan_mode 提交审批）".to_string());
        } else {
            self.plan_mode.exit();
            fb.push("（已退出 plan 模式）".to_string());
        }
    }

}

/// 反馈行环形缓冲（cap 8）：命令分发反馈 / turn 报告 / 审批提示统一进
/// 这里渲染（非 durable，不进 transcript）。
pub struct FeedbackRing {
    lines: VecDeque<String>,
    cap: usize,
}

impl FeedbackRing {
    pub fn new() -> Self {
        Self { lines: VecDeque::new(), cap: 8 }
    }

    pub fn push(&mut self, line: String) {
        if self.lines.len() >= self.cap {
            self.lines.pop_front();
        }
        self.lines.push_back(line);
    }

    /// 批量追加（命令分发反馈），溢出按插入顺序淘汰最旧行。
    pub fn extend(&mut self, lines: impl IntoIterator<Item = String>) {
        for line in lines {
            self.push(line);
        }
    }

    pub fn lines(&self) -> impl Iterator<Item = &String> {
        self.lines.iter()
    }
}

/// 斜杠命令目录（`dispatch` 分支的展示快照，hints 菜单同源）：命令签名 +
/// 一句话描述。命令名列全 ASCII，保证两列对齐不受 CJK 宽度影响。
pub const COMMAND_CATALOG: &[(&str, &str)] = &[
    ("/new", "新建会话（收尾当前会话）"),
    ("/resume [id]", "恢复历史会话（无参列出）"),
    ("/model [id]", "切换模型（无参开选择器）"),
    ("/provider [model]", "新增供应商/模型（无参开表单）"),
    ("/agent [id]", "切换 Agent（无参列出可选 id）"),
    ("/steer <msg>", "运行中引导（下一 step 生效）"),
    ("/inject <msg>", "注入消息（不唤醒新轮）"),
    ("/followup <msg>", "排队为下一轮"),
    ("/compact", "请求上下文压缩"),
    ("/goal [obj]", "查看 / 创建 / 编辑目标"),
    ("/plan [msg]", "plan 模式（enter / exit）"),
    ("/permission [...]", "审批预设管理"),
    ("/status", "会话状态一览"),
    ("/approve", "批准当前待决审批"),
    ("/deny", "拒绝当前待决审批"),
    ("/answer <id> <val>", "答复 ask_user 问询"),
    ("/exit", "退出"),
];

/// 命令名两列布局的列宽（目录内最长命令名 + 2 空格间距）。
const HINT_CMD_COL: usize = 20;
/// 提示菜单最多展示条数（防长目录溢屏）。
const HINT_MAX: usize = 8;

/// 命令提示行：缓冲以 `/` 开头时按前缀过滤目录（最多 `HINT_MAX` 条），
/// 整行渲染为 `/name  描述` 两列暗绿（命令名列全 ASCII 保证对齐，对齐
/// 参考稿补全菜单）。非斜杠前缀返回空（普通消息输入不提示）。
pub fn command_hints(buffer: &str) -> Vec<Line> {
    if !buffer.starts_with('/') {
        return Vec::new();
    }
    COMMAND_CATALOG
        .iter()
        .filter(|(name, _)| name.starts_with(buffer))
        .take(HINT_MAX)
        .map(|(name, desc)| {
            Line::styled(
                format!("{name:<HINT_CMD_COL$}{desc}"),
                Style::new().color(Color::DarkGreen),
            )
        })
        .collect()
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
