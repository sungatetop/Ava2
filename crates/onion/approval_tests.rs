//! `approval` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::collections::HashMap;
use std::sync::RwLock;

use serde_json::json;
use tokio::sync::mpsc;

use super::*;
use crate::inbox::Inbox;
use crate::listener::{Listener, ListenerCtx, ToolAdmission};
use crate::listeners::{ApprovalGate, ApprovalRequest};
use crate::llm::{AssistantTurn, LlmPort, LlmStream, LlmStreamItem};
use crate::session::{SessionEvent, SessionLog};
use crate::types::{StopReason, Usage};
use crate::waterfall::Next;
use tokio_util::sync::CancellationToken;

/// decide 全矩阵：3 风险 × 3 沙箱 × 2 策略 = 18 组合逐格断言（判定内核纯函数，
/// 表驱动覆盖全部语义格）。
#[test]
fn decide_full_matrix() {
    use SandboxLevel::{DangerFullAccess, ReadOnly as Ro, WorkspaceWrite as Ww};
    use ToolRisk::{Escalating, ReadOnly, WorkspaceWrite};
    let cases: &[(ToolRisk, SandboxLevel, ApprovalPolicy, Decision)] = &[
        // sandbox = read-only：仅只读直通；写/升级按策略 Escalate | Deny
        (ReadOnly, Ro, ApprovalPolicy::Ask, Decision::Allow),
        (ReadOnly, Ro, ApprovalPolicy::Never, Decision::Allow),
        (WorkspaceWrite, Ro, ApprovalPolicy::Ask, Decision::Escalate),
        (WorkspaceWrite, Ro, ApprovalPolicy::Never, Decision::Deny),
        (Escalating, Ro, ApprovalPolicy::Ask, Decision::Escalate),
        (Escalating, Ro, ApprovalPolicy::Never, Decision::Deny),
        // sandbox = workspace-write：只读 + 工作区写直通（工作区写入免审批语义）
        (ReadOnly, Ww, ApprovalPolicy::Ask, Decision::Allow),
        (ReadOnly, Ww, ApprovalPolicy::Never, Decision::Allow),
        (WorkspaceWrite, Ww, ApprovalPolicy::Ask, Decision::Allow),
        (WorkspaceWrite, Ww, ApprovalPolicy::Never, Decision::Allow),
        (Escalating, Ww, ApprovalPolicy::Ask, Decision::Escalate),
        (Escalating, Ww, ApprovalPolicy::Never, Decision::Deny),
        // sandbox = danger-full-access：全放行（审批策略不参与）
        (ReadOnly, DangerFullAccess, ApprovalPolicy::Ask, Decision::Allow),
        (ReadOnly, DangerFullAccess, ApprovalPolicy::Never, Decision::Allow),
        (WorkspaceWrite, DangerFullAccess, ApprovalPolicy::Ask, Decision::Allow),
        (WorkspaceWrite, DangerFullAccess, ApprovalPolicy::Never, Decision::Allow),
        (Escalating, DangerFullAccess, ApprovalPolicy::Ask, Decision::Allow),
        (Escalating, DangerFullAccess, ApprovalPolicy::Never, Decision::Allow),
    ];
    for (risk, sandbox, policy, expected) in cases {
        assert_eq!(
            decide(*risk, *sandbox, *policy),
            *expected,
            "risk={risk:?} sandbox={sandbox:?} policy={policy:?}"
        );
    }
}

/// fail-closed 铁律：`Never` 策略下允许集外的操作一律直接拒绝，永不放行。
#[test]
fn never_policy_fails_closed_outside_allow_set() {
    assert_eq!(
        decide(ToolRisk::Escalating, SandboxLevel::WorkspaceWrite, ApprovalPolicy::Never),
        Decision::Deny
    );
    assert_eq!(
        decide(ToolRisk::WorkspaceWrite, SandboxLevel::ReadOnly, ApprovalPolicy::Never),
        Decision::Deny
    );
}

