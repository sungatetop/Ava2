//! `listener` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//!
//! 覆盖：压力触发 / 阈值内不触发 / manual 强制 / manual 无区域降级 /
//! shadowed 低于下限跳过事务 / 溢出恢复与限流 / 溢出失败 Abort /
//! 事务失败降级 / 压缩后投影收缩 / 空闲 compact_now 直驱 /
//! 空闲 compact_now 无区域降级。

use std::sync::Arc;

use tokio::sync::mpsc;

use super::*;
use crate::inbox::Inbox;
use crate::llm::{AssistantTurn, LlmFailure, LlmFailureCode, LlmStream, LlmStreamItem};
use crate::session::SessionLog;
use crate::types::{AgentMessage, StopReason};

// ---------- 测试基建 ----------

/// 确定性 fake LLM：成功返回固定摘要 / 失败返回固定错误。
enum FakeMode {
    Ok(String),
    Err(String),
}

struct FakeLlm {
    mode: FakeMode,
}

impl LlmPort for FakeLlm {
    fn stream(
        &self,
        _draft: &crate::context::RequestDraft,
        _cancel: CancellationToken,
    ) -> crate::waterfall::BoxFuture<LlmStream> {
        let mut turn = AssistantTurn {
            message: AgentMessage::assistant(vec![crate::types::ContentBlock::Text {
                text: "checkpoint".to_string(),
            }]),
            stop_reason: StopReason::EndTurn,
            error: None,
            failure: None,
            cancelled: false,
            usage: crate::types::Usage::default(),
            model: "fake-model".to_string(),
        };
        if let FakeMode::Err(message) = &self.mode {
            turn.error = Some(message.clone());
        }
        Box::pin(async move {
            Box::pin(futures_util::stream::iter(vec![LlmStreamItem::Done(turn)])) as LlmStream
        })
    }
}

fn listener(mode: FakeMode) -> (CompactionListener, mpsc::UnboundedSender<()>) {
    let spec = CompactSpec {
        context_window_tokens: 4_000,
        threshold_tokens: 800,
        retain_tokens: 200,
        compact_max_retries: 1,
        max_overflow_retries: 1,
    };
    CompactionListener::new(
        Arc::new(FakeLlm { mode }),
        spec,
        SummaryConfig { model: "fake-model".to_string(), max_tokens: 256, temperature: None },
        Vec::new(),
    )
}

fn ctx() -> ListenerCtx {
    let bus = crate::session::EventBus::new();
    ListenerCtx {
        inbox: Arc::new(Inbox::new(bus)),
        session: Arc::new(SessionLog::in_memory()),
        state: String::new(),
    }
}

/// 单条约 1_104 tok 的 user 消息（"x" × 4400 → chars/4 ≈ 1100 + 4 结构开销）。
/// 取 2 条 shadowed = 2_208 tok ≥ MIN_SHADOWED_TOKENS（2_048），越过事务
/// 发起下限（下限保护见 `tiny_shadowed_region_skips_transaction`）。
fn big_message(tag: &str) -> AgentMessage {
    AgentMessage::user(format!("{tag}: {}", "x".repeat(4_400)))
}

fn seed(session: &SessionLog, count: usize) {
    for index in 0..count {
        session.append(SessionEvent::UserMessage {
            turn: 1,
            step: 1,
            message: big_message(&format!("m{index}")),
        });
    }
}

fn input(turn: u32, step: u32) -> ClaimedInput {
    ClaimedInput {
        turn,
        step,
        source: crate::listener::ClaimSource::NextStep,
        messages: vec![],
        starts_request_series: false,
    }
}

fn passthrough_next() -> Next<ClaimedInput, PreStepDecision> {
    Box::new(|input| {
        Box::pin(async move {
            PreStepDecision::Enter {
                messages: input.messages,
                starts_request_series: input.starts_request_series,
            }
        })
    })
}

/// terminal 恒 Retry：Abort 断言依赖 listener 短路（若误调 next 会得到 Retry）。
fn retry_next() -> Next<RequestError, RetryDecision> {
    Box::new(|_| Box::pin(async { RetryDecision::Retry { delay_ms: 0 } }))
}

fn error_message(message: &str) -> RequestError {
    RequestError { message: message.to_string(), cancelled: false, failure: None }
}

/// 结构化溢出错误（对齐内核接线：溢出恢复按 failure.code 判定，不再字符串匹配）。
fn overflow_error(message: &str) -> RequestError {
    RequestError {
        message: message.to_string(),
        cancelled: false,
        failure: Some(LlmFailure {
            code: LlmFailureCode::ContextWindowExceeded,
            message: message.to_string(),
            retry_after_ms: None,
        }),
    }
}

