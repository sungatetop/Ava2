//! `tools::workflow` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//!
//! 覆盖工具层契约：入参形状校验、结果截断、前台成败呈示（非 completed 记
//! is_error）、后台注册为 workflow job 并落地终态。子代理一律用回声哑 LLM。

use std::sync::Arc;

use onion_jobs::{JobKind, JobOutcome, JobRegistry, JobStatus, SessionId};
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::*;
use crate::config::Config;
use crate::context::RequestDraft;
use crate::llm::{AssistantTurn, LlmPort, LlmStream, LlmStreamItem};
use crate::subagent::runtime::{ChildSession, SubagentRuntime};
use crate::tools::jobs::{owner_cell, wire_owner, Owner};
use crate::tools::{ToolPort, ToolRegistry};
use crate::types::{AgentMessage, ContentBlock, StopReason as LlmStopReason, Usage};
use crate::waterfall::BoxFuture;

/// 回声哑 LLM：回显本轮最后一条文本消息。
struct EchoLlm;

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

fn runtime() -> Arc<SubagentRuntime> {
    let config = Config::from_env();
    let factory_config = config.clone();
    Arc::new(SubagentRuntime::new(
        1,
        config,
        Arc::new(EchoLlm),
        Arc::new(move |_cwd: Option<&str>, _preset: Option<&str>| {
            Ok(ChildSession::new(&factory_config, ToolRegistry::new()))
        }),
        None,
    ))
}

fn tool() -> (WorkflowTool, Arc<JobRegistry>, Owner) {
    let registry = Arc::new(JobRegistry::new());
    let owner = owner_cell();
    wire_owner(&owner, "session-1");
    (WorkflowTool::new(runtime(), registry.clone(), owner.clone()), registry, owner)
}

fn call(input: Value) -> ToolCall {
    ToolCall { id: "call-1".to_string(), name: "workflow".to_string(), input }
}

/// 直接读取工具返回的单条文本块（非文本块视为失败）。
fn text_of(blocks: Vec<ContentBlock>) -> String {
    match blocks.as_slice() {
        [ContentBlock::Text { text }] => text.clone(),
        other => panic!("expected a single text block, got {other:?}"),
    }
}

#[tokio::test]
async fn foreground_completed_program_returns_run_value() {
    let (tool, _registry, _owner) = tool();
    let blocks = tool
        .execute(
            call(json!({
                "program": {
                    "meta": { "name": "demo", "description": "demo" },
                    "steps": [{ "id": "one", "prompt": "work on {{args.task}}" }]
                },
                "args": { "task": "docs" }
            })),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    let text = text_of(blocks);
    assert!(text.starts_with("workflow \"demo\" completed (1 agents)."));
    assert!(text.contains("echo:work on docs"));
}

#[tokio::test]
async fn foreground_program_error_is_reported_as_is_error() {
    let (tool, _registry, _owner) = tool();
    let error = tool
        .execute(
            call(json!({
                "program": {
                    "meta": { "name": "bad", "description": "demo" },
                    "steps": [{ "id": "one", "prompt": "{{args.missing}}" }]
                }
            })),
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(error.starts_with("workflow \"bad\" error (0 agents)."));
    assert!(error.contains("does not resolve"));
}

#[tokio::test]
async fn background_run_registers_workflow_job_and_settles() {
    let (tool, registry, owner) = tool();
    let blocks = tool
        .execute(
            call(json!({
                "program": {
                    "meta": { "name": "bg", "description": "demo" },
                    "steps": [{ "id": "one", "prompt": "{{args.missing}}" }]
                },
                "run_in_background": true
            })),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    let text = text_of(blocks);
    assert!(text.starts_with("workflow \"bg\" started in background as job workflow-"));
    let owner = owner.get().cloned().unwrap_or_else(|| SessionId("unwired".to_string()));
    let views = registry.list(&owner);
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].spec.kind, JobKind::Workflow);
    // 等待后台任务落地（模板错误 → Failed）。
    let mut settled = None;
    for _ in 0..200 {
        if let Some(view) = registry.get(&views[0].spec.id.0) {
            if let JobStatus::Settled(outcome) = view.status {
                settled = Some(outcome);
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    match settled {
        Some(JobOutcome::Failed { error }) => assert!(error.contains("does not resolve")),
        other => panic!("expected a failed background workflow, got {other:?}"),
    }
}

#[tokio::test]
async fn background_requires_wired_session_owner() {
    let registry = Arc::new(JobRegistry::new());
    let tool = WorkflowTool::new(runtime(), registry, owner_cell());
    let error = tool
        .execute(
            call(json!({
                "program": {
                    "meta": { "name": "bg", "description": "demo" },
                    "steps": [{ "id": "one", "prompt": "go" }]
                },
                "run_in_background": true
            })),
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(error.contains("session owner not wired"));
}

#[test]
fn parse_input_rejects_malformed_arguments() {
    assert!(WorkflowTool::parse_input(&json!({})).unwrap_err().contains("`program` is required"));
    assert!(WorkflowTool::parse_input(&json!({
        "program": {
            "meta": { "name": "x", "description": "y" },
            "steps": [{ "id": "one", "prompt": "go" }]
        },
        "args": []
    }))
    .unwrap_err()
    .contains("`args` must be an object"));
    let (_, args) = WorkflowTool::parse_input(&json!({
        "program": {
            "meta": { "name": "x", "description": "y" },
            "steps": [{ "id": "one", "prompt": "go" }]
        }
    }))
    .unwrap();
    assert_eq!(args, json!({}));
}

#[test]
fn truncate_chars_marks_overflow_without_splitting_utf8() {
    assert_eq!(truncate_chars("short".to_string(), 10), "short");
    let long = "中".repeat(12);
    let truncated = truncate_chars(long, 5);
    assert!(truncated.starts_with("中中中中中"));
    assert!(truncated.contains("[result truncated]"));
    assert_eq!(MAX_RESULT_CHARS, 50_000);
}
