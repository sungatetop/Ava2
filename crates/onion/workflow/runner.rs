//! workflow 执行器：串行步骤 + `for_each` 并发 fan-out + 取消 + 进度回调。
//!
//! 每个步骤经 [`SubagentRuntime::run_round`] 派发一个**全新子代理**（depth=1，
//! 与 ralph / subagent 工具一致固定一层），返回其最终 assistant 文本。
//!
//! 核心语义：
//! - 单个 agent 失败不终止整个 workflow——该步骤（或该 fan-out 项）输出为 `null`；
//! - 终态三态：`completed`（全部步骤跑完）/ `cancelled`（取消令牌触发）/
//!   `error`（程序级错误：模板未解析、`for_each` 路径不存在等）；
//! - 最终 value 为 `{ "<step id>": <output> }`（fan-out 步骤为数组）。
//!
//! 取消粒度：取消令牌触发时执行器停止派发新步骤并放弃在飞 fan-out future
//! （子会话自身的驱动随之 drop），已完成步骤的输出保留在 value 中。

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde_json::{json, Map, Value};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

use super::program::{
    render_prompt, resolve_for_each, Program, Step, TemplateContext,
};
use crate::subagent::runtime::SubagentRuntime;

/// 步骤派发子代理的相对深度（固定一层：workflow 子代理无本工具族，不递归）。
const STEP_DEPTH: u32 = 1;

/// run id 序列（进程内单调递增，避免同毫秒碰撞）。
static RUN_SEQ: AtomicU64 = AtomicU64::new(0);

/// 终态（workflow stop reason 三态）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopReason {
    Completed,
    Cancelled,
    Error,
}

impl StopReason {
    /// 模型可见文本（`completed` / `cancelled` / `error`）。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Error => "error",
        }
    }
}

/// 进度回调（后台 job 用它把进度写进输出环；前台传 None）。
pub type Progress = Arc<dyn Fn(&str) + Send + Sync>;

/// 一次 workflow 运行结果。
#[derive(Debug)]
pub struct RunReport {
    pub run_id: String,
    /// 已派发的子代理数（fan-out 按项计）。
    pub agents_started: u64,
    pub stop_reason: StopReason,
    /// 程序级错误（`stop_reason == Error` 时非空）。
    pub error: Option<String>,
    /// `{ "<step id>": <output> }`。
    pub value: Value,
}

