//! `workflow` 工具：声明式 JSON 程序的子代理编排。
//!
//! 与经典脚本式编排的差异（诚实降级）：脚本式 `tool-workflow` 接收一段 JavaScript
//! （`agent()` / `pipeline` / `parallel` / `phase` / `log` 钩子）并在沙箱 JS 运行时
//! 执行；本实现不引入 JS 运行时，改收**声明式 JSON 程序**（见 [`crate::workflow`]）。
//! 保留的对齐点：
//! - 单个 agent 失败解析为 `null`（不终止整个编排）；
//! - 终态三态 `completed` / `cancelled` / `error`，非 completed 一律按 is_error 呈示；
//! - 结果 JSON 上限 [`MAX_RESULT_CHARS`] 字符（超出截断并标注）；
//! - `run_in_background` 注册为后台 job，经 `job_output` 读进度、`job_kill` 取消。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use onion_jobs::{JobId, JobKind, JobOutcome, JobRegistry, JobSpec, OutputRing};
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::jobs::{current_owner, Owner};
use super::ToolPort;
use crate::approval::ToolRisk;
use crate::subagent::runtime::SubagentRuntime;
use crate::types::{ContentBlock, ToolCall, ToolDefinition};
use crate::waterfall::BoxFuture;
use crate::workflow::{self, Program, StopReason};

/// 结果 JSON 字符上限。
const MAX_RESULT_CHARS: usize = 50_000;

/// 后台 job id 序列（进程内单调递增）。
static JOB_SEQ: AtomicU64 = AtomicU64::new(0);

/// workflow 工具（模型侧一次调用 = 一次编排运行）。
pub struct WorkflowTool {
    runtime: Arc<SubagentRuntime>,
    registry: Arc<JobRegistry>,
    owner: Owner,
}

impl WorkflowTool {
    pub fn new(runtime: Arc<SubagentRuntime>, registry: Arc<JobRegistry>, owner: Owner) -> Self {
        Self { runtime, registry, owner }
    }

    /// 解析 `program` 与 `args`（形状错误一次性报给模型）。
    fn parse_input(input: &Value) -> Result<(Program, Value), String> {
        let program_value = input
            .get("program")
            .ok_or_else(|| "`program` is required".to_string())?;
        let program = Program::parse(program_value)?;
        let args = match input.get("args") {
            None | Some(Value::Null) => json!({}),
            Some(value @ Value::Object(_)) => value.clone(),
            Some(_) => return Err("`args` must be an object".to_string()),
        };
        Ok((program, args))
    }
}

impl ToolPort for WorkflowTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "workflow".to_string(),
            description: "Run a declarative JSON workflow that fans work out to subagents. Unlike \
                script-based workflow engines this takes a JSON program: `steps` run in order, each \
                step dispatches one fresh subagent with its `prompt`; a step with `for_each` (array \
                path inside `args`) dispatches one subagent per item concurrently (bounded by \
                `concurrency`, default 4). Prompts may reference `{{args.<path>}}`, \
                `{{steps.<id>}}`, `{{item}}` and `{{index}}`; unresolved references fail the run. A \
                subagent that fails yields null for that step instead of aborting the run. Use this \
                for independent parallel research, multi-file review, or map-reduce over a list; use \
                `subagent` for a single delegation and `ralph` only when the human asks for Ralph."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "program": {
                        "type": "object",
                        "description": "The workflow program.",
                        "properties": {
                            "meta": {
                                "type": "object",
                                "description": "Program metadata.",
                                "properties": {
                                    "name": { "type": "string", "description": "Short program name." },
                                    "description": { "type": "string", "description": "What the workflow does." },
                                    "when_to_use": { "type": "string", "description": "Optional usage guidance." },
                                    "phases": {
                                        "type": "array",
                                        "description": "Optional progress phases; step.phase must match a declared title.",
                                        "items": {
                                            "type": "object",
                                            "properties": {
                                                "title": { "type": "string" },
                                                "detail": { "type": "string" }
                                            },
                                            "required": ["title"]
                                        }
                                    }
                                },
                                "required": ["name", "description"]
                            },
                            "steps": {
                                "type": "array",
                                "description": "Steps executed in order.",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "id": { "type": "string", "description": "Unique step id (letters, digits, `_`, `-`)." },
                                        "phase": { "type": "string", "description": "Progress phase title (must be declared in meta.phases when phases are declared)." },
                                        "prompt": { "type": "string", "description": "Subagent prompt template." },
                                        "for_each": { "type": "string", "description": "Array path inside args; dispatches one subagent per item and yields an array." },
                                        "label": { "type": "string", "description": "Optional display label (defaults to the id)." }
                                    },
                                    "required": ["id", "prompt"]
                                }
                            },
                            "concurrency": { "type": "number", "description": "Concurrent subagents per fan-out step (default 4, capped at 16)." }
                        },
                        "required": ["meta", "steps"]
                    },
                    "args": { "type": "object", "description": "Values referenced by prompt templates." },
                    "run_in_background": { "type": "boolean", "description": "Run as a background job and return its job id (default false)." }
                },
                "required": ["program"]
            }),
        }
    }

    fn risk(&self) -> ToolRisk {
        // 派生 subagent（可能写入工作区），与 subagent 工具同级。
        ToolRisk::WorkspaceWrite
    }

    fn execute(&self, call: ToolCall, cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let runtime = self.runtime.clone();
        let registry = self.registry.clone();
        let owner = self.owner.clone();
        let input = call.input;
        Box::pin(async move {
            let (program, args) = WorkflowTool::parse_input(&input)?;
            let background = input
                .get("run_in_background")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if background {
                return start_background(runtime, registry, owner, program, args, cancel);
            }
            let report = workflow::run(runtime, &program, &args, cancel, None).await;
            let body = render_result(&report);
            let text = format!(
                "workflow \"{}\" {} ({} agents).\nReturn value:\n{body}",
                program.meta.name,
                report.stop_reason.as_str(),
                report.agents_started
            );
            if report.stop_reason == StopReason::Completed {
                Ok(vec![ContentBlock::Text { text }])
            } else {
                Err(text)
            }
        })
    }
}