/// PresetSpec 四内置预设的语义：默认表行为逐项断言。
#[test]
fn preset_defaults_match_reference() {
    let ww = PresetSpec::workspace_write();
    assert_eq!(ww.name, "workspace-write");
    assert_eq!(ww.sandbox, SandboxLevel::WorkspaceWrite);
    assert_eq!(ww.approval, ApprovalPolicy::Ask);

    let auto = PresetSpec::auto();
    assert_eq!(auto.name, "auto");
    assert_eq!(auto.sandbox, SandboxLevel::WorkspaceWrite);
    assert_eq!(auto.approval, ApprovalPolicy::Ask);

    let dfa = PresetSpec::danger_full_access();
    assert_eq!(dfa.name, "danger-full-access");
    assert_eq!(dfa.sandbox, SandboxLevel::DangerFullAccess);
    assert_eq!(dfa.approval, ApprovalPolicy::Never);
}

/// decide_full：deny > allow > 沙箱允许集 > 审批策略 的优先级逐层断言。
#[test]
fn decide_full_applies_restriction_priority() {
    // deny 优先：即使 danger-full-access 下也强制拒绝
    let preset = PresetSpec::custom(
        SandboxLevel::DangerFullAccess,
        ApprovalPolicy::Never,
        ToolRestriction { allow: vec![], deny: vec!["rm".to_string()] },
    );
    assert_eq!(decide_full("rm", ToolRisk::Escalating, &preset), Decision::Deny);

    // allow 覆盖沙箱：read-only 下强制放行升级操作
    let preset = PresetSpec::custom(
        SandboxLevel::ReadOnly,
        ApprovalPolicy::Ask,
        ToolRestriction { allow: vec!["write_file".to_string()], deny: vec![] },
    );
    assert_eq!(decide_full("write_file", ToolRisk::WorkspaceWrite, &preset), Decision::Allow);

    // allow 与 deny 冲突时 deny 胜
    let preset = PresetSpec::custom(
        SandboxLevel::DangerFullAccess,
        ApprovalPolicy::Ask,
        ToolRestriction { allow: vec!["exec".to_string()], deny: vec!["exec".to_string()] },
    );
    assert_eq!(decide_full("exec", ToolRisk::Escalating, &preset), Decision::Deny);
}

/// HeuristicReviewer：allow 名单命中放行，未命中拒绝（fail-closed）。
#[tokio::test]
async fn heuristic_reviewer_allows_listed_only() {
    let reviewer = HeuristicReviewer::new(vec!["write_file".to_string()]);
    let call = |name: &str| ToolCall { id: "c1".to_string(), name: name.to_string(), input: json!({}) };
    assert_eq!(reviewer.review(call("write_file")).await, ApprovalDecision::AllowedOnce);
    assert_eq!(reviewer.review(call("rm")).await, ApprovalDecision::Rejected);
}

/// ApprovalConfig::for_preset：各预设的裁决者接线——`auto` 附带 reviewer，
/// 其余预设无 reviewer（升级操作走人工复决）。
#[test]
fn for_preset_wires_reviewer_only_for_auto() {
    let ww = ApprovalConfig::for_preset("workspace-write");
    assert_eq!(ww.preset.name, "workspace-write");
    assert!(ww.reviewer.is_none());

    let auto = ApprovalConfig::for_preset("auto");
    assert_eq!(auto.preset.name, "auto");
    assert!(auto.reviewer.is_some());

    let dfa = ApprovalConfig::for_preset("danger-full-access");
    assert_eq!(dfa.preset.name, "danger-full-access");
    assert!(dfa.reviewer.is_none());

    let unknown = ApprovalConfig::for_preset("bogus");
    assert_eq!(unknown.preset.name, "workspace-write"); // 回退默认
    assert!(unknown.reviewer.is_none());
}

// ========== 阶段 1：会话级授权键（allowed-for-session） ==========

#[test]
fn session_grant_uses_first_word_for_simple_commands() {
    // 轮询循环里的 `sleep 0.5` / `sleep 1` 属同一授权族。
    assert_eq!(session_grant("bash", &json!({ "command": "sleep 0.5" })), Some("bash:sleep".to_string()));
    assert_eq!(session_grant("bash", &json!({ "command": "  Sleep   1 " })), Some("bash:sleep".to_string()));
}

