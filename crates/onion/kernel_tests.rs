//! `kernel` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//! FakeLlm 脚本驱动，无网络：①纯文本单 step ②工具欠请求续跑 ③steer 落下一 step
//! ④pre-step reject 空轮 ⑤预取消不提交 user。

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::*;
use crate::listener::Listener;
use crate::llm::{LlmFailure, LlmFailureCode, LlmStream, LlmStreamItem};
use crate::runtime::Runtime;
use crate::system_prompt::{
    context_orders, section_orders, PromptContext, PromptSection, SectionText, SystemPrompt,
};
use crate::types::{ContentBlock, ToolDefinition, Usage};
use crate::waterfall::{BoxFuture, Next};

/// 测试专用回声工具（内核单测只需一个恒定返回文本的工具面）。
struct EchoTool;

impl crate::tools::ToolPort for EchoTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "echo".to_string(),
            description: "echo".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": { "text": { "type": "string" } },
                "required": ["text"]
            }),
        }
    }

    fn execute(
        &self,
        call: ToolCall,
        _cancel: crate::CancellationToken,
    ) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let text = call.input.get("text").and_then(serde_json::Value::as_str).unwrap_or("").to_string();
        Box::pin(async move { Ok(vec![ContentBlock::Text { text: format!("echo: {text}") }]) })
    }
}

/// 脚本化 FakeLlm：按序弹出落定结果（耗尽后报错）。
struct FakeLlm {
    script: Mutex<VecDeque<AssistantTurn>>,
}

impl FakeLlm {
    fn new(script: Vec<AssistantTurn>) -> Self {
        Self { script: Mutex::new(script.into_iter().collect()) }
    }
}

impl crate::llm::LlmPort for FakeLlm {
    fn stream(&self, _draft: &RequestDraft, _cancel: CancellationToken) -> BoxFuture<LlmStream> {
        let turn = self
            .script
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| AssistantTurn::error("fake", "script exhausted", false));
        Box::pin(async move {
            Box::pin(futures_util::stream::iter(vec![LlmStreamItem::Done(turn)])) as LlmStream
        })
    }
}

fn text_turn(text: &str) -> AssistantTurn {
    AssistantTurn {
        message: AgentMessage::assistant(vec![ContentBlock::Text { text: text.to_string() }]),
        stop_reason: StopReason::EndTurn,
        error: None,
        failure: None,
        cancelled: false,
        usage: Usage { input_tokens: 1, output_tokens: 1, cache_read_tokens: 0, cache_write_tokens: 0 },
        model: "fake".to_string(),
    }
}

fn echo_call() -> ToolCall {
    ToolCall { id: "call-1".to_string(), name: "echo".to_string(), input: serde_json::json!({"text": "hi"}) }
}

fn tool_turn() -> AssistantTurn {
    let call = echo_call();
    AssistantTurn {
        message: AgentMessage::assistant(vec![ContentBlock::ToolUse {
            id: call.id,
            name: call.name,
            input: call.input,
        }]),
        stop_reason: StopReason::ToolUse,
        error: None,
        failure: None,
        cancelled: false,
        usage: Usage::default(),
        model: "fake".to_string(),
    }
}

/// 结构化失败轮（#16）：error turn 附加失败分类，驱动内核链尾的重试/终止决策。
fn error_turn(code: LlmFailureCode, message: &str) -> AssistantTurn {
    AssistantTurn::error("fake", message, false).with_failure(LlmFailure {
        code,
        message: message.to_string(),
        retry_after_ms: None,
    })
}

fn test_config() -> Config {
    Config {
        model: "fake".to_string(),
        base_url: String::new(),
        api_key: String::new(),
        max_tokens: 100,
        temperature: None,
        max_retries: 1,
        tool_timeout: Duration::from_secs(5),
        max_parallel_tools: 10,
        include_identity: false,
        include_runtime_context: true,
        persona_prefix: String::new(),
        persona_suffix: String::new(),
        tool_order: None,
        context_window_tokens: 128_000,
        compact_threshold_ratio: 0.8,
        compact_retain_ratio: 0.16,
        compact_max_retries: 1,
        reserved_completion_tokens: 2048,
        compact_headroom_tokens: 65536,
        compact_model_policies: Vec::new(),
        workspace_enabled: false,
        workspace_max_bytes: 65_536,
        approval_preset: "workspace-write".to_string(),
        allow_parallel_in_progress: false,
        web_search_base_url: String::new(),
        web_search_model: String::new(),
        web_search_max_results: 8,
        web_search_max_queries: 4,
        web_fetch_max_output_chars: 200_000,
        session_query_max_results: 100,
        present_max_files: 8,
        terminal_enable_background: true,
        terminal_max_result_chars: 32_768,
        sandbox_executable: None,
        mcp_servers_config: None,
        workspace_deps_source: None,
        agent_preset: "coding".to_string(),
        agents_dir: None,
        subagent_model_selection: None,
        agent_team: None,
        a2a_token: None,
    }
}