/// compaction bracket 事件序（start 携带 trigger；end 携带 error 调试形态）。
fn compaction_kinds(session: &SessionLog) -> Vec<String> {
    session
        .snapshot()
        .iter()
        .filter_map(|event| match event {
            SessionEvent::CompactionStart { trigger, .. } => Some(format!("start:{trigger}")),
            SessionEvent::CompactionSummary { .. } => Some("summary".to_string()),
            SessionEvent::CompactionEnd { error, .. } => Some(format!("end:{error:?}")),
            _ => None,
        })
        .collect()
}

// ---------- 用例 ----------

#[tokio::test]
async fn pressure_trigger_runs_transaction() {
    let (listener, _tx) = listener(FakeMode::Ok("checkpoint body".to_string()));
    let ctx = ctx();
    seed(&ctx.session, 3);

    let decision = listener.pre_step(ctx.clone(), input(1, 2), passthrough_next()).await;
    assert!(matches!(decision, PreStepDecision::Enter { .. }));
    assert_eq!(
        compaction_kinds(&ctx.session),
        vec!["start:pressure".to_string(), "summary".to_string(), "end:None".to_string()]
    );
}

#[tokio::test]
async fn below_threshold_no_transaction() {
    let (listener, _tx) = listener(FakeMode::Ok("checkpoint body".to_string()));
    let ctx = ctx();
    // 单条小消息（~5 tok），远低于阈值 800。
    ctx.session.append(SessionEvent::UserMessage {
        turn: 1,
        step: 1,
        message: AgentMessage::user("tiny"),
    });
    let decision = listener.pre_step(ctx.clone(), input(1, 1), passthrough_next()).await;
    assert!(matches!(decision, PreStepDecision::Enter { .. }));
    assert!(compaction_kinds(&ctx.session).is_empty());
}

#[tokio::test]
async fn manual_signal_forces_transaction() {
    let (listener, tx) = listener(FakeMode::Ok("checkpoint body".to_string()));
    let ctx = ctx();
    seed(&ctx.session, 3);
    tx.send(()).unwrap();

    let decision = listener.pre_step(ctx.clone(), input(1, 2), passthrough_next()).await;
    assert!(matches!(decision, PreStepDecision::Enter { .. }));
    assert_eq!(
        compaction_kinds(&ctx.session),
        vec!["start:manual".to_string(), "summary".to_string(), "end:None".to_string()]
    );
}

#[tokio::test]
async fn manual_without_compactable_region_degrades() {
    let (listener, tx) = listener(FakeMode::Ok("checkpoint body".to_string()));
    let ctx = ctx();
    // 单条小消息全部落在 retain 区 → select_compactable_range = None → 无 bracket。
    ctx.session.append(SessionEvent::UserMessage {
        turn: 1,
        step: 1,
        message: AgentMessage::user("tiny"),
    });
    tx.send(()).unwrap();

    let decision = listener.pre_step(ctx.clone(), input(1, 1), passthrough_next()).await;
    assert!(matches!(decision, PreStepDecision::Enter { .. }));
    assert!(compaction_kinds(&ctx.session).is_empty());
}

#[tokio::test]
async fn tiny_shadowed_region_skips_transaction() {
    // E1.5 卸载图片后的回归：shadowed region 估算 < MIN_SHADOWED_TOKENS 时
    // 不发起摘要事务——框架摘要自身数百 token，注定 no-shrink（实证：framed
    // summary 513 tok vs shadowed 80 tok）。事务未开、无 bracket，降级继续。
    let (listener, tx) = listener(FakeMode::Ok("checkpoint body".to_string()));
    let ctx = ctx();
    // 2 条大消息：select 出 {start:0, count:1}（尾部 1 条留 retain 区），
    // shadowed ≈ 1_104 tok < 2_048 → 下限保护跳过（若无保护则会发起事务）。
    seed(&ctx.session, 2);
    tx.send(()).unwrap();

    let decision = listener.pre_step(ctx.clone(), input(1, 2), passthrough_next()).await;
    assert!(matches!(decision, PreStepDecision::Enter { .. }));
    assert!(compaction_kinds(&ctx.session).is_empty());
}

#[tokio::test]
async fn request_error_overflow_recovers_and_retries() {
    let (listener, _tx) = listener(FakeMode::Ok("checkpoint body".to_string()));
    let ctx = ctx();
    seed(&ctx.session, 3);
    // RequestError 不携带 turn/step：经 on_event StepStart 快照跟踪。
    listener.on_event(
        &ctx,
        &LiveEvent::Durable(SessionEvent::StepStart { turn: 2, step: 1 }),
    );

    let decision =
        listener.request_error(ctx.clone(), overflow_error("maximum context length exceeded"), retry_next()).await;
    assert!(matches!(decision, RetryDecision::Retry { .. }));
    assert_eq!(
        compaction_kinds(&ctx.session),
        vec!["start:context_overflow".to_string(), "summary".to_string(), "end:None".to_string()]
    );
}

