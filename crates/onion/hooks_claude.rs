//! Claude Code 兼容桥：把 `hooks.json` 命令钩子挂到 onion Listener 边界。
//!
//! 以 Listener 实现（`ctx.on(point)` 的
//! onion 等价）监听 harness 既有扩展点，内核零改动。payload 构造、env、matcher
//! 模式、typed 决策映射在本桥自持；共享协议见 `hooks/`。
//!
//! 支持事件映射（7/30）：
//! - SessionStart      → SessionStarted（detached 观测，上下文注入）
//! - UserPromptSubmit  → pre_step（deny → Reject）
//! - PreToolUse        → tool（deny → Deny）
//! - PostToolUse       → tool_post（block + feedback + context 注入）
//! - Stop              → turn_stopping（deny → steer 强制继续）
//! - SubagentStart/Stop→ 预留扩展点（onion 无子 agent，仅记录）
//!
//! 设计文档：`ava/docs/ava-core/plan/onion-command-hooks-implementation.md`。

use std::sync::Arc;

use futures_util::FutureExt;
use tokio_util::sync::CancellationToken;

use crate::hooks::config::{
    parse_claude_code_config, ClaudeCodeHookConfig, SkippedHook, SubstitutionVars,
};
use crate::hooks::matcher::select_hook_groups;
use crate::hooks::merge::merge_hook_outputs;
use crate::hooks::runner::{run_hook, RunHookOptions, DEFAULT_HOOK_TIMEOUT_MS};
use crate::hooks::types::{CommandHook, HookDialect, MatcherMode, MergedDecision, MergedHookOutcome};
use crate::listener::{ClaimedInput, Listener, ListenerCtx, PreStepDecision, ToolAdmission};
use crate::reminders::{escape_reminder_body, system_reminder};
use crate::session::{LiveEvent, SessionEvent, SessionLog};
use crate::types::{AgentMessage, MessageSource, ToolCall, ToolResult};
use crate::waterfall::{BoxFuture, Next};

/// hook additionalContext → reminder 帧注入（source=HooksContext）。
fn hook_context_message(ctx: &str) -> AgentMessage {
    AgentMessage {
        role: crate::types::Role::User,
        blocks: vec![crate::types::ContentBlock::Text {
            text: system_reminder(&escape_reminder_body(ctx)),
        }],
        source: Some(MessageSource::HooksContext),
    }
}

/// stderr 摘要（带截断上限）。
fn summarize(stderr: &str) -> Option<String> {
    let trimmed = stderr.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut s = trimmed.to_string();
    if s.chars().count() > 500 {
        s = s.chars().take(500).collect::<String>();
        s.push('…');
    }
    Some(s)
}

/// Claude Code 兼容桥：把 `hooks.json` 命令钩子挂到 onion Listener 边界。
/// 仅 command hooks 运行；非 command 钩子解析后告警跳过（faithful-but-degraded）。
pub struct ClaudeCodeHooksListener {
    /// 事件名 → matcher 组（仅 command hooks）。
    config: ClaudeCodeHookConfig,
    /// 被跳过的非 command 钩子（启动时告警）。
    skipped: Vec<SkippedHook>,
    /// 会话日志（append 审计事件对）。
    session: Arc<SessionLog>,
    /// 取消句柄（detached 运行在 dispose 时 abort）。
    cancel: CancellationToken,
    /// 默认超时（ms）。
    default_timeout_ms: u64,
}

impl ClaudeCodeHooksListener {
    /// 从 `hooks.json` 的解析 JSON 构造桥。`vars` 应用于命令字符串替换
    /// （plugin_root / project_dir）。解析失败返回 Err（启动期拒绝）。
    pub fn from_json(
        raw: &serde_json::Value,
        vars: &SubstitutionVars,
        session: Arc<SessionLog>,
    ) -> Result<Self, String> {
        let parsed = parse_claude_code_config(raw, vars)?;
        Ok(Self {
            config: parsed.config,
            skipped: parsed.skipped,
            session,
            cancel: CancellationToken::new(),
            default_timeout_ms: DEFAULT_HOOK_TIMEOUT_MS,
        })
    }

    /// 取消句柄（供宿主在 dispose 时 abort 在飞 detached 钩子）。
    pub fn cancel_handle(&self) -> CancellationToken {
        self.cancel.clone()
    }