#[test]
fn session_grant_never_covers_destructive_commands() {
    for cmd in ["rm -rf build", "sudo apt update", "del /s /q x", "format d:", "kill 1234"] {
        assert_eq!(session_grant("bash", &json!({ "command": cmd })), None, "cmd: {cmd}");
    }
}

#[test]
fn session_grant_keeps_composite_commands_exact() {
    // 组合命令只按完整命令授权，避免 `sleep 0.5 && rm -rf build` 借首词扩散。
    let key = session_grant("bash", &json!({ "command": "sleep 0.5 && rm -rf build" }));
    assert_eq!(key, Some("bash:sleep 0.5 && rm -rf build".to_string()));
    assert_ne!(key, Some("bash:sleep".to_string()));
}

#[test]
fn session_grant_falls_back_to_tool_name_without_command() {
    assert_eq!(session_grant("write_file", &json!({ "path": "a.txt" })), Some("write_file".to_string()));
    assert_eq!(session_grant("bash", &json!({ "command": "   " })), None);
}

// ========== Phase D1：审批审计事件 ==========

fn audited(session: &SessionLog) -> Vec<(String, String, String)> {
    session
        .snapshot()
        .iter()
        .filter_map(|event| match event {
            SessionEvent::ApprovalAudited { tool, decision, via } => {
                Some((tool.clone(), decision.clone(), via.clone()))
            }
            _ => None,
        })
        .collect()
}

fn gate_ctx(session: &Arc<SessionLog>) -> (Arc<RwLock<ApprovalConfig>>, ListenerCtx) {
    let config = Arc::new(RwLock::new(ApprovalConfig::for_preset("workspace-write")));
    // 审批必须发生在打开的轮次内（#25）：测试上下文先开一轮，模拟真实工具调用。
    session.append(crate::session::SessionEvent::TurnStart { turn: 1 });
    let ctx = ListenerCtx {
        inbox: Arc::new(Inbox::new(session.bus().clone())),
        session: session.clone(),
        state: String::new(),
    };
    (config, ctx)
}

fn passthrough() -> Next<ToolCall, ToolAdmission> {
    Box::new(|call| Box::pin(async move { ToolAdmission::Allow { call } }))
}

/// 把「工具名 → 风险」静态映射转成 ApprovalGate 需要的分类闭包（按名取风险）。
fn classifier(map: HashMap<String, ToolRisk>) -> Arc<dyn Fn(&ToolCall) -> ToolRisk + Send + Sync> {
    Arc::new(move |call: &ToolCall| map.get(&call.name).copied().unwrap_or(ToolRisk::ReadOnly))
}

/// 构造审批 gate + 请求接收端（Phase E-2 起 `ApprovalGate::new` 需外部传入发送端）。
fn gate_pair(
    config: Arc<RwLock<ApprovalConfig>>,
    classify: Arc<dyn Fn(&ToolCall) -> ToolRisk + Send + Sync>,
) -> (ApprovalGate, mpsc::UnboundedReceiver<ApprovalRequest>) {
    let (tx, rx) = mpsc::unbounded_channel();
    (ApprovalGate::new(config, classify, tx), rx)
}

/// 允许集直通：workspace-write 下 write_file 免审批放行，落 `allowed-once/sandbox` 审计。
#[tokio::test]
async fn gate_audits_sandbox_allow() {
    let session = Arc::new(SessionLog::in_memory());
    let (config, ctx) = gate_ctx(&session);
    let (gate, _rx) = gate_pair(
        config,
        classifier(HashMap::from([("write_file".to_string(), ToolRisk::WorkspaceWrite)])),
    );
    let call = ToolCall { id: "c1".to_string(), name: "write_file".to_string(), input: json!({}) };
    let admission = gate.tool(ctx, call, passthrough()).await;
    assert!(matches!(admission, ToolAdmission::Allow { .. }));
    assert_eq!(audited(&session), vec![("write_file".to_string(), "allowed-once".to_string(), "sandbox".to_string())]);
}