/// 执行一个 workflow 程序。
pub async fn run(
    runtime: Arc<SubagentRuntime>,
    program: &Program,
    args: &Value,
    cancel: CancellationToken,
    progress: Option<Progress>,
) -> RunReport {
    let run_id = format!(
        "workflow-{}-{}",
        crate::session::epoch_millis(),
        RUN_SEQ.fetch_add(1, Ordering::SeqCst)
    );
    emit(&progress, format!("workflow \"{}\" started ({run_id})", program.meta.name));
    let mut outputs: BTreeMap<String, Value> = BTreeMap::new();
    let mut agents_started: u64 = 0;
    let mut stop_reason = StopReason::Completed;
    let mut error: Option<String> = None;
    let mut current_phase: Option<String> = None;

    for step in &program.steps {
        if cancel.is_cancelled() {
            stop_reason = StopReason::Cancelled;
            break;
        }
        announce_phase(program, step, &mut current_phase, &progress);
        let label = step.label.as_deref().unwrap_or(&step.id);

        let Some(for_each) = &step.for_each else {
            // 单步：模板渲染 → 派发 1 个子代理。
            let prompt = match render_prompt(step, &context(args, &outputs, None, None)) {
                Ok(prompt) => prompt,
                Err(failure) => {
                    stop_reason = StopReason::Error;
                    error = Some(failure);
                    break;
                }
            };
            emit(&progress, format!("step {label}: started"));
            agents_started += 1;
            match runtime.run_round(prompt, STEP_DEPTH).await {
                Ok(text) => {
                    emit(&progress, format!("step {label}: completed"));
                    outputs.insert(step.id.clone(), Value::String(text));
                }
                Err(failure) => {
                    // 单个 agent 失败解析为 null，不终止整个 workflow。
                    emit(&progress, format!("step {label}: failed ({failure})"));
                    outputs.insert(step.id.clone(), Value::Null);
                }
            }
            continue;
        };

        // fan-out：路径解析 → 逐项渲染 → 信号量限流并发派发。
        let items = match resolve_for_each(for_each, args) {
            Ok(items) => items,
            Err(failure) => {
                stop_reason = StopReason::Error;
                error = Some(failure);
                break;
            }
        };
        let mut prompts = Vec::with_capacity(items.len());
        let mut render_error = None;
        for (index, item) in items.iter().enumerate() {
            match render_prompt(step, &context(args, &outputs, Some(item), Some(index))) {
                Ok(prompt) => prompts.push(prompt),
                Err(failure) => {
                    render_error = Some(failure);
                    break;
                }
            }
        }
        if let Some(failure) = render_error {
            stop_reason = StopReason::Error;
            error = Some(failure);
            break;
        }
        let dispatched = prompts.len() as u64;
        emit(
            &progress,
            format!(
                "step {label}: fan-out over {dispatched} item(s), concurrency {}",
                program.concurrency
            ),
        );
        agents_started += dispatched;
        let semaphore = Arc::new(Semaphore::new(program.concurrency));
        let futures = prompts.into_iter().enumerate().map(|(index, prompt)| {
            let runtime = runtime.clone();
            let semaphore = semaphore.clone();
            async move {
                // 并发闸门：拿到许可才真正派发（许可随 future 结束释放）。
                let Ok(_permit) = semaphore.acquire().await else {
                    return (index, Err("workflow concurrency gate closed".to_string()));
                };
                (index, runtime.run_round(prompt, STEP_DEPTH).await.map_err(|failure| failure.to_string()))
            }
        });
        let results = tokio::select! {
            _ = cancel.cancelled() => {
                stop_reason = StopReason::Cancelled;
                break;
            }
            results = futures_util::future::join_all(futures) => results,
        };
        let mut values = vec![Value::Null; results.len()];
        let mut succeeded = 0usize;
        for (index, result) in results {
            match result {
                Ok(text) => {
                    values[index] = Value::String(text);
                    succeeded += 1;
                }
                Err(failure) => emit(&progress, format!("step {label}[{index}]: failed ({failure})")),
            }
        }
        emit(
            &progress,
            format!("step {label}: completed ({succeeded}/{})", values.len()),
        );
        outputs.insert(step.id.clone(), Value::Array(values));
    }

    let value = Value::Object(outputs.into_iter().collect::<Map<String, Value>>());
    emit(
        &progress,
        format!(
            "workflow \"{}\" {} ({agents_started} agent(s))",
            program.meta.name,
            stop_reason.as_str()
        ),
    );
    RunReport { run_id, agents_started, stop_reason, error, value }
}

/// 组装模板上下文。
fn context<'a>(
    args: &'a Value,
    outputs: &'a BTreeMap<String, Value>,
    item: Option<&'a Value>,
    index: Option<usize>,
) -> TemplateContext<'a> {
    TemplateContext { args, outputs, item, index }
}

/// 进入新 phase 时输出一行进度（`meta.phases` 声明的 detail 一并带出）。
fn announce_phase(
    program: &Program,
    step: &Step,
    current: &mut Option<String>,
    progress: &Option<Progress>,
) {
    let Some(phase) = &step.phase else {
        return;
    };
    if current.as_deref() == Some(phase.as_str()) {
        return;
    }
    match program
        .meta
        .phases
        .iter()
        .find(|declared| &declared.title == phase)
        .and_then(|declared| declared.detail.as_deref())
    {
        Some(detail) => emit(progress, format!("phase: {phase} — {detail}")),
        None => emit(progress, format!("phase: {phase}")),
    }
    *current = Some(phase.clone());
}

/// 输出一行进度（无回调时静默）。
fn emit(progress: &Option<Progress>, line: impl Into<String>) {
    if let Some(progress) = progress {
        progress(&line.into());
    }
}

/// 模型可见的结果投影（前台工具与后台 job 收尾共用）。
pub fn result_view(report: &RunReport) -> Value {
    json!({
        "runId": report.run_id,
        "agentsStarted": report.agents_started,
        "result": {
            "stopReason": report.stop_reason.as_str(),
            "value": report.value,
            "error": report.error,
        }
    })
}