    /// 对 `event` 点的全部命中钩子串行运行，合并后返回。
    /// 审计 `hook/invoked` + `hook/result`（log-only）。无命中钩子返回中性结果。
    async fn run_point(&self, point: &str, query: &str, payload: serde_json::Value) -> MergedHookOutcome {
        let Some(groups) = self.config.get(point) else {
            return MergedHookOutcome::default();
        };
        let matched = select_hook_groups(Some(groups), query, MatcherMode::ClaudeCode);
        if matched.is_empty() {
            return MergedHookOutcome::default();
        }
        let mut outputs = Vec::new();
        let mut handler_id = 0usize;
        for group in &matched {
            let matcher = group.matcher.clone();
            for hook in &group.hooks {
                handler_id += 1;
                let hid = format!("{point}:{handler_id}");
                // 审计：hook/invoked。
                self.session.append(SessionEvent::HookInvoked {
                    point: point.to_string(),
                    handler_id: hid.clone(),
                    matcher: matcher.clone(),
                });
                let result = run_hook(
                    hook,
                    RunHookOptions {
                        payload: payload.clone(),
                        env: self.env_for(hook, point),
                        cwd: None,
                        cancel: self.cancel.clone(),
                        trailing_newline: true,
                        default_timeout_ms: self.default_timeout_ms,
                        expected_event_name: Some(point.to_string()),
                    },
                )
                .await;
                // 审计：hook/result。
                self.session.append(SessionEvent::HookResult {
                    point: point.to_string(),
                    handler_id: hid,
                    decision: decision_label_from_output(&result.output),
                    exit_code: result.output.exit_code,
                    stderr_summary: summarize(&result.output.stderr),
                    duration_ms: result.duration_ms,
                });
                outputs.push(result.output);
            }
        }
        merge_hook_outputs(&outputs)
    }

    /// 桥 env：向钩子暴露方言环境变量。
    fn env_for(&self, _hook: &CommandHook, _point: &str) -> Vec<(String, String)> {
        vec![
            ("ONION_HOOK_DIALECT".to_string(), dialect_name(HookDialect::ClaudeCode).to_string()),
        ]
    }
}

/// 结构化输出决策 → 审计标签（含 stop 与退出码）。
fn decision_label_from_output(output: &crate::hooks::types::HookOutput) -> String {
    match output.decision {
        Some(crate::hooks::types::HookDecision::Deny | crate::hooks::types::HookDecision::Block) => {
            if output.continue_ == Some(false) {
                "stop".to_string()
            } else {
                "deny".to_string()
            }
        }
        Some(crate::hooks::types::HookDecision::Ask) => "ask".to_string(),
        Some(crate::hooks::types::HookDecision::Approve | crate::hooks::types::HookDecision::Allow) => {
            "allow".to_string()
        }
        None => "pass".to_string(),
    }
}

fn dialect_name(dialect: HookDialect) -> &'static str {
    match dialect {
        HookDialect::ClaudeCode => "claude-code",
        HookDialect::Codex => "codex",
    }
}

/// ConfigChange：部署侧配置变更显式触发（main `/mode` 命令）。
/// onion 无配置热更新总线，桥提供 pub API 由宿主旁路调用。
impl ClaudeCodeHooksListener {
    pub async fn emit_config_change(&self, field: &str, value: &str) {
        let _ = self
            .run_point(
                "ConfigChange",
                field,
                serde_json::json!({ "field": field, "value": value }),
            )
            .await;
    }
}

impl Listener for ClaudeCodeHooksListener {
    fn on_event(&self, _ctx: &ListenerCtx, event: &LiveEvent) {
        // SessionStart（SessionStarted durable）：detached 观测。onion 无子
        // agent，此处不注入上下文（预留）。仅记录被跳过钩子告警一次。
        if let LiveEvent::Durable(SessionEvent::SessionStarted { .. }) = event {
            for skipped in &self.skipped {
                tracing::warn!(
                    event = %skipped.event,
                    hook_type = %skipped.hook_type,
                    "skipped non-command hook (faithful-but-degraded)"
                );
            }
        }
        // SessionEnd：durable 收尾事件旁路（纯观测转发）。
        if let LiveEvent::Durable(SessionEvent::SessionEnd { reason }) = event {
            let this = self.clone_inner();
            let reason = reason.clone();
            tokio::spawn(async move {
                let _ = this
                    .run_point(
                        "SessionEnd",
                        &reason,
                        serde_json::json!({
                            "session_id": this.session.session_id(),
                            "reason": reason,
                        }),
                    )
                    .await;
            });
        }
        // Notification：状态迁移 live 事件旁路（纯观测转发）。
        if let LiveEvent::StateChanged { from, to } = event {
            let this = self.clone_inner();
            let message = format!("{from} → {to}");
            tokio::spawn(async move {
                let _ = this
                    .run_point(
                        "Notification",
                        &message,
                        serde_json::json!({ "message": message, "title": "onion" }),
                    )
                    .await;
            });
        }
    }

