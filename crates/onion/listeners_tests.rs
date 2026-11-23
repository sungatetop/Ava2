//! `listeners` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）：
//! ApprovalGate 审批流 + EnvInjector 环境基线注入。

use std::sync::{Arc, RwLock};

use serde_json::json;
use tokio::sync::mpsc;

use super::{ApprovalConfig, ApprovalGate, ApprovalRequest, EnvInjector, ToolRisk};
use crate::approval::{PresetSpec, SandboxLevel};
use crate::inbox::Inbox;
use crate::listener::{Listener, ListenerCtx, ToolAdmission};
use crate::session::{LiveEvent, SessionEvent, SessionLog};
use crate::session_cwd::SessionCwd;
use crate::types::ToolCall;
use crate::waterfall::{BoxFuture, Next};

fn ctx() -> ListenerCtx {
    let session = Arc::new(SessionLog::in_memory());
    // 审批必须发生在打开的轮次内（gate 对轮次外请求 fail-closed），测试统一
    // 以 TurnStart 打开轮次；需要轮次外语义的测试自行重开裸 session。
    session.append(SessionEvent::TurnStart { turn: 1 });
    let bus = session.bus().clone();
    ListenerCtx { inbox: Arc::new(Inbox::new(bus)), session, state: "test".to_string() }
}

fn call(name: &str) -> ToolCall {
    ToolCall { id: "c1".to_string(), name: name.to_string(), input: json!({ "path": "a.txt" }) }
}

/// 内层直通处理器：无论输入如何都放行（验证 gate 决策本身）。
fn pass_through() -> Next<ToolCall, ToolAdmission> {
    Box::new(|call| Box::pin(async move { ToolAdmission::Allow { call } }) as BoxFuture<ToolAdmission>)
}

/// 构造 gate + 请求接收端（Phase E-2 起需外部传入发送端）：
/// 沙箱 read-only——`write_file`(WorkspaceWrite) 升级审批，`echo`(ReadOnly) 直通。
fn gate_with_requests() -> (ApprovalGate, mpsc::UnboundedReceiver<ApprovalRequest>) {
    let mut preset = PresetSpec::workspace_write();
    preset.sandbox = SandboxLevel::ReadOnly;
    let config = Arc::new(RwLock::new(ApprovalConfig { preset, reviewer: None }));
    let classify: Arc<dyn Fn(&ToolCall) -> ToolRisk + Send + Sync> = Arc::new(|call: &ToolCall| {
        if call.name == "write_file" { ToolRisk::WorkspaceWrite } else { ToolRisk::ReadOnly }
    });
    let (tx, rx) = mpsc::unbounded_channel();
    (ApprovalGate::new(config, classify, tx), rx)
}

#[tokio::test]
async fn approval_passes_through_when_not_in_policy() {
    let (gate, mut requests) = gate_with_requests();
    let decision = gate.tool(ctx(), call("echo"), pass_through()).await;
    assert!(matches!(decision, ToolAdmission::Allow { .. }));
    assert!(requests.try_recv().is_err(), "no approval request expected");
}

#[tokio::test]
async fn approval_grants_on_approve_reply() {
    let (gate, mut requests) = gate_with_requests();
    let handle = tokio::spawn(gate.tool(ctx(), call("write_file"), pass_through()));
    let request = requests.recv().await.unwrap_or_else(|| panic!("approval request expected"));
    assert_eq!(request.name, "write_file");
    request.reply.send(super::ApprovalDecision::AllowedOnce).unwrap_or_else(|_| panic!("reply send failed"));
    let decision = handle.await.unwrap_or_else(|error| panic!("task panicked: {error}"));
    assert!(matches!(decision, ToolAdmission::Allow { .. }));
}

#[tokio::test]
async fn approval_denies_on_deny_reply() {
    let (gate, mut requests) = gate_with_requests();
    let handle = tokio::spawn(gate.tool(ctx(), call("write_file"), pass_through()));
    let request = requests.recv().await.unwrap_or_else(|| panic!("approval request expected"));
    request.reply.send(super::ApprovalDecision::Rejected).unwrap_or_else(|_| panic!("reply send failed"));
    let decision = handle.await.unwrap_or_else(|error| panic!("task panicked: {error}"));
    match decision {
        ToolAdmission::Deny { reason, .. } => assert_eq!(reason, "rejected by operator"),
        ToolAdmission::Allow { .. } => panic!("deny reply must not allow"),
    }
}