/// EchoTool 注册表 + 空上下文（多数测试共用）。
fn registry() -> (ToolRegistry, Context) {
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(EchoTool));
    tools.state().set_agent(["echo".to_string()]);
    let context = Context { system: None, messages: Vec::new(), tools: tools.definitions() };
    (tools, context)
}

fn build_runtime(llm: Arc<dyn crate::llm::LlmPort>, listeners: Vec<Arc<dyn Listener>>) -> Arc<Runtime> {
    let (tools, context) = registry();
    let config = test_config();
    let prompts = SystemPrompt::new(&config);
    let mut runtime = Runtime::in_memory(config, prompts, context, llm, tools);
    for listener in listeners {
        runtime = runtime.with_listener(listener);
    }
    Arc::new(runtime)
}

fn tags(runtime: &Runtime) -> Vec<String> {
    runtime
        .session()
        .snapshot()
        .iter()
        .map(|event| match event {
            SessionEvent::SessionStarted { .. } => "session_started".to_string(),
            SessionEvent::TurnStart { .. } => "turn_start".to_string(),
            SessionEvent::SystemMessage { .. } => "system_message".to_string(),
            SessionEvent::UserMessage { .. } => "user_message".to_string(),
            SessionEvent::AssistantMessage { .. } => "assistant_message".to_string(),
            SessionEvent::AssistantAttempt { .. } => "assistant_attempt".to_string(),
            SessionEvent::ToolCall { .. } => "tool_call".to_string(),
            SessionEvent::ToolResult { .. } => "tool_result".to_string(),
            SessionEvent::StepStart { .. } => "step_start".to_string(),
            SessionEvent::StepEnd { .. } => "step_end".to_string(),
            SessionEvent::TurnEnd { .. } => "turn_end".to_string(),
            SessionEvent::SessionEnd { .. } => "session_end".to_string(),
            SessionEvent::CompactionStart { .. } => "compaction_start".to_string(),
            SessionEvent::CompactionSummary { .. } => "compaction_summary".to_string(),
            SessionEvent::CompactionEnd { .. } => "compaction_end".to_string(),
            SessionEvent::CompactionPruned { .. } => "compaction_pruned".to_string(),
            SessionEvent::HookInvoked { .. } => "hook_invoked".to_string(),
            SessionEvent::HookResult { .. } => "hook_result".to_string(),
            SessionEvent::WorkspaceInstructions { .. } => "workspace_instructions".to_string(),
            SessionEvent::WorkspaceChanged { .. } => "workspace_changed".to_string(),
            SessionEvent::GoalChange { .. } => "goal_change".to_string(),
            SessionEvent::PlanMode { .. } => "plan_mode".to_string(),
            SessionEvent::PlanWanted { .. } => "plan_wanted".to_string(),
            SessionEvent::PlanRunDone { .. } => "plan_run_done".to_string(),
            SessionEvent::ApprovalAudited { .. } => "approval_audited".to_string(),
            SessionEvent::TodoWrite { .. } => "todo_write".to_string(),
            SessionEvent::Presented { .. } => "presented".to_string(),
            SessionEvent::WorkingDir { .. } => "working_dir".to_string(),
            SessionEvent::WorktreeChange { .. } => "worktree_change".to_string(),
            SessionEvent::TeamMemberChange { .. } => "team_member_change".to_string(),
            SessionEvent::TeamTaskChange { .. } => "team_task_change".to_string(),
            SessionEvent::ModelSelection { .. } => "model_selection".to_string(),
            SessionEvent::SessionTitle { .. } => "session_title".to_string(),
            SessionEvent::CommandRun { .. } => "command_run".to_string(),
            SessionEvent::CommandDone { .. } => "command_done".to_string(),
            SessionEvent::FeedbackRecord { .. } => "feedback_record".to_string(),
            SessionEvent::ApprovalPreset { .. } => "approval_preset".to_string(),
            SessionEvent::SubagentModelSelectionPolicy { .. } => {
                "subagent_model_selection_policy".to_string()
            }
            SessionEvent::SubagentDescriptor { .. } => "subagent_descriptor".to_string(),
            SessionEvent::ForkSeed { .. } => "fork_seed".to_string(),
        })
        .collect()
}