/// deny 规则优先：即使 danger-full-access 也拒绝，落 `denied/restriction` 审计。
#[tokio::test]
async fn gate_audits_restriction_deny() {
    let session = Arc::new(SessionLog::in_memory());
    let (config, ctx) = gate_ctx(&session);
    config.write().unwrap_or_else(std::sync::PoisonError::into_inner).preset =
        PresetSpec::custom(SandboxLevel::DangerFullAccess, ApprovalPolicy::Never, ToolRestriction {
            allow: vec![],
            deny: vec!["exec".to_string()],
        });
    let (gate, _rx) = gate_pair(
        config,
        classifier(HashMap::from([("exec".to_string(), ToolRisk::Escalating)])),
    );
    let call = ToolCall { id: "c1".to_string(), name: "exec".to_string(), input: json!({}) };
    let admission = gate.tool(ctx, call, passthrough()).await;
    assert!(matches!(admission, ToolAdmission::Deny { .. }));
    assert_eq!(audited(&session), vec![("exec".to_string(), "rejected".to_string(), "restriction".to_string())]);
}

/// reviewer 裁决路径：升级操作交裁决者，落 `allowed-once|rejected/reviewer` 审计。
#[tokio::test]
async fn gate_audits_reviewer_verdict() {
    // allow 名单命中 → allowed/reviewer
    let session = Arc::new(SessionLog::in_memory());
    let (config, ctx) = gate_ctx(&session);
    config.write().unwrap_or_else(std::sync::PoisonError::into_inner).reviewer =
        Some(Arc::new(HeuristicReviewer::new(vec!["exec".to_string()])));
    let (gate, _rx) = gate_pair(
        config,
        classifier(HashMap::from([("exec".to_string(), ToolRisk::Escalating)])),
    );
    let call = ToolCall { id: "c1".to_string(), name: "exec".to_string(), input: json!({}) };
    let admission = gate.tool(ctx, call, passthrough()).await;
    assert!(matches!(admission, ToolAdmission::Allow { .. }));
    assert_eq!(audited(&session), vec![("exec".to_string(), "allowed-once".to_string(), "reviewer".to_string())]);

    // 名单未命中 → denied/reviewer
    let session = Arc::new(SessionLog::in_memory());
    let (config, ctx) = gate_ctx(&session);
    config.write().unwrap_or_else(std::sync::PoisonError::into_inner).reviewer =
        Some(Arc::new(HeuristicReviewer::new(vec![])));
    let (gate, _rx) = gate_pair(
        config,
        classifier(HashMap::from([("exec".to_string(), ToolRisk::Escalating)])),
    );
    let call = ToolCall { id: "c2".to_string(), name: "exec".to_string(), input: json!({}) };
    let admission = gate.tool(ctx, call, passthrough()).await;
    assert!(matches!(admission, ToolAdmission::Deny { .. }));
    assert_eq!(audited(&session), vec![("exec".to_string(), "rejected".to_string(), "reviewer".to_string())]);
}

/// 轮次外审批：fail-closed 拒绝且**不落审计**（拒绝发生在写入任何审计记录之前）。
#[tokio::test]
async fn gate_rejects_approval_outside_open_turn() {
    let session = Arc::new(SessionLog::in_memory());
    let config = Arc::new(RwLock::new(ApprovalConfig::for_preset("workspace-write")));
    // 不开轮：直接构造上下文（不经 gate_ctx）。
    let ctx = ListenerCtx {
        inbox: Arc::new(Inbox::new(session.bus().clone())),
        session: session.clone(),
        state: String::new(),
    };
    let (gate, _rx) = gate_pair(
        config,
        classifier(HashMap::from([("exec".to_string(), ToolRisk::Escalating)])),
    );
    let call = ToolCall { id: "c1".to_string(), name: "exec".to_string(), input: json!({}) };
    let admission = gate.tool(ctx, call, passthrough()).await;
    let ToolAdmission::Deny { reason, .. } = admission else {
        panic!("轮次外审批应 fail-closed 拒绝");
    };
    assert!(reason.contains("outside an open turn"), "reason={reason}");
    assert!(audited(&session).is_empty(), "轮次外审批不应落审计事件");
}