#[tokio::test]
async fn approval_fails_closed_when_reply_dropped() {
    let (gate, mut requests) = gate_with_requests();
    let handle = tokio::spawn(gate.tool(ctx(), call("write_file"), pass_through()));
    let request = requests.recv().await.unwrap_or_else(|| panic!("approval request expected"));
    drop(request); // reply sender 随请求一起丢弃（如会话退出前 main 侧清理）
    let decision = handle.await.unwrap_or_else(|error| panic!("task panicked: {error}"));
    match decision {
        ToolAdmission::Deny { reason, .. } => assert_eq!(reason, "approval cancelled (fail-closed)"),
        ToolAdmission::Allow { .. } => panic!("dropped reply must fail closed"),
    }
}

#[tokio::test]
async fn approval_denies_when_request_channel_closed() {
    let (gate, requests) = gate_with_requests();
    drop(requests); // main 侧接收端先行关闭
    let decision = gate.tool(ctx(), call("write_file"), pass_through()).await;
    match decision {
        ToolAdmission::Deny { reason, .. } => assert_eq!(reason, "approval channel closed"),
        ToolAdmission::Allow { .. } => panic!("closed channel must fail closed"),
    }
}

// ========== 阶段 1：会话级授权（approved-for-session） ==========

fn ctx_of(session: &Arc<SessionLog>) -> ListenerCtx {
    // 审批必须发生在打开的轮次内（gate 对轮次外请求 fail-closed），测试统一
    // 以 TurnStart 打开轮次（幂等：同一 session 多次进入仍是一个打开轮次）。
    session.append(SessionEvent::TurnStart { turn: 1 });
    ListenerCtx {
        inbox: Arc::new(Inbox::new(session.bus().clone())),
        session: session.clone(),
        state: "test".to_string(),
    }
}

/// shell 场景 gate：沙箱 read-only + 全部 `Escalating`，使 `bash` 必须走人工审批。
fn shell_gate() -> (ApprovalGate, mpsc::UnboundedReceiver<ApprovalRequest>) {
    let mut preset = PresetSpec::workspace_write();
    preset.sandbox = SandboxLevel::ReadOnly;
    let config = Arc::new(RwLock::new(ApprovalConfig { preset, reviewer: None }));
    let classify: Arc<dyn Fn(&ToolCall) -> ToolRisk + Send + Sync> = Arc::new(|_: &ToolCall| ToolRisk::Escalating);
    let (tx, rx) = mpsc::unbounded_channel();
    (ApprovalGate::new(config, classify, tx), rx)
}

fn shell_call(command: &str) -> ToolCall {
    ToolCall { id: "c1".to_string(), name: "bash".to_string(), input: json!({ "command": command }) }
}

fn reply(request: ApprovalRequest, decision: super::ApprovalDecision) {
    request.reply.send(decision).unwrap_or_else(|_| panic!("reply send failed"));
}