#[tokio::test]
async fn plain_text_single_step_completes() {
    let runtime = build_runtime(Arc::new(FakeLlm::new(vec![text_turn("hello back")])), vec![]);
    let report = runtime.run(vec![AgentMessage::user("hi")]).await.unwrap().expect("应开轮");
    assert_eq!(report.turn, 1);
    assert_eq!(report.steps, 1);
    assert_eq!(report.reason, TurnEndReason::Completed);
    assert_eq!(
        tags(&runtime),
        vec![
            "session_started",
            "turn_start",
            "step_start",
            "user_message",
            "assistant_message",
            "step_end",
            "turn_end"
        ]
    );
}

#[tokio::test]
async fn tool_call_owes_another_step() {
    let runtime = build_runtime(
        Arc::new(FakeLlm::new(vec![tool_turn(), text_turn("tool done")])),
        vec![],
    );
    let report = runtime.run(vec![AgentMessage::user("use echo")]).await.unwrap().expect("应开轮");
    assert_eq!(report.steps, 2, "工具欠请求自动续跑一步");
    assert_eq!(report.reason, TurnEndReason::Completed);
    let events = runtime.session().snapshot();
    let tool_calls = events.iter().filter(|e| matches!(e, SessionEvent::ToolCall { .. })).count();
    let tool_results = events.iter().filter(|e| matches!(e, SessionEvent::ToolResult { .. })).count();
    assert_eq!(tool_calls, 1);
    assert_eq!(tool_results, 1);
    // tool_use / tool_result 配对：投影中 assistant 消息后紧跟 Tool 角色消息
    let context = Context::project(&events, vec![]);
    assert_eq!(context.messages.len(), 4, "user + assistant(tool_use) + tool_result + assistant(text)");
    assert_eq!(context.messages[2].role, crate::types::Role::Tool);
}

/// listener 在 ToolResult 后 steer：注入消息落在同一 turn 的下一 step。
struct ToolSteerer;

impl Listener for ToolSteerer {
    fn on_event(&self, ctx: &ListenerCtx, event: &LiveEvent) {
        if matches!(event, LiveEvent::Durable(SessionEvent::ToolResult { .. })) {
            let _ = ctx.inbox.steer(AgentMessage::user("steered hint"));
        }
    }
}

#[tokio::test]
async fn steered_message_lands_in_next_step() {
    let runtime = build_runtime(
        Arc::new(FakeLlm::new(vec![tool_turn(), text_turn("ok")])),
        vec![Arc::new(ToolSteerer)],
    );
    let report = runtime.run(vec![AgentMessage::user("use echo")]).await.unwrap().expect("应开轮");
    assert_eq!(report.steps, 2);
    let events = runtime.session().snapshot();
    let user_messages: Vec<&AgentMessage> = events
        .iter()
        .filter_map(|e| match e {
            SessionEvent::UserMessage { message, .. } => Some(message),
            _ => None,
        })
        .collect();
    assert_eq!(user_messages.len(), 2);
    assert_eq!(user_messages[1].text_content(), "steered hint");
}

/// 空闲唤醒开轮：空输入 + steer 置位的唤醒直接开轮，steer 消息成为首轮批次。
#[tokio::test]
async fn idle_wakeup_opens_turn_and_consumes_steered_message() {
    let runtime = build_runtime(Arc::new(FakeLlm::new(vec![text_turn("woken")])), vec![]);
    runtime.inbox().steer(AgentMessage::user("wake up")).unwrap();
    let report = runtime.run(Vec::new()).await.unwrap().expect("唤醒应开轮");
    assert_eq!(report.steps, 1);
    let events = runtime.session().snapshot();
    let user_messages: Vec<&AgentMessage> = events
        .iter()
        .filter_map(|e| match e {
            SessionEvent::UserMessage { message, .. } => Some(message),
            _ => None,
        })
        .collect();
    assert_eq!(user_messages.len(), 1);
    assert_eq!(user_messages[0].text_content(), "wake up");
}