/// fail-closed：审批通道关闭时拒绝执行，落 `unavailable/fail-closed` 审计。
#[tokio::test]
async fn gate_audits_fail_closed_on_channel_drop() {
    let session = Arc::new(SessionLog::in_memory());
    let (config, ctx) = gate_ctx(&session);
    let (gate, rx) = gate_pair(
        config,
        classifier(HashMap::from([("exec".to_string(), ToolRisk::Escalating)])),
    );
    drop(rx); // 审批答复端关闭 → send 失败 → fail-closed
    let call = ToolCall { id: "c1".to_string(), name: "exec".to_string(), input: json!({}) };
    let admission = gate.tool(ctx, call, passthrough()).await;
    assert!(matches!(admission, ToolAdmission::Deny { .. }));
    assert_eq!(audited(&session), vec![("exec".to_string(), "unavailable".to_string(), "fail-closed".to_string())]);
}

// ========== Phase D2：LlmReviewer ==========

/// 确定性 fake LLM：返回固定文本（或错误）的裁决响应。
struct VerdictLlm {
    text: &'static str,
    error: bool,
}

impl LlmPort for VerdictLlm {
    fn stream(&self, _draft: &crate::context::RequestDraft, _cancel: CancellationToken) -> crate::waterfall::BoxFuture<LlmStream> {
        let mut turn = AssistantTurn {
            message: crate::types::AgentMessage::assistant(vec![ContentBlock::Text {
                text: self.text.to_string(),
            }]),
            stop_reason: StopReason::EndTurn,
            error: None,
            failure: None,
            cancelled: false,
            usage: Usage::default(),
            model: "fake-model".to_string(),
        };
        if self.error {
            turn.error = Some("boom".to_string());
        }
        Box::pin(async move {
            Box::pin(futures_util::stream::iter(vec![LlmStreamItem::Done(turn)])) as LlmStream
        })
    }
}

/// parse_verdict 表驱动：`allow` 前缀放行，deny/空/垃圾一律拒绝。
#[test]
fn parse_verdict_table() {
    let cases: &[(&str, ApprovalDecision)] = &[
        ("allow", ApprovalDecision::AllowedOnce),
        ("ALLOW", ApprovalDecision::AllowedOnce),
        ("allow: 输入合理", ApprovalDecision::AllowedOnce),
        ("  allowed", ApprovalDecision::AllowedOnce),
        ("deny", ApprovalDecision::Rejected),
        ("", ApprovalDecision::Rejected),
        ("我不确定", ApprovalDecision::Rejected),
    ];
    for (text, expected) in cases {
        assert_eq!(LlmReviewer::parse_verdict(text), *expected, "text={text:?}");
    }
}

/// LlmReviewer 全链路：模型回复 allow 放行；deny / 错误落定 fail-closed 拒绝。
#[tokio::test]
async fn llm_reviewer_verdict_and_fail_closed() {
    let call = ToolCall { id: "c1".to_string(), name: "exec".to_string(), input: json!({"cmd": "ls"}) };
    let allow = LlmReviewer::new(Arc::new(VerdictLlm { text: "allow", error: false }), "fake-model");
    assert_eq!(allow.review(call.clone()).await, ApprovalDecision::AllowedOnce);

    let deny = LlmReviewer::new(Arc::new(VerdictLlm { text: "deny", error: false }), "fake-model");
    assert_eq!(deny.review(call.clone()).await, ApprovalDecision::Rejected);

    let error = LlmReviewer::new(Arc::new(VerdictLlm { text: "allow", error: true }), "fake-model");
    assert_eq!(error.review(call).await, ApprovalDecision::Rejected);
}

// ========== #26：审批 outcome 闭集 ==========

/// 闭集词汇表：四个字面值，顺序固定。
#[test]
fn approval_outcome_vocabulary_matches_reference() {
    let words: Vec<&str> = ApprovalOutcome::ALL.iter().map(|outcome| outcome.as_str()).collect();
    assert_eq!(words, vec!["allowed-once", "rejected", "cancelled", "unavailable"]);
}

/// 词汇表归一：闭集内原样返回，闭集外一律 fail-closed 收敛为 `unavailable`。
#[test]
fn approval_outcome_parse_normalizes_unknown() {
    for expected in ApprovalOutcome::ALL {
        assert_eq!(ApprovalOutcome::parse(expected.as_str()), expected);
    }
    for unknown in ["allowed", "denied", "approved", ""] {
        assert_eq!(ApprovalOutcome::parse(unknown), ApprovalOutcome::Unavailable, "text={unknown:?}");
    }
}

