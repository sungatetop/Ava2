//! CompactionListener：上下文压缩 listener。
//!
//! 三条触发通道：
//! - `pre_step` 压力评估：投影重建后 `estimate_context >= threshold_tokens` 触发，
//!   失败降级（仍 Enter，压缩对路由透明）；
//! - `request_error` 溢出恢复：结构化溢出分类（ContextWindowExceeded）→ 每 step 限
//!   spec.max_overflow_retries 次事务 → 修复后显式立即重试（内核链尾对溢出分类默认
//!   Abort，恢复决策由本 listener 显式给出）；事务失败 → Abort（上下文未修复，
//!   重试无意义）；
//! - 人工 `/compact`：mpsc 信号（main 持发送端），在最近一次 pre_step 窗口内
//!   强制执行；空闲时无 pre_step 窗口，宿主经 `compact_now` 直接驱动事务。
//!
//! 事务：select_compactable_range →
//! CompactionStart（锁）→ summarize_with_llm（重试）→ 收缩校验 →
//! CompactionSummary → CompactionEnd（释放锁）。全部 durable 事件直写
//! `ctx.session`；内核投影在 dispatch_request 重建时回流（session 是唯一权威）。
//!
//! 内部状态收进 `Arc<Inner>`：瀑布方法签名要求 `BoxFuture`（'static），
//! async 块捕获 `&self` 无法满足，clone Arc 进入 future。

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::compaction::config::CompactSpec;
use crate::compaction::estimate::{estimate_context, estimate_message};
use crate::compaction::region::{
    assert_summary_shrinks, build_summarization_input, select_compactable_range, MIN_SHADOWED_TOKENS,
};
use crate::compaction::summarizer::{SummaryConfig, frame_checkpoint, frame_summary, summarize_with_llm};
use crate::compaction::types::{
    CompactionError, CompactionId, CompactionResult, CompactionTrigger,
};
use crate::context::Context;
use crate::listener::{
    ClaimedInput, Listener, ListenerCtx, PreStepDecision, RequestError, RetryDecision,
};
use crate::llm::LlmPort;
use crate::session::{LiveEvent, SessionEvent, SessionLog};
use crate::types::ToolDefinition;
use crate::waterfall::{BoxFuture, Next};

