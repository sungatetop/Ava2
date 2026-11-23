//! `hooks_claude` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//! 用真实子进程（echo / exit 2）验证边界映射与审计事件对；不依赖 LLM。

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::ClaudeCodeHooksListener;
use crate::hooks::config::SubstitutionVars;
use crate::hooks::runner::run_hook;
use crate::hooks::types::CommandHook;
use crate::listener::{ClaimedInput, ClaimSource, Listener, ListenerCtx, PreStepDecision, ToolAdmission};
use crate::session::{LiveEvent, SessionEvent, SessionLog};
use crate::types::{AgentMessage, ToolCall};
use crate::waterfall::Next;

fn in_memory_session() -> Arc<SessionLog> {
    Arc::new(SessionLog::in_memory())
}

fn config_with(point: &str, command: &str) -> serde_json::Value {
    json!({
        point: [ { "hooks": [ { "type": "command", "command": command } ] } ]
    })
}

/// 收集 metrics 风格的审计事件对。
fn audit_pairs(session: &SessionLog) -> (Vec<SessionEvent>, Vec<SessionEvent>) {
    let mut invoked = Vec::new();
    let mut results = Vec::new();
    for event in session.snapshot() {
        match event {
            SessionEvent::HookInvoked { .. } => invoked.push(event),
            SessionEvent::HookResult { .. } => results.push(event),
            _ => {}
        }
    }
    (invoked, results)
}