#[tokio::test]
async fn request_error_non_overflow_passthrough() {
    let (listener, _tx) = listener(FakeMode::Ok("checkpoint body".to_string()));
    let ctx = ctx();
    seed(&ctx.session, 3);
    listener.on_event(
        &ctx,
        &LiveEvent::Durable(SessionEvent::StepStart { turn: 2, step: 1 }),
    );

    let decision =
        listener.request_error(ctx.clone(), error_message("HTTP 500 internal"), retry_next()).await;
    assert!(matches!(decision, RetryDecision::Retry { .. }));
    assert!(compaction_kinds(&ctx.session).is_empty());
}

#[tokio::test]
async fn request_error_limited_once_per_step() {
    let (listener, _tx) = listener(FakeMode::Ok("checkpoint body".to_string()));
    let ctx = ctx();
    // 4 条大消息：首次压缩 shadowed 2 条后仍有可压缩区域（排除 Nothing 干扰）。
    seed(&ctx.session, 4);
    listener.on_event(
        &ctx,
        &LiveEvent::Durable(SessionEvent::StepStart { turn: 2, step: 1 }),
    );

    let first =
        listener.request_error(ctx.clone(), overflow_error("context length exceeded"), retry_next()).await;
    let second =
        listener.request_error(ctx.clone(), overflow_error("context length exceeded"), retry_next()).await;
    assert!(matches!(first, RetryDecision::Retry { .. }));
    assert!(matches!(second, RetryDecision::Retry { .. }));
    // 同 step 第二次溢出被限流：仅一次事务（1 start + 1 summary + 1 end）。
    assert_eq!(compaction_kinds(&ctx.session).len(), 3);
}

#[tokio::test]
async fn request_error_abort_when_transaction_fails() {
    let (listener, _tx) = listener(FakeMode::Err("summarizer unavailable".to_string()));
    let ctx = ctx();
    seed(&ctx.session, 3);
    listener.on_event(
        &ctx,
        &LiveEvent::Durable(SessionEvent::StepStart { turn: 2, step: 1 }),
    );

    // retry_next 恒 Retry：得到 Abort 说明 listener 短路（未调 next）。
    let decision =
        listener.request_error(ctx.clone(), overflow_error("context length exceeded"), retry_next()).await;
    assert_eq!(decision, RetryDecision::Abort);
    assert_eq!(
        compaction_kinds(&ctx.session),
        vec![
            "start:context_overflow".to_string(),
            "end:Some(\"summarizer unavailable\")".to_string()
        ]
    );
}

#[tokio::test]
async fn transaction_failure_degrades_to_enter() {
    let (listener, _tx) = listener(FakeMode::Err("summarizer unavailable".to_string()));
    let ctx = ctx();
    seed(&ctx.session, 3);

    let decision = listener.pre_step(ctx.clone(), input(1, 2), passthrough_next()).await;
    assert!(matches!(decision, PreStepDecision::Enter { .. }));
    assert_eq!(
        compaction_kinds(&ctx.session),
        vec![
            "start:pressure".to_string(),
            "end:Some(\"summarizer unavailable\")".to_string()
        ]
    );
}

#[tokio::test]
async fn projection_shrinks_after_compaction() {
    let (listener, _tx) = listener(FakeMode::Ok("checkpoint body".to_string()));
    let ctx = ctx();
    seed(&ctx.session, 3);

    let decision = listener.pre_step(ctx.clone(), input(1, 2), passthrough_next()).await;
    assert!(matches!(decision, PreStepDecision::Enter { .. }));
    // 投影重建：前 2 条被替换为 1 条 <compacted-summary> 承载，尾部 m2 保留。
    let context = Context::project(&ctx.session.snapshot(), Vec::new());
    assert_eq!(context.messages.len(), 2);
    assert!(context.messages[0].text_content().contains(super::super::summarizer::SUMMARY_OPEN_TAG));
    assert!(context.messages[1].text_content().contains("m2"));
}

#[tokio::test]
async fn checkpoint_message_carries_source_and_is_not_projected_twice() {
    let (listener, _tx) = listener(FakeMode::Ok("checkpoint body".to_string()));
    let ctx = ctx();
    seed(&ctx.session, 3);

    listener.pre_step(ctx.clone(), input(1, 2), passthrough_next()).await;

    // durable 层：CompactionSummary 之后紧跟一条带 compact_checkpoint source 的
    // user 消息（summary + checkpoint 双写）。
    let events = ctx.session.snapshot();
    let summary_at = events
        .iter()
        .position(|event| matches!(event, SessionEvent::CompactionSummary { .. }))
        .expect("compaction summary");
    let checkpoint = events[summary_at + 1..]
        .iter()
        .find_map(|event| match event {
            SessionEvent::UserMessage { message, .. } => Some(message),
            _ => None,
        })
        .expect("checkpoint user message");
    assert!(matches!(
        checkpoint.source,
        Some(crate::types::MessageSource::CompactCheckpoint { .. })
    ));
    assert!(checkpoint.text_content().contains("checkpoint"));

    // 投影层：替换只由 CompactionSummary 完成，检查点消息不重复插入。
    let context = Context::project(&events, Vec::new());
    assert_eq!(context.messages.len(), 2);
}

