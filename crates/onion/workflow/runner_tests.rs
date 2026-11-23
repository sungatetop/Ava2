//! `workflow::runner` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//!
//! 用回声哑 LLM（把收到的 prompt 原样回显为 assistant 文本）验证执行器不变量：
//! 步骤串行与模板引用、`for_each` fan-out 的项数与顺序、取消与程序级错误。
//! 真实模型链路由 `examples/` 冒烟覆盖。

use std::sync::{Arc, Mutex};

use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::*;
use crate::config::Config;
use crate::context::RequestDraft;
use crate::llm::{AssistantTurn, LlmPort, LlmStream, LlmStreamItem};
use crate::subagent::runtime::{ChildSession, SubagentRuntime};
use crate::tools::ToolRegistry;
use crate::types::{AgentMessage, ContentBlock, StopReason as LlmStopReason, Usage};
use crate::waterfall::BoxFuture;
use crate::workflow::Program;

/// 回声哑 LLM：把本轮最后一条文本消息回显为 `echo:<text>`，并记录收到的 prompt。
struct EchoLlm {
    seen: Arc<Mutex<Vec<String>>>,
}

impl LlmPort for EchoLlm {
    fn stream(&self, draft: &RequestDraft, _cancel: CancellationToken) -> BoxFuture<LlmStream> {
        let prompt = draft
            .messages
            .iter()
            .rev()
            .find_map(|message| {
                message.blocks.iter().find_map(|block| match block {
                    ContentBlock::Text { text } => Some(text.clone()),
                    _ => None,
                })
            })
            .unwrap_or_default();
        self.seen.lock().unwrap().push(prompt.clone());
        Box::pin(async move {
            let turn = AssistantTurn {
                message: AgentMessage::assistant(vec![ContentBlock::Text {
                    text: format!("echo:{prompt}"),
                }]),
                stop_reason: LlmStopReason::EndTurn,
                error: None,
                failure: None,
                cancelled: false,
                usage: Usage::default(),
                model: "fake".to_string(),
            };
            Box::pin(futures_util::stream::iter(vec![LlmStreamItem::Done(turn)])) as LlmStream
        })
    }
}

fn runtime() -> (Arc<SubagentRuntime>, Arc<Mutex<Vec<String>>>) {
    let config = Config::from_env();
    let factory_config = config.clone();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let runtime = Arc::new(SubagentRuntime::new(
        1,
        config,
        Arc::new(EchoLlm { seen: seen.clone() }),
        Arc::new(move |_cwd: Option<&str>, _preset: Option<&str>| {
            Ok(ChildSession::new(&factory_config, ToolRegistry::new()))
        }),
        None,
    ));
    (runtime, seen)
}

fn program(value: serde_json::Value) -> Program {
    Program::parse(&value).unwrap()
}

#[tokio::test]
async fn runs_steps_in_order_and_threads_step_outputs() {
    let (runtime, seen) = runtime();
    let program = program(json!({
        "meta": { "name": "demo", "description": "demo" },
        "steps": [
            { "id": "scan", "prompt": "scan {{args.root}}" },
            { "id": "report", "prompt": "report on {{steps.scan}}" }
        ]
    }));
    let report = super::run(
        runtime,
        &program,
        &json!({ "root": "/tmp/x" }),
        CancellationToken::new(),
        None,
    )
    .await;
    assert_eq!(report.stop_reason, StopReason::Completed);
    assert_eq!(report.agents_started, 2);
    assert_eq!(
        report.value,
        json!({ "scan": "echo:scan /tmp/x", "report": "echo:report on echo:scan /tmp/x" })
    );
    assert_eq!(
        seen.lock().unwrap().as_slice(),
        ["scan /tmp/x".to_string(), "report on echo:scan /tmp/x".to_string()]
    );
}