/// 前台结果 JSON（超上限截断并标注）。
fn render_result(report: &workflow::RunReport) -> String {
    let body = serde_json::to_string_pretty(&workflow::result_view(report))
        .unwrap_or_else(|_| report.value.to_string());
    truncate_chars(body, MAX_RESULT_CHARS)
}

/// 按字符数截断（不按字节切，避免破坏 UTF-8）。
fn truncate_chars(text: String, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text;
    }
    let mut truncated: String = text.chars().take(limit).collect();
    truncated.push_str("\n… [result truncated]");
    truncated
}

/// 后台运行：注册 job（附带取消钩子）→ spawn 执行 → 收尾 settle。
fn start_background(
    runtime: Arc<SubagentRuntime>,
    registry: Arc<JobRegistry>,
    owner: Owner,
    program: Program,
    args: Value,
    cancel: CancellationToken,
) -> Result<Vec<ContentBlock>, String> {
    let owner = current_owner(&owner)?;
    let job_id = JobId(format!(
        "workflow-{}-{}",
        crate::session::epoch_millis(),
        JOB_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    let summary = format!(
        "workflow \"{}\" ({} steps)",
        program.meta.name,
        program.steps.len()
    );
    registry.register_with_cancel(
        JobSpec { id: job_id.clone(), owner, kind: JobKind::Workflow, command: summary },
        OutputRing::new(),
        {
            let cancel = cancel.clone();
            Arc::new(move || cancel.cancel())
        },
    );
    let name = program.meta.name.clone();
    let display_name = name.clone();
    let steps = program.steps.len();
    let job = job_id.clone();
    let progress: workflow::Progress = {
        let registry = registry.clone();
        let id = job_id.0.clone();
        Arc::new(move |line: &str| registry.push_output(&id, line))
    };
    tokio::spawn(async move {
        let report = workflow::run(runtime, &program, &args, cancel, Some(progress)).await;
        let outcome = match report.stop_reason {
            StopReason::Completed => JobOutcome::Exited { code: 0 },
            StopReason::Cancelled => JobOutcome::Killed,
            StopReason::Error => JobOutcome::Failed {
                error: report
                    .error
                    .clone()
                    .unwrap_or_else(|| "workflow failed".to_string()),
            },
        };
        registry.settle(&job.0, outcome);
        // 终态摘要与最终 value 进输出环（settle 后仍可读：条目保留到 owner 清理）。
        registry.push_output(
            &job.0,
            &format!(
                "workflow \"{name}\" {}\n{}",
                report.stop_reason.as_str(),
                render_result(&report)
            ),
        );
    });
    Ok(vec![ContentBlock::Text {
        text: format!(
            "workflow \"{display_name}\" started in background as job {} ({steps} steps).\nUse \
             job_output to read progress and job_kill to stop it.",
            job_id.0
        ),
    }])
}

#[cfg(test)]
#[path = "workflow_tests.rs"]
mod tests;