/// inject 不唤醒：空输入 + 仅 inject 消息 → 不开轮，消息保留在 next-step 分区。
#[tokio::test]
async fn injected_message_alone_does_not_open_turn() {
    let runtime = build_runtime(Arc::new(FakeLlm::new(vec![text_turn("unused")])), vec![]);
    runtime.inbox().inject(AgentMessage::user("later")).unwrap();
    let report = runtime.run(Vec::new()).await.unwrap();
    assert!(report.is_none(), "inject 不应触发开轮");
    assert!(runtime.inbox().has_next_step(), "inject 消息保留待下一个 step");
}

/// 同批 followup 合并单 turn（#15，合并呈现）：开轮批量领取
/// next-turn 全量——两条 followup 同批投递不拆两轮，一并进入同一 turn
/// 的首个 step。
#[tokio::test]
async fn same_batch_followups_merge_into_single_turn() {
    let runtime = build_runtime(Arc::new(FakeLlm::new(vec![text_turn("merged")])), vec![]);
    runtime.inbox().followup(AgentMessage::user("followup a")).unwrap();
    runtime.inbox().followup(AgentMessage::user("followup b")).unwrap();
    let report = runtime.run(Vec::new()).await.unwrap().expect("followup 唤醒应开轮");
    assert_eq!(report.turn, 1);
    assert_eq!(report.steps, 1);
    assert_eq!(report.reason, TurnEndReason::Completed);
    // 单 turn：仅一次 turn_start。
    let event_tags = tags(&runtime);
    assert_eq!(event_tags.iter().filter(|tag| *tag == "turn_start").count(), 1);
    // 两条消息同一批提交（同 turn 内各一条 UserMessage）。
    let events = runtime.session().snapshot();
    let user_messages: Vec<&AgentMessage> = events
        .iter()
        .filter_map(|e| match e {
            SessionEvent::UserMessage { message, .. } => Some(message),
            _ => None,
        })
        .collect();
    assert_eq!(user_messages.len(), 2);
    assert_eq!(user_messages[0].text_content(), "followup a");
    assert_eq!(user_messages[1].text_content(), "followup b");
    // claim 即移除：领取后 next-turn 分区清空。
    assert!(!runtime.inbox().has_next_turn());
}

/// 可重试分类（RateLimit）走退避重试（#16，adapter-failure 分类 + retry-policy）：
/// 失败落 AssistantAttempt → 链尾按退避策略 Retry → 重装配后下一脚本项成功。
#[tokio::test]
async fn rate_limit_error_retries_then_succeeds() {
    let runtime = build_runtime(
        Arc::new(FakeLlm::new(vec![
            error_turn(LlmFailureCode::RateLimit, "429 too many requests"),
            text_turn("recovered"),
        ])),
        vec![],
    );
    let report = runtime.run(vec![AgentMessage::user("hi")]).await.unwrap().expect("应开轮");
    assert_eq!(report.steps, 1, "重试发生在同一 step 内");
    assert_eq!(report.reason, TurnEndReason::Completed);
    let event_tags = tags(&runtime);
    assert_eq!(
        event_tags.iter().filter(|tag| *tag == "assistant_attempt").count(),
        1,
        "失败尝试落 AssistantAttempt"
    );
    assert_eq!(
        event_tags.iter().filter(|tag| *tag == "assistant_message").count(),
        1,
        "重试成功后仅一条 assistant 消息"
    );
}

/// 不可重试分类（Auth）不进退避：轮以 Rejected 终止，仅一次失败尝试。
#[tokio::test]
async fn auth_error_aborts_without_retry() {
    let runtime = build_runtime(
        Arc::new(FakeLlm::new(vec![error_turn(LlmFailureCode::Auth, "401 unauthorized")])),
        vec![],
    );
    let report = runtime.run(vec![AgentMessage::user("hi")]).await.unwrap().expect("应开轮");
    assert!(matches!(report.reason, TurnEndReason::Rejected(_)));
    let event_tags = tags(&runtime);
    assert_eq!(
        event_tags.iter().filter(|tag| *tag == "assistant_attempt").count(),
        1,
        "仅一次失败尝试（无重试）"
    );
    assert!(
        !event_tags.contains(&"assistant_message".to_string()),
        "不可重试错误不产生 assistant 消息"
    );
}