#[tokio::test]
async fn fan_out_dispatches_one_agent_per_item_in_order() {
    let (runtime, seen) = runtime();
    let program = program(json!({
        "meta": { "name": "fan", "description": "demo" },
        "steps": [
            { "id": "each", "prompt": "item={{index}}:{{item}}", "for_each": "files" }
        ],
        "concurrency": 2
    }));
    let report = super::run(
        runtime,
        &program,
        &json!({ "files": ["a.txt", "b.txt", "c.txt"] }),
        CancellationToken::new(),
        None,
    )
    .await;
    assert_eq!(report.stop_reason, StopReason::Completed);
    assert_eq!(report.agents_started, 3);
    // 结果按输入顺序排列（并发完成顺序不影响下标装配）。
    assert_eq!(
        report.value,
        json!({ "each": ["echo:item=0:a.txt", "echo:item=1:b.txt", "echo:item=2:c.txt"] })
    );
    let mut seen = seen.lock().unwrap().clone();
    seen.sort();
    assert_eq!(
        seen,
        ["item=0:a.txt", "item=1:b.txt", "item=2:c.txt"]
            .map(str::to_string)
            .to_vec()
    );
}

#[tokio::test]
async fn cancelled_token_stops_before_dispatching() {
    let (runtime, seen) = runtime();
    let program = program(json!({
        "meta": { "name": "cancel", "description": "demo" },
        "steps": [{ "id": "one", "prompt": "never" }]
    }));
    let cancel = CancellationToken::new();
    cancel.cancel();
    let report = super::run(runtime, &program, &json!({}), cancel, None).await;
    assert_eq!(report.stop_reason, StopReason::Cancelled);
    assert_eq!(report.agents_started, 0);
    assert_eq!(report.value, json!({}));
    assert!(seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn missing_for_each_path_is_program_error() {
    let (runtime, seen) = runtime();
    let program = program(json!({
        "meta": { "name": "bad", "description": "demo" },
        "steps": [{ "id": "each", "prompt": "{{item}}", "for_each": "files" }]
    }));
    let report = super::run(runtime, &program, &json!({}), CancellationToken::new(), None).await;
    assert_eq!(report.stop_reason, StopReason::Error);
    assert!(report.error.unwrap_or_default().contains("does not resolve in `args`"));
    assert_eq!(report.agents_started, 0);
    assert!(seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn forward_step_reference_is_program_error() {
    let (runtime, _seen) = runtime();
    let program = program(json!({
        "meta": { "name": "forward", "description": "demo" },
        "steps": [
            { "id": "first", "prompt": "uses {{steps.second}}" },
            { "id": "second", "prompt": "later" }
        ]
    }));
    let report = super::run(runtime, &program, &json!({}), CancellationToken::new(), None).await;
    assert_eq!(report.stop_reason, StopReason::Error);
    assert!(report
        .error
        .unwrap_or_default()
        .contains("has not produced output yet"));
}

#[tokio::test]
async fn progress_callback_receives_phase_and_step_lines() {
    let (runtime, _seen) = runtime();
    let program = program(json!({
        "meta": {
            "name": "progress",
            "description": "demo",
            "phases": [{ "title": "scan", "detail": "reading" }]
        },
        "steps": [{ "id": "one", "phase": "scan", "prompt": "go" }]
    }));
    let lines = Arc::new(Mutex::new(Vec::new()));
    let sink = lines.clone();
    let progress: Progress = Arc::new(move |line: &str| sink.lock().unwrap().push(line.to_string()));
    let report = super::run(
        runtime,
        &program,
        &json!({}),
        CancellationToken::new(),
        Some(progress),
    )
    .await;
    assert_eq!(report.stop_reason, StopReason::Completed);
    let lines = lines.lock().unwrap().clone();
    assert!(lines.iter().any(|line| line == "phase: scan — reading"));
    assert!(lines.iter().any(|line| line == "step one: started"));
    assert!(lines.iter().any(|line| line == "step one: completed"));
}