#[tokio::test]
async fn summarizer_retries_before_failing() {
    // compact_max_retries = 1 → 首次 + 1 次重试 = 2 次 LLM 调用后 Summarizer 失败。
    struct CountingLlm {
        calls: std::sync::atomic::AtomicU64,
    }
    impl LlmPort for CountingLlm {
        fn stream(
            &self,
            _draft: &crate::context::RequestDraft,
            _cancel: CancellationToken,
        ) -> crate::waterfall::BoxFuture<LlmStream> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let turn = AssistantTurn {
                message: AgentMessage::assistant(vec![]),
                stop_reason: StopReason::EndTurn,
                error: Some("provider down".to_string()),
                failure: None,
                cancelled: false,
                usage: crate::types::Usage::default(),
                model: "fake-model".to_string(),
            };
            Box::pin(async move {
                Box::pin(futures_util::stream::iter(vec![LlmStreamItem::Done(turn)])) as LlmStream
            })
        }
    }
    let spec = CompactSpec {
        context_window_tokens: 4_000,
        threshold_tokens: 800,
        retain_tokens: 200,
        compact_max_retries: 1,
        max_overflow_retries: 1,
    };
    let llm = Arc::new(CountingLlm { calls: std::sync::atomic::AtomicU64::new(0) });
    let (listener, _tx) = CompactionListener::new(
        llm.clone(),
        spec,
        SummaryConfig { model: "fake-model".to_string(), max_tokens: 256, temperature: None },
        Vec::new(),
    );
    let ctx = ctx();
    seed(&ctx.session, 3);

    let decision = listener.pre_step(ctx.clone(), input(1, 2), passthrough_next()).await;
    assert!(matches!(decision, PreStepDecision::Enter { .. }));
    assert_eq!(llm.calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    assert_eq!(
        compaction_kinds(&ctx.session),
        vec!["start:pressure".to_string(), "end:Some(\"provider down\")".to_string()]
    );
}

#[tokio::test]
async fn idle_compact_now_runs_transaction() {
    let (listener, _tx) = listener(FakeMode::Ok("checkpoint body".to_string()));
    let ctx = ctx();
    seed(&ctx.session, 3);
    // 空闲会话已有 t2 的 turn 记录 → compact_now 推导下一轮 t3。
    ctx.session.append(SessionEvent::TurnStart { turn: 2 });

    listener.compact_now(&ctx.session).await;
    assert_eq!(
        compaction_kinds(&ctx.session),
        vec!["start:manual".to_string(), "summary".to_string(), "end:None".to_string()]
    );
    // bracket 记账在 t3（最近 TurnStart + 1）。
    let start_turn = ctx
        .session
        .snapshot()
        .iter()
        .find_map(|event| match event {
            SessionEvent::CompactionStart { turn, .. } => Some(*turn),
            _ => None,
        })
        .unwrap();
    assert_eq!(start_turn, 3);
}

#[tokio::test]
async fn compact_now_result_summary_seq_points_at_summary_event() {
    // 回执指针必须精确落在摘要事件上，
    // 而非事务首条事件（CompactionStart）。
    let (listener, _tx) = listener(FakeMode::Ok("checkpoint body".to_string()));
    let ctx = ctx();
    seed(&ctx.session, 3);

    let result = listener.compact_now_result(&ctx.session).await.unwrap();
    let snapshot = ctx.session.snapshot();
    let summary_index = snapshot
        .iter()
        .position(|event| matches!(event, SessionEvent::CompactionSummary { .. }))
        .unwrap();
    assert_eq!(result.summary_seq, summary_index as u64);
    let SessionEvent::CompactionSummary { compaction_id, .. } = &snapshot[summary_index] else {
        panic!("下标应指向 CompactionSummary");
    };
    assert_eq!(compaction_id, &result.id.0);
}

#[tokio::test]
async fn idle_compact_now_without_region_degrades() {
    let (listener, _tx) = listener(FakeMode::Ok("checkpoint body".to_string()));
    let ctx = ctx();
    // 单条小消息全部落在 retain 区 → 无可压缩区域 → 事务未开、无 bracket。
    ctx.session.append(SessionEvent::UserMessage {
        turn: 1,
        step: 1,
        message: AgentMessage::user("tiny"),
    });

    listener.compact_now(&ctx.session).await;
    assert!(compaction_kinds(&ctx.session).is_empty());
}