/// pre-step reject：含 BLOCK 的消息被拦截，空轮关闭（无 StepStart）。
struct BlockGate;

impl Listener for BlockGate {
    fn pre_step(
        &self,
        _ctx: ListenerCtx,
        _input: ClaimedInput,
        _next: Next<ClaimedInput, PreStepDecision>,
    ) -> BoxFuture<PreStepDecision> {
        Box::pin(async move {
            PreStepDecision::Reject { reason: "blocked by gate".to_string() }
        })
    }
}

#[tokio::test]
async fn pre_step_reject_closes_empty_turn() {
    let runtime = build_runtime(Arc::new(FakeLlm::new(vec![text_turn("unused")])), vec![Arc::new(BlockGate)]);
    let report = runtime.run(vec![AgentMessage::user("BLOCK this")]).await.unwrap().expect("应开轮（空轮）");
    assert_eq!(report.steps, 0);
    assert!(matches!(report.reason, TurnEndReason::Rejected(_)));
    let events = runtime.session().snapshot();
    assert!(!events.iter().any(|e| matches!(e, SessionEvent::StepStart { .. })), "reject 不发 StepStart");
    assert!(!events.iter().any(|e| matches!(e, SessionEvent::UserMessage { .. })), "reject 不提交 user");
}

#[tokio::test]
async fn pre_cancelled_token_is_replaced_by_run() {
    // 取消按活动而非按会话：`run` 每轮开轮前换新令牌，
    // 上一轮遗留的 cancel 不会毒化后续轮次（interrupt 后会话可继续使用）。
    let runtime = build_runtime(Arc::new(FakeLlm::new(vec![text_turn("ok")])), vec![]);
    runtime.cancel_handle().cancel();
    let report = runtime.run(vec![AgentMessage::user("hi")]).await.unwrap().expect("应开轮");
    assert_ne!(report.reason, TurnEndReason::Cancelled, "旧令牌的 cancel 不应影响新一轮");
    assert_eq!(report.steps, 1);
    let events = runtime.session().snapshot();
    assert!(
        events.iter().any(|e| matches!(e, SessionEvent::UserMessage { .. })),
        "新一轮正常提交 user"
    );
}

#[tokio::test]
async fn no_input_returns_none() {
    let runtime = build_runtime(Arc::new(FakeLlm::new(vec![])), vec![]);
    let result = runtime.run(Vec::new()).await.unwrap();
    assert!(result.is_none(), "无输入不开轮");
}