/// 触发通道落日志标签（与 serde snake_case 词表一致）。
fn trigger_label(trigger: CompactionTrigger) -> &'static str {
    match trigger {
        CompactionTrigger::Pressure => "pressure",
        CompactionTrigger::ContextOverflow => "context_overflow",
        CompactionTrigger::Manual => "manual",
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// 上下文压缩 listener。
pub struct CompactionListener {
    inner: Arc<Inner>,
}

/// listener 内部共享状态。
struct Inner {
    /// 摘要 LLM 端口（复用内核链路的 OpenAiSseLlm）。
    llm: Arc<dyn LlmPort>,
    /// 压力预算（threshold / retain / 摘要重试上限）。
    spec: CompactSpec,
    /// 摘要调用配置。
    summary_config: SummaryConfig,
    /// 工具 schema 快照（投影重建的 tools 表；与内核装配同源 ToolRegistry）。
    tools: Vec<ToolDefinition>,
    /// 人工 `/compact` 信号接收端（main 持发送端；drain 消费，多次请求合并）。
    manual_rx: Mutex<mpsc::UnboundedReceiver<()>>,
    /// compaction id 序号（单调递增）。
    next_seq: AtomicU64,
    /// 当前在飞 step（经 on_event StepStart 跟踪；RequestError 不携带 turn/step）。
    current_step: Mutex<Option<(u32, u32)>>,
    /// 溢出恢复计数（(turn, step) → 已用次数）：每 step 限 spec.max_overflow_retries
    /// 次，事务失败同样消耗次数（上下文未修复，重试无意义，防无限循环）。
    overflow_retries: Mutex<Option<((u32, u32), u32)>>,
}

impl CompactionListener {
    /// 构造：返回 (listener, `/compact` 信号发送端)。
    pub fn new(
        llm: Arc<dyn LlmPort>,
        spec: CompactSpec,
        summary_config: SummaryConfig,
        tools: Vec<ToolDefinition>,
    ) -> (Self, mpsc::UnboundedSender<()>) {
        let (manual_tx, manual_rx) = mpsc::unbounded_channel();
        (
            Self {
                inner: Arc::new(Inner {
                    llm,
                    spec,
                    summary_config,
                    tools,
                    manual_rx: Mutex::new(manual_rx),
                    next_seq: AtomicU64::new(1),
                    current_step: Mutex::new(None),
                    overflow_retries: Mutex::new(None),
                }),
            },
            manual_tx,
        )
    }

    /// 空闲 `/compact` 直驱：无 pre_step 窗口时由宿主（main 的 stdin 分支）直接
    /// 执行压缩事务，信号不再滞留到下一轮。
    ///
    /// turn 取最近一次 TurnStart 的下一轮（尚无 turn 记录则从 1 起）；manual 信号
    /// drain 消费保持幂等（本方法已直接执行，后续 pre_step 不会重复触发）。
    pub async fn compact_now(&self, session: &SessionLog) {
        let turn = session
            .snapshot()
            .iter()
            .rev()
            .find_map(|event| match event {
                SessionEvent::TurnStart { turn } => Some(turn + 1),
                _ => None,
            })
            .unwrap_or(1);

        // 人工信号 drain 消费（幂等：事务由本方法直接执行）。
        while lock(&self.inner.manual_rx).try_recv().is_ok() {}

        let estimate = estimate_context(&self.inner.project(session));
        println!(
            "    \x1b[34m[compact]\x1b[0m t{turn} manual（estimate {estimate} tok）→ 空闲直接执行压缩事务"
        );
        match self.inner.run_compaction(session, turn, CompactionTrigger::Manual).await {
            Ok(result) => println!(
                "    \x1b[34m[compact]\x1b[0m 完成 {}：shadowed {} 条 / {} tok → 摘要 {} tok",
                result.id.0,
                result.shadowed_count,
                result.shadowed_tokens,
                result.summary_tokens
            ),
            Err(error) => {
                println!("    \x1b[34m[compact]\x1b[0m 失败（{}）", error.label());
            }
        }
    }

    /// 空闲 `/compact` 直驱并返回事务结果（REST 命令面需要计数回执）。
    ///
    /// 与 [`Self::compact_now`] 同语义，只是把结果交给调用方而非仅打印。
    pub async fn compact_now_result(
        &self,
        session: &SessionLog,
    ) -> Result<CompactionResult, CompactionError> {
        let turn = session
            .snapshot()
            .iter()
            .rev()
            .find_map(|event| match event {
                SessionEvent::TurnStart { turn } => Some(turn + 1),
                _ => None,
            })
            .unwrap_or(1);
        while lock(&self.inner.manual_rx).try_recv().is_ok() {}
        self.inner.run_compaction(session, turn, CompactionTrigger::Manual).await
    }
}

impl Inner {
    /// 投影重建（durable 日志是唯一权威；listener 与内核内存 context 解耦，
    /// 事务以「提交时刻」的投影为准）。
    fn project(&self, session: &SessionLog) -> Context {
        Context::project(&session.snapshot(), self.tools.clone())
    }

    /// 压缩事务：bracket 事件成对落日志。
    async fn run_compaction(
        &self,
        session: &SessionLog,
        turn: u32,
        trigger: CompactionTrigger,
    ) -> Result<CompactionResult, CompactionError> {
        let seq = self.next_seq.fetch_add(1, Ordering::SeqCst);
        let id = CompactionId::new(seq);
        let context = self.project(session);

        // ① 范围选择：无可压缩区域 → Nothing（事务未开，无 bracket）。
        let range = select_compactable_range(&context.messages, &self.spec)
            .ok_or(CompactionError::Nothing)?;
        let shadowed_tokens: u64 = context.messages[range.start..range.start + range.count]
            .iter()
            .map(estimate_message)
            .sum();

        // ①′ 下限保护：shadowed region 估算 < MIN_SHADOWED_TOKENS 时摘要必然
        // 不划算——框架摘要自身数百 token，事务注定以 no-shrink 失败收场
        // （E1.5 卸载图片后 shadowed 仅 80 tok 仍发起摘要的实证回归）。低于
        // 下限直接跳过（事务未开，无 bracket 事件）。
        if shadowed_tokens < MIN_SHADOWED_TOKENS {
            return Err(CompactionError::Nothing);
        }

        // ② bracket 打开：CompactionStart（持久化锁）。
        session.append(SessionEvent::CompactionStart {
            turn,
            compaction_id: id.0.clone(),
            trigger: trigger_label(trigger).to_string(),
        });

        // ③ 摘要调用（首次 + compact_max_retries 次重试）。教学示例的 listener
        // 不与内核取消令牌联动：事务一旦开启自然跑完（bracket 完整性优先）。
        let replay = build_summarization_input(&context.messages, range);
        let attempts = self.spec.compact_max_retries.saturating_add(1);
        let mut success = None;
        let mut last_error = String::new();
        for _ in 0..attempts {
            match summarize_with_llm(
                self.llm.as_ref(),
                self.summary_config.clone(),
                replay.clone(),
                CancellationToken::new(),
            )
            .await
            {
                Ok(result) => {
                    success = Some(result);
                    break;
                }
                Err(error) => last_error = error.0,
            }
        }
        let Some(result) = success else {
            self.end_failed(session, turn, &id, &last_error);
            return Err(CompactionError::Summarizer);
        };

        // ④ 收缩校验：框架摘要必须严格小于被替换区域。
        let framed = frame_summary(&result.summary);
        let summary_tokens = estimate_message(&framed);
        if let Err(reason) = assert_summary_shrinks(summary_tokens, shadowed_tokens) {
            self.end_failed(session, turn, &id, &reason);
            return Err(CompactionError::NoShrink);
        }

        // ⑤ bracket 提交：CompactionSummary（内核投影重建时按区间替换）。
        // 先取下标再落事件：该下标即摘要事件的 durable 位置。
        let summary_seq = session.len() as u64;
        session.append(SessionEvent::CompactionSummary {
            turn,
            compaction_id: id.0.clone(),
            summary: result.summary.clone(),
            shadowed_count: range.count,
            shadowed_tokens,
            usage: result.usage,
        });
        // ⑤′ 检查点消息：`compaction/summary` 之后紧随一条
        // `source: compact-checkpoint` 的 user 消息。正文即摘要承载帧；投影层
        // 已由 ⑤ 完成替换，此事件的价值是 durable 层面的显式语义标记——
        // session_query / 前端无需解析正文即可识别压缩点，也不会把它当成
        // 真实用户输入（标题、goal 权威判定等均按 source 排除）。
        session.append(SessionEvent::UserMessage {
            turn,
            step: 0,
            message: frame_checkpoint(&result.summary, &id.0),
        });
        // ⑥ bracket 关闭：CompactionEnd（释放锁）。
        session.append(SessionEvent::CompactionEnd {
            turn,
            compaction_id: id.0.clone(),
            error: None,
        });

        Ok(CompactionResult {
            id,
            trigger,
            shadowed_start: range.start,
            shadowed_count: range.count,
            shadowed_tokens,
            summary_tokens,
            usage: result.usage,
            summary_seq,
        })
    }

    /// 失败收尾：CompactionEnd 携带 error（Context 不动，session 继续）。
    fn end_failed(&self, session: &SessionLog, turn: u32, id: &CompactionId, error: &str) {
        session.append(SessionEvent::CompactionEnd {
            turn,
            compaction_id: id.0.clone(),
            error: Some(error.to_string()),
        });
    }
}

impl Listener for CompactionListener {
    /// 跟踪当前 step（RequestError 不携带 turn/step，溢出限流依赖此快照）。
    fn on_event(&self, _ctx: &ListenerCtx, event: &LiveEvent) {
        if let LiveEvent::Durable(SessionEvent::StepStart { turn, step }) = event {
            *lock(&self.inner.current_step) = Some((*turn, *step));
        }
    }

    /// 步准入前压缩评估：manual 信号强制 / 压力超阈触发；失败降级继续。
    fn pre_step(
        &self,
        ctx: ListenerCtx,
        input: ClaimedInput,
        next: Next<ClaimedInput, PreStepDecision>,
    ) -> BoxFuture<PreStepDecision> {
        let inner = self.inner.clone();
        Box::pin(async move {
            // ① 人工信号 drain 消费（多次请求合并，幂等）。
            let mut manual = false;
            while lock(&inner.manual_rx).try_recv().is_ok() {
                manual = true;
            }

            // ② 压力评估（投影重建：durable 日志是唯一权威）。
            let estimate = estimate_context(&inner.project(&ctx.session));
            let pressured = estimate >= inner.spec.threshold_tokens;
            if manual || pressured {
                let trigger =
                    if manual { CompactionTrigger::Manual } else { CompactionTrigger::Pressure };
                let verdict = if manual {
                    format!("manual（estimate {estimate} tok）")
                } else {
                    format!("pressure {estimate}/{} tok", inner.spec.threshold_tokens)
                };
                println!(
                    "    \x1b[34m[compact]\x1b[0m t{}.s{} {verdict} → 执行压缩事务",
                    input.turn, input.step
                );
                match inner.run_compaction(&ctx.session, input.turn, trigger).await {
                    Ok(result) => println!(
                        "    \x1b[34m[compact]\x1b[0m 完成 {}：shadowed {} 条 / {} tok → 摘要 {} tok",
                        result.id.0, result.shadowed_count, result.shadowed_tokens,
                        result.summary_tokens
                    ),
                    Err(error) => {
                        println!("    \x1b[34m[compact]\x1b[0m 失败（{}）→ 降级继续", error.label());
                    }
                }
            }
            // ③ 透传：本步批次消息落在新投影上（Enter 后 dispatch_request 重建投影）。
            next(input).await
        })
    }

    /// 溢出恢复：结构化溢出分类（ContextWindowExceeded）→ 每 step 限
    /// spec.max_overflow_retries 次事务 → 修复后显式立即重试；事务失败 →
    /// Abort（短路，上下文未修复重试无意义）。内核链尾对溢出分类默认 Abort，
    /// 恢复决策必须由本 listener 显式给出（不委托内层）。
    fn request_error(
        &self,
        ctx: ListenerCtx,
        error: RequestError,
        next: Next<RequestError, RetryDecision>,
    ) -> BoxFuture<RetryDecision> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let Some(failure) = &error.failure else {
                return next(error).await;
            };
            if error.cancelled || failure.code != crate::llm::LlmFailureCode::ContextWindowExceeded {
                return next(error).await;
            }
            // 每 step 限次（RequestError 不携带 turn/step，用 on_event 快照）。
            let Some((turn, step)) = *lock(&inner.current_step) else {
                return next(error).await;
            };
            let exhausted = {
                let mut retries = lock(&inner.overflow_retries);
                match *retries {
                    Some(((seen_turn, seen_step), used)) if (seen_turn, seen_step) == (turn, step) => {
                        if used >= inner.spec.max_overflow_retries {
                            true
                        } else {
                            *retries = Some(((turn, step), used + 1));
                            false
                        }
                    }
                    _ => {
                        *retries = Some(((turn, step), 1));
                        false
                    }
                }
            };
            if exhausted {
                return next(error).await;
            }
            let head: String = error.message.chars().take(80).collect();
            println!("    \x1b[34m[compact]\x1b[0m t{turn} 溢出恢复（{head}…）→ 执行压缩事务");
            match inner.run_compaction(&ctx.session, turn, CompactionTrigger::ContextOverflow).await
            {
                // 修复完成 → 显式立即重试（内核链尾对溢出分类 Abort，不能委托）。
                Ok(result) => {
                    println!(
                        "    \x1b[34m[compact]\x1b[0m 溢出恢复完成 {}（-{} tok）",
                        result.id.0,
                        result.shadowed_tokens.saturating_sub(result.summary_tokens)
                    );
                    RetryDecision::Retry { delay_ms: 0 }
                }
                Err(failed) => {
                    println!(
                        "    \x1b[34m[compact]\x1b[0m 溢出恢复失败（{}）→ abort",
                        failed.label()
                    );
                    RetryDecision::Abort
                }
            }
        })
    }
}

#[cfg(test)]
#[path = "listener_tests.rs"]
mod tests;