/// 回执收敛：一次性 / 本会话授权同为 `allowed-once`，拒绝为 `rejected`。
#[test]
fn approval_decision_maps_to_outcome() {
    assert_eq!(ApprovalDecision::AllowedOnce.outcome(), ApprovalOutcome::AllowedOnce);
    assert_eq!(ApprovalDecision::AllowedForSession.outcome(), ApprovalOutcome::AllowedOnce);
    assert_eq!(ApprovalDecision::Rejected.outcome(), ApprovalOutcome::Rejected);
    assert!(ApprovalOutcome::AllowedOnce.is_allowed());
    assert!(!ApprovalOutcome::Cancelled.is_allowed());
}

// ========== #27：委派审批预设种子（`source` 取 `delegation`） ==========

/// 折叠规则：**最后一条**生效（运行期切换会改写有效预设），无记录时为 None。
#[test]
fn preset_from_events_takes_last_record() {
    use crate::session::SessionEvent;

    let events = vec![
        SessionEvent::TurnStart { turn: 1 },
        SessionEvent::ApprovalPreset { preset: "auto".to_string(), source: None },
        SessionEvent::ApprovalPreset {
            preset: "danger-full-access".to_string(),
            source: Some(DELEGATION_SOURCE.to_string()),
        },
        SessionEvent::TurnEnd { turn: 1, reason: "completed".to_string() },
    ];
    assert_eq!(preset_from_events(&events), Some("danger-full-access".to_string()));
    assert_eq!(preset_from_events(&[]), None);
    // 空预设视为无效记录（不让坏行覆盖有效预设）。
    assert_eq!(
        preset_from_events(&[
            SessionEvent::ApprovalPreset { preset: "auto".to_string(), source: None },
            SessionEvent::ApprovalPreset { preset: String::new(), source: None },
        ]),
        Some("auto".to_string())
    );
}

// ========== #25：审批必须 turn 内 ==========

/// 轮次开合折叠：`TurnStart` 打开、`TurnEnd` 关闭；空日志视为未开轮。
#[test]
fn turn_open_tracks_last_turn_boundary() {
    use crate::session::SessionEvent;

    assert!(!turn_open(&[]));
    assert!(!turn_open(&[SessionEvent::SessionStarted {
        session_id: "s".to_string(),
        model: "m".to_string(),
    }]));
    assert!(turn_open(&[
        SessionEvent::TurnStart { turn: 1 },
        SessionEvent::StepStart { turn: 1, step: 1 },
    ]));
    assert!(!turn_open(&[
        SessionEvent::TurnStart { turn: 1 },
        SessionEvent::TurnEnd { turn: 1, reason: "completed".to_string() },
    ]));
    // 多轮：只看最后一条边界。
    assert!(turn_open(&[
        SessionEvent::TurnStart { turn: 1 },
        SessionEvent::TurnEnd { turn: 1, reason: "completed".to_string() },
        SessionEvent::TurnStart { turn: 2 },
    ]));
}

/// 会话中断撤回待回填审批：答复端被 drop → 等待端按 `cancelled` 结局落审计。
#[tokio::test]
async fn cancelled_outcome_when_request_aborted() {
    let session = Arc::new(SessionLog::in_memory());
    let (config, ctx) = gate_ctx(&session);
    let (gate, mut requests) = gate_pair(
        config,
        classifier(HashMap::from([("exec".to_string(), ToolRisk::Escalating)])),
    );
    let handle = tokio::spawn(gate.tool(ctx, ToolCall { id: "c1".to_string(), name: "exec".to_string(), input: json!({"command": "make"}) }, passthrough()));
    let request = requests.recv().await.unwrap_or_else(|| panic!("approval request expected"));
    drop(request); // 中断：撤回挂起审批
    let admission = handle.await.unwrap_or_else(|error| panic!("task panicked: {error}"));
    assert!(matches!(admission, ToolAdmission::Deny { .. }));
    assert_eq!(audited(&session), vec![("exec".to_string(), "cancelled".to_string(), "interrupted".to_string())]);
}