/// 开轮装配：mission 段（含 {{model}} 插值）+ clock 上下文 →
/// 首步恰提交一条 system_message 与一条运行时快照 user_message（批次第 0 条）。
#[tokio::test]
async fn system_prompt_and_context_snapshot_commit_at_first_step() {
    let (tools, context) = registry();
    let config = test_config();
    let mut prompts = SystemPrompt::new(&config);
    prompts
        .section(PromptSection {
            name: "onion:mission".to_string(),
            order: section_orders::TOOL_GUIDE,
            text: SectionText::Static("mission for {{model}}".to_string()),
            interpolate: true,
            complete: false,
        })
        .unwrap();
    prompts.variable("model", Arc::new(|| Some("fake".to_string()))).unwrap();
    prompts
        .context(PromptContext {
            name: "runtime_clock".to_string(),
            order: context_orders::RUNTIME_CLOCK,
            text: SectionText::Static("epoch=1".to_string()),
            interpolate: false,
            required: true,
        })
        .unwrap();
    let runtime = Runtime::in_memory(
        config,
        prompts,
        context,
        Arc::new(FakeLlm::new(vec![text_turn("hi back")])),
        tools,
    );
    let report = runtime.run(vec![AgentMessage::user("hi")]).await.unwrap().expect("应开轮");
    assert_eq!(report.reason, TurnEndReason::Completed);
    let events = runtime.session().snapshot();
    let systems: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            SessionEvent::SystemMessage { message, .. } => Some(message.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(systems, vec!["mission for fake"], "首步恰提交一条渲染后的 system_message");
    let user_messages: Vec<&AgentMessage> = events
        .iter()
        .filter_map(|e| match e {
            SessionEvent::UserMessage { message, .. } => Some(message),
            _ => None,
        })
        .collect();
    assert_eq!(user_messages.len(), 2, "运行时快照 + 原始输入共两条 user");
    assert!(
        user_messages[0].text_content().starts_with("Current runtime context."),
        "首步批次第 0 条为运行时快照"
    );
    assert!(user_messages[0].text_content().contains("epoch=1"));
    assert_eq!(user_messages[1].text_content(), "hi");
}

/// prune 接线（E1-in-route）：超长工具结果在请求装配期被 head/marker/tail
/// 裁剪，发射 log-only `CompactionPruned` durable 事件（投影不消费）。
#[tokio::test]
async fn oversized_tool_result_emits_compaction_pruned() {
    // echo 输入超 PRUNE_THRESHOLD_CHARS → 工具结果文本超阈被裁。
    let oversized = "x".repeat(9_000);
    let call = ToolCall {
        id: "call-1".to_string(),
        name: "echo".to_string(),
        input: serde_json::json!({ "text": oversized }),
    };
    let tool_step = AssistantTurn {
        message: AgentMessage::assistant(vec![ContentBlock::ToolUse {
            id: call.id,
            name: call.name,
            input: call.input,
        }]),
        stop_reason: StopReason::ToolUse,
        error: None,
        failure: None,
        cancelled: false,
        usage: Usage::default(),
        model: "fake".to_string(),
    };
    let runtime = build_runtime(
        Arc::new(FakeLlm::new(vec![tool_step, text_turn("done")])),
        vec![],
    );
    let report = runtime
        .run(vec![AgentMessage::user("echo big")])
        .await
        .unwrap()
        .expect("应开轮");
    assert_eq!(report.steps, 2);
    let events = runtime.session().snapshot();
    // log-only 事件已发射：1 条被裁（pruned==1），saved_tokens > 0。
    let pruned: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            SessionEvent::CompactionPruned { pruned, saved_tokens } => Some((*pruned, *saved_tokens)),
            _ => None,
        })
        .collect();
    assert_eq!(pruned.len(), 1, "恰好发射一次 CompactionPruned");
    assert_eq!(pruned[0].0, 1, "单条超长工具结果 → pruned==1");
    assert!(pruned[0].1 > 0, "saved_tokens 为正");
    // 投影视图不消费该事件：tool_result 消息仍是原样全量（投影全史不动）。
    let context = Context::project(&events, vec![]);
    assert_eq!(context.messages.len(), 4);
    let tool_msg = &context.messages[2];
    assert_eq!(tool_msg.role, crate::types::Role::Tool);
    let full_len = tool_msg
        .blocks
        .iter()
        .filter_map(|b| match b {
            ContentBlock::ToolResult { content, .. } => Some(content.len()),
            _ => None,
        })
        .sum::<usize>();
    assert!(full_len > 0, "投影保留原样全量工具结果（视图级变换在装配期重放）");
}

/// 引用未注册变量的段：组装失败 → 空轮拒绝（steps == 0，无 StepStart / UserMessage）。
#[tokio::test]
async fn prompt_assembly_failure_rejects_turn() {
    let (tools, context) = registry();
    let config = test_config();
    let mut prompts = SystemPrompt::new(&config);
    prompts
        .section(PromptSection {
            name: "onion:broken".to_string(),
            order: section_orders::TOOL_GUIDE,
            text: SectionText::Static("missing {{nope}}".to_string()),
            interpolate: true,
            complete: false,
        })
        .unwrap();
    let runtime = Runtime::in_memory(
        config,
        prompts,
        context,
        Arc::new(FakeLlm::new(vec![text_turn("unused")])),
        tools,
    );
    let report = runtime
        .run(vec![AgentMessage::user("hi")])
        .await
        .unwrap()
        .expect("应开轮（空轮）");
    assert_eq!(report.steps, 0);
    assert!(matches!(report.reason, TurnEndReason::Rejected(_)));
    let events = runtime.session().snapshot();
    assert!(
        !events.iter().any(|e| matches!(e, SessionEvent::StepStart { .. })),
        "组装失败不发 StepStart"
    );
    assert!(
        !events.iter().any(|e| matches!(e, SessionEvent::UserMessage { .. })),
        "组装失败不提交输入"
    );
}