/// 轮询等待 detached on_event spawn 落定（audit 事件对齐期望条数），
/// 带截止时间，避免全量并行下固定 sleep 的调度竞态。
async fn wait_for_audit_pairs(session: &SessionLog, expected: usize) -> (Vec<SessionEvent>, Vec<SessionEvent>) {
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        let pairs = audit_pairs(session);
        if pairs.1.len() >= expected || std::time::Instant::now() > deadline {
            return pairs;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

#[tokio::test]
async fn run_point_emits_invoked_result_audit_pair() {
    let session = in_memory_session();
    let raw = config_with("PreToolUse", "echo ok");
    let bridge = ClaudeCodeHooksListener::from_json(&raw, &SubstitutionVars::default(), session.clone())
        .expect("parses");
    let outcome = bridge.run_point("PreToolUse", "write_file", json!({ "x": 1 })).await;
    // 无决策（echo ok 纯 stdout）
    assert_eq!(outcome.decision, crate::hooks::types::MergedDecision::None);
    let (invoked, results) = audit_pairs(&session);
    assert_eq!(invoked.len(), 1);
    assert_eq!(results.len(), 1);
    if let SessionEvent::HookInvoked { point, matcher, .. } = &invoked[0] {
        assert_eq!(point, "PreToolUse");
        assert_eq!(matcher.as_deref(), None);
    }
    if let SessionEvent::HookResult { point, decision, exit_code, .. } = &results[0] {
        assert_eq!(point, "PreToolUse");
        assert_eq!(decision, "pass");
        assert_eq!(*exit_code, Some(0));
    }
}

#[tokio::test]
async fn no_matching_hook_emits_nothing() {
    let session = in_memory_session();
    let raw = config_with("PreToolUse", "echo ok");
    let bridge = ClaudeCodeHooksListener::from_json(&raw, &SubstitutionVars::default(), session.clone())
        .expect("parses");
    // 事件点无配置 → 中性结果，无审计。
    let outcome = bridge.run_point("Stop", "", json!({})).await;
    assert_eq!(outcome.decision, crate::hooks::types::MergedDecision::None);
    let (invoked, results) = audit_pairs(&session);
    assert!(invoked.is_empty() && results.is_empty());
}

#[tokio::test]
async fn exit_two_hook_decodes_as_deny() {
    let session = in_memory_session();
    #[cfg(windows)]
    let command = "echo blocked 1>&2 & exit 2";
    #[cfg(not(windows))]
    let command = "echo blocked 1>&2; exit 2";
    let raw = config_with("PreToolUse", command);
    let bridge = ClaudeCodeHooksListener::from_json(&raw, &SubstitutionVars::default(), session.clone())
        .expect("parses");
    let outcome = bridge.run_point("PreToolUse", "write_file", json!({})).await;
    assert_eq!(outcome.decision, crate::hooks::types::MergedDecision::Deny);
    assert_eq!(outcome.reason.as_deref(), Some("blocked"));
}

// —— Listener 边界映射 ——

fn claimed(turn: u32, step: u32, messages: Vec<AgentMessage>) -> ClaimedInput {
    ClaimedInput {
        turn,
        step,
        source: ClaimSource::TurnOpen,
        messages,
        starts_request_series: true,
    }
}

#[tokio::test]
async fn pre_step_deny_rejects() {
    let session = in_memory_session();
    #[cfg(windows)]
    let command = "echo no 1>&2 & exit 2";
    #[cfg(not(windows))]
    let command = "echo no 1>&2; exit 2";
    let raw = config_with("UserPromptSubmit", command);
    let bridge = ClaudeCodeHooksListener::from_json(&raw, &SubstitutionVars::default(), session.clone())
        .expect("parses");
    let ctx = ListenerCtx {
        inbox: Arc::new(crate::inbox::Inbox::new(session.bus().clone())),
        session: session.clone(),
        state: String::new(),
    };
    let input = claimed(1, 1, vec![AgentMessage::user("hello")]);
    let next: Next<ClaimedInput, PreStepDecision> = Box::new(move |input| Box::pin(async move {
        PreStepDecision::Enter { messages: input.messages, starts_request_series: true }
    }));
    let decision = bridge.pre_step(ctx, input, next).await;
    assert!(matches!(decision, PreStepDecision::Reject { .. }));
}

#[tokio::test]
async fn pre_step_allow_enters() {
    let session = in_memory_session();
    let raw = config_with("UserPromptSubmit", "echo ok");
    let bridge = ClaudeCodeHooksListener::from_json(&raw, &SubstitutionVars::default(), session.clone())
        .expect("parses");
    let ctx = ListenerCtx {
        inbox: Arc::new(crate::inbox::Inbox::new(session.bus().clone())),
        session: session.clone(),
        state: String::new(),
    };
    let input = claimed(1, 1, vec![AgentMessage::user("hello")]);
    let next: Next<ClaimedInput, PreStepDecision> = Box::new(move |input| Box::pin(async move {
        PreStepDecision::Enter { messages: input.messages, starts_request_series: true }
    }));
    let decision = bridge.pre_step(ctx, input, next).await;
    let PreStepDecision::Enter { messages, .. } = decision else {
        panic!("expected Enter");
    };
    assert_eq!(messages.len(), 1);
}

#[tokio::test]
async fn tool_deny_rejects_tool() {
    let session = in_memory_session();
    #[cfg(windows)]
    let command = "echo no 1>&2 & exit 2";
    #[cfg(not(windows))]
    let command = "echo no 1>&2; exit 2";
    let raw = config_with("PreToolUse", command);
    let bridge = ClaudeCodeHooksListener::from_json(&raw, &SubstitutionVars::default(), session.clone())
        .expect("parses");
    let ctx = ListenerCtx {
        inbox: Arc::new(crate::inbox::Inbox::new(session.bus().clone())),
        session: session.clone(),
        state: String::new(),
    };
    let call = ToolCall { id: "c1".into(), name: "write_file".into(), input: json!({}) };
    let next: Next<ToolCall, ToolAdmission> = Box::new(move |call| Box::pin(async move {
        ToolAdmission::Allow { call }
    }));
    let admission = bridge.tool(ctx, call, next).await;
    assert!(matches!(admission, ToolAdmission::Deny { .. }));
}

#[tokio::test]
async fn tool_allow_passes_through() {
    let session = in_memory_session();
    let raw = config_with("PreToolUse", "echo ok");
    let bridge = ClaudeCodeHooksListener::from_json(&raw, &SubstitutionVars::default(), session.clone())
        .expect("parses");
    let ctx = ListenerCtx {
        inbox: Arc::new(crate::inbox::Inbox::new(session.bus().clone())),
        session: session.clone(),
        state: String::new(),
    };
    let call = ToolCall { id: "c1".into(), name: "echo".into(), input: json!({}) };
    let next: Next<ToolCall, ToolAdmission> = Box::new(move |call| Box::pin(async move {
        ToolAdmission::Allow { call }
    }));
    let admission = bridge.tool(ctx, call, next).await;
    assert!(matches!(admission, ToolAdmission::Allow { .. }));
}

// —— 协议层 runner 冒烟（真实子进程）——

#[tokio::test]
async fn runner_executes_echo() {
    let hook = CommandHook { command: "echo hello".to_string(), timeout_sec: None };
    let result = run_hook(
        &hook,
        crate::hooks::runner::RunHookOptions {
            payload: json!({}),
            env: Vec::new(),
            cwd: None,
            cancel: CancellationToken::new(),
            trailing_newline: true,
            default_timeout_ms: 5000,
            expected_event_name: None,
        },
    )
    .await;
    assert_eq!(result.output.exit_code, Some(0));
    assert_eq!(result.output.stdout, "hello");
}

#[tokio::test]
async fn runner_timeout_is_non_blocking() {
    #[cfg(windows)]
    let command = "ping -n 10 127.0.0.1 >nul".to_string();
    #[cfg(not(windows))]
    let command = "sleep 10".to_string();
    let hook = CommandHook { command, timeout_sec: None };
    let result = run_hook(
        &hook,
        crate::hooks::runner::RunHookOptions {
            payload: json!({}),
            env: Vec::new(),
            cwd: None,
            cancel: CancellationToken::new(),
            trailing_newline: true,
            // timeout 2000ms << ping 9s：消除全量并行下的调度延迟竞态。
            default_timeout_ms: 2000,
            expected_event_name: None,
        },
    )
    .await;
    assert_eq!(result.output.exit_code, None);
    assert_eq!(result.output.decision, None);
}

// —— 纯观测事件（12/30 扩展：SessionEnd / Notification / ConfigChange）——

#[tokio::test]
async fn session_end_observation_fires_hook() {
    let session = in_memory_session();
    let raw = config_with("SessionEnd", "echo bye");
    let bridge = ClaudeCodeHooksListener::from_json(&raw, &SubstitutionVars::default(), session.clone())
        .expect("parses");
    // 桥作为 listener 收到 durable SessionEnd → dispatch SessionEnd 钩子。
    bridge.on_event(
        &ListenerCtx {
            inbox: Arc::new(crate::inbox::Inbox::new(session.bus().clone())),
            session: session.clone(),
            state: String::new(),
        },
        &LiveEvent::Durable(SessionEvent::SessionEnd { reason: "user exit".to_string() }),
    );
    // detached：轮询等待在飞 spawn 落定。
    let (invoked, results) = wait_for_audit_pairs(&session, 1).await;
    assert_eq!(invoked.len(), 1);
    assert_eq!(results.len(), 1);
    if let SessionEvent::HookInvoked { point, .. } = &invoked[0] {
        assert_eq!(point, "SessionEnd");
    }
}

#[tokio::test]
async fn notification_observation_fires_hook() {
    let session = in_memory_session();
    let raw = config_with("Notification", "echo notified");
    let bridge = ClaudeCodeHooksListener::from_json(&raw, &SubstitutionVars::default(), session.clone())
        .expect("parses");
    bridge.on_event(
        &ListenerCtx {
            inbox: Arc::new(crate::inbox::Inbox::new(session.bus().clone())),
            session: session.clone(),
            state: String::new(),
        },
        &LiveEvent::StateChanged { from: "Idle".to_string(), to: "Executing".to_string() },
    );
    let (invoked, results) = wait_for_audit_pairs(&session, 1).await;
    assert_eq!(invoked.len(), 1);
    assert_eq!(results.len(), 1);
    if let SessionEvent::HookInvoked { point, .. } = &invoked[0] {
        assert_eq!(point, "Notification");
    }
}

#[tokio::test]
async fn config_change_explicit_dispatch_fires_hook() {
    let session = in_memory_session();
    let raw = config_with("ConfigChange", "echo reloaded");
    let bridge = ClaudeCodeHooksListener::from_json(&raw, &SubstitutionVars::default(), session.clone())
        .expect("parses");
    bridge.emit_config_change("mode", "work").await;
    let (invoked, results) = audit_pairs(&session);
    assert_eq!(invoked.len(), 1);
    assert_eq!(results.len(), 1);
    if let SessionEvent::HookInvoked { point, .. } = &invoked[0] {
        assert_eq!(point, "ConfigChange");
    }
}

// 供编译期确认 live 事件枚举可消费（桥的 on_event 观测）。
#[allow(dead_code)]
fn _observe(event: &LiveEvent) -> Option<String> {
    match event {
        LiveEvent::Durable(SessionEvent::SessionStarted { session_id, .. }) => Some(session_id.clone()),
        _ => None,
    }
}

// 保留 Duration 引用（跨平台超时语义文档化）。
const _: Duration = Duration::from_secs(5);