    /// UserPromptSubmit → pre_step：deny → Reject。
    fn pre_step(
        &self,
        _ctx: ListenerCtx,
        input: ClaimedInput,
        next: Next<ClaimedInput, PreStepDecision>,
    ) -> BoxFuture<PreStepDecision> {
        let this = self.clone_inner();
        async move {
            let query = input.message_text();
            let outcome = this
                .run_point("UserPromptSubmit", &query, serde_json::json!({ "prompt": query }))
                .await;
            if matches!(outcome.decision, MergedDecision::Deny | MergedDecision::Ask) {
                let reason = outcome.reason.unwrap_or_else(|| "blocked by UserPromptSubmit hook".to_string());
                return PreStepDecision::Reject { reason };
            }
            // Enter：把 additionalContext 注入为本步的额外 user 输入（前置）。
            // 多个钩子都产出上下文时全部注入（对齐 Claude Code：拼接而非取首条）。
            let mut messages = input.messages;
            if !outcome.additional_context.is_empty() {
                let joined = outcome.additional_context.join("\n\n");
                messages.insert(0, hook_context_message(&joined));
            }
            drop(next);
            PreStepDecision::Enter { messages, starts_request_series: input.starts_request_series }
        }
        .boxed()
    }

    /// PreToolUse → tool：deny → Deny。
    fn tool(
        &self,
        _ctx: ListenerCtx,
        call: ToolCall,
        next: Next<ToolCall, ToolAdmission>,
    ) -> BoxFuture<ToolAdmission> {
        let this = self.clone_inner();
        async move {
            let outcome = this
                .run_point("PreToolUse", &call.name, serde_json::json!({
                    "tool_name": call.name,
                    "tool_input": call.input,
                }))
                .await;
            if matches!(outcome.decision, MergedDecision::Deny | MergedDecision::Ask) {
                let reason = outcome.reason.unwrap_or_else(|| "blocked by PreToolUse hook".to_string());
                return ToolAdmission::Deny { call, reason };
            }
            drop(next);
            ToolAdmission::Allow { call }
        }
        .boxed()
    }

    /// PostToolUse → tool_post：block + feedback + additionalContext 注入下一次 step。
    fn tool_post(&self, ctx: ListenerCtx, call: &ToolCall, result: &ToolResult) -> BoxFuture<()> {
        let this = self.clone_inner();
        let call = call.clone();
        let result_text = result.text_content();
        async move {
            let outcome = this
                .run_point("PostToolUse", &call.name, serde_json::json!({
                    "tool_name": call.name,
                    "tool_input": call.input,
                    "tool_response": result_text,
                }))
                .await;
            // block/deny → feedback + context 注入下一次 step（steer 继续）。
            if matches!(outcome.decision, MergedDecision::Deny | MergedDecision::Ask) {
                let mut text = vec![];
                if let Some(reason) = &outcome.reason {
                    text.push(hook_context_message(&format!("[hook feedback]\n{reason}")));
                }
                for ctx_str in &outcome.additional_context {
                    text.push(hook_context_message(ctx_str));
                }
                if !text.is_empty() {
                    let _ = ctx.inbox.steer(join_hook_context_messages(&text));
                }
            }
        }
        .boxed()
    }

    /// Stop → turn_stopping：deny → steer 强制继续。
    fn turn_stopping(&self, ctx: ListenerCtx) -> BoxFuture<()> {
        let this = self.clone_inner();
        async move {
            let outcome = this
                .run_point("Stop", "", serde_json::json!({ "stop_hook_active": true }))
                .await;
            if matches!(outcome.decision, MergedDecision::Deny | MergedDecision::Ask) {
                // 强制继续：注入一条 next-step 消息，让轮继续而非关闭。
                let reason = outcome.reason.unwrap_or_else(|| "steered by Stop hook".to_string());
                let _ = ctx.inbox.steer(hook_context_message(&format!("[hook: continue]\n{reason}")));
            }
        }
        .boxed()
    }
}

/// 多条 hook context 消息合并为一条 steer（对齐原 text.join("\n\n")）。
fn join_hook_context_messages(messages: &[AgentMessage]) -> AgentMessage {
    let text = messages
        .iter()
        .map(AgentMessage::text_content)
        .collect::<Vec<_>>()
        .join("\n\n");
    AgentMessage::user(text)
}

/// 桥内部状态（Arc 包装以便 `clone_inner` 在 BoxFuture 中共享）。
type Inner = Arc<ClaudeCodeHooksListener>;

impl ClaudeCodeHooksListener {
    fn clone_inner(&self) -> Inner {
        Arc::new(ClaudeCodeHooksListener {
            config: self.config.clone(),
            skipped: self.skipped.clone(),
            session: self.session.clone(),
            cancel: self.cancel.clone(),
            default_timeout_ms: self.default_timeout_ms,
        })
    }
}

// 供 pre_step 的 query 使用。
impl ClaimedInput {
    fn message_text(&self) -> String {
        self.messages
            .iter()
            .map(AgentMessage::text_content)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
#[path = "hooks_claude_tests.rs"]
mod tests;