/// 「本会话允许」按命令首词生效：同族命令直通，其它命令仍逐次审批，且审计标记
/// `operator-session`。
#[tokio::test]
async fn allowed_for_session_covers_same_command_family() {
    let session = Arc::new(SessionLog::in_memory());
    let (gate, mut requests) = shell_gate();
    let handle = tokio::spawn(gate.tool(ctx_of(&session), shell_call("sleep 0.5"), pass_through()));
    let request = requests.recv().await.unwrap_or_else(|| panic!("approval request expected"));
    reply(request, super::ApprovalDecision::AllowedForSession);
    let decision = handle.await.unwrap_or_else(|error| panic!("task panicked: {error}"));
    assert!(matches!(decision, ToolAdmission::Allow { .. }));

    // 同族命令（同上首词）：直通，不再产生审批请求。
    let decision = gate.tool(ctx_of(&session), shell_call("sleep 1"), pass_through()).await;
    assert!(matches!(decision, ToolAdmission::Allow { .. }));
    assert!(requests.try_recv().is_err(), "same command family must not ask again");

    // 不同命令：仍需审批。
    let handle = tokio::spawn(gate.tool(ctx_of(&session), shell_call("mkdir build"), pass_through()));
    let request = requests.recv().await.unwrap_or_else(|| panic!("other command must ask again"));
    reply(request, super::ApprovalDecision::Rejected);
    let decision = handle.await.unwrap_or_else(|error| panic!("task panicked: {error}"));
    assert!(matches!(decision, ToolAdmission::Deny { .. }));

    let via: Vec<String> = session
        .snapshot()
        .iter()
        .filter_map(|event| match event {
            crate::session::SessionEvent::ApprovalAudited { via, .. } => Some(via.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(via, vec!["operator-session".to_string(), "operator-session".to_string(), "operator".to_string()]);
}

/// 破坏性命令即使收到 `allowed-for-session` 也只放行本次（不落会话授权）。
#[tokio::test]
async fn allowed_for_session_never_covers_destructive_commands() {
    let (gate, mut requests) = shell_gate();
    let handle = tokio::spawn(gate.tool(ctx(), shell_call("rm -rf build"), pass_through()));
    let request = requests.recv().await.unwrap_or_else(|| panic!("approval request expected"));
    reply(request, super::ApprovalDecision::AllowedForSession);
    let decision = handle.await.unwrap_or_else(|error| panic!("task panicked: {error}"));
    assert!(matches!(decision, ToolAdmission::Allow { .. }));

    let handle = tokio::spawn(gate.tool(ctx(), shell_call("rm -rf dist"), pass_through()));
    let request = requests.recv().await.unwrap_or_else(|| panic!("destructive command must ask again"));
    reply(request, super::ApprovalDecision::Rejected);
    let decision = handle.await.unwrap_or_else(|error| panic!("task panicked: {error}"));
    assert!(matches!(decision, ToolAdmission::Deny { .. }));
}

/// 在飞去重：同键并发调用共享一次人工裁决（只弹一张卡）。
#[tokio::test(flavor = "current_thread")]
async fn concurrent_same_key_requests_share_one_decision() {
    let (gate, mut requests) = shell_gate();
    let gate = Arc::new(gate);

    let first = tokio::spawn(gate.tool(ctx(), shell_call("sleep 0.5"), pass_through()));
    let request = requests.recv().await.unwrap_or_else(|| panic!("approval request expected"));
    let second = tokio::spawn(gate.tool(ctx(), shell_call("sleep 0.5"), pass_through()));
    tokio::task::yield_now().await; // 让第二调用注册为跟随者
    reply(request, super::ApprovalDecision::AllowedOnce);

    let first = first.await.unwrap_or_else(|error| panic!("task panicked: {error}"));
    let second = second.await.unwrap_or_else(|error| panic!("task panicked: {error}"));
    assert!(matches!(first, ToolAdmission::Allow { .. }));
    assert!(matches!(second, ToolAdmission::Allow { .. }));
    assert!(requests.try_recv().is_err(), "in-flight dedupe must reuse one decision");
}

/// EnvInjector 注入的 environment cwd 必须来自会话 [`SessionCwd`]，而非宿主
/// 进程 cwd——多会话部署（onion-server）下进程 cwd 是装配目录，注入它曾让
/// 模型自报失真的工作目录（用户侧表现为「工作区切换未生效」）。
#[test]
fn env_injector_reports_session_cwd_not_process_cwd() {
    let root = std::env::temp_dir().join(format!("onion_env_injector_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root)
        .unwrap_or_else(|error| panic!("create temp root failed: {error}"));
    let ctx = ctx();
    let injector = EnvInjector::with_session_cwd(Arc::new(SessionCwd::new(root.clone())));
    injector.on_event(
        &ctx,
        &LiveEvent::Durable(SessionEvent::SessionStarted {
            session_id: "s1".to_string(),
            model: "test-model".to_string(),
        }),
    );
    let injected = ctx.inbox.drain_next_step();
    assert_eq!(injected.len(), 1, "SessionStarted must inject exactly one baseline");
    let text = match &injected[0].blocks[0] {
        crate::types::ContentBlock::Text { text } => text.clone(),
        other => panic!("injected baseline must be text, got {other:?}"),
    };
    let session_cwd_text = root.display().to_string();
    assert!(
        text.contains(&session_cwd_text),
        "baseline cwd must be the session cwd ({session_cwd_text}), got: {text}"
    );
    let _ = std::fs::remove_dir_all(&root);
}
