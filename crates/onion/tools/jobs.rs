//! jobs 控制面工具：`job_list` / `job_output` / `job_kill`。
//!
//! kind 无关薄壳——只读取 `JobRegistry`（`onion-jobs` 独立 crate），不持有执行器。
//! 后台进程的 spawn / kill 归 `shell_exec::background::BackgroundExecutor`；本模块
//! 只负责把注册表状态与输出环呈现给模型。
//!
//! owner 锚定：任务归属调用会话，会话 id 在 Runtime 装配后才有（工具构造早于
//! 装配），故经 [`Owner`]（`Arc<OnceLock<SessionId>>`）延迟接线（对齐 subagent
//! `wire_parent` 模式）。未接线时 `job_list` 报错（fail-closed）。
//!
//! 风险与模式：`job_list` / `job_output` 只读（`ReadOnly`）；`job_kill` 终止进程
//! 属 `Escalating`。均在 `work` 模式下可见（与 shell 工具一致）。

use std::sync::{Arc, OnceLock};

use onion_jobs::{JobRegistry, JobStatus, JobView, KillResult, SessionId};
use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::ToolPort;
use crate::approval::ToolRisk;
use crate::types::{ContentBlock, ToolCall, ToolDefinition};
use crate::waterfall::BoxFuture;

/// 会话 owner 延迟接线单元（工具间共享同一 cell，装配后 wire 一次）。
pub type Owner = Arc<OnceLock<SessionId>>;

/// 创建空的 owner cell（装配期构造，Runtime 装配后 `wire_owner` 接线）。
pub fn owner_cell() -> Owner {
    Arc::new(OnceLock::new())
}

/// 接线 owner cell（宿主装配完成后调用；跨会话重建时重建新 cell）。
pub fn wire_owner(owner: &Owner, session_id: impl Into<String>) {
    let _ = owner.set(SessionId(session_id.into()));
}

/// 从 owner cell 取当前会话；未接线返回 Err（fail-closed）。
pub(crate) fn current_owner(owner: &Owner) -> Result<SessionId, String> {
    owner
        .get()
        .cloned()
        .ok_or_else(|| "session owner not wired (jobs tools require a session)".to_string())
}

/// job_list：列出当前会话的全部任务（id 排序，含状态与命令摘要）。
pub struct JobListTool {
    pub registry: Arc<JobRegistry>,
    pub owner: Owner,
}

/// job_output：读取任务输出尾窗（尾部 `limit` 行）。
pub struct JobOutputTool {
    pub registry: Arc<JobRegistry>,
    pub owner: Owner,
}

/// job_kill：终止在飞任务（first-wins 标 Killed + 取消进程）。
pub struct JobKillTool {
    pub registry: Arc<JobRegistry>,
    pub owner: Owner,
}

impl JobListTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "job_list".to_string(),
            description: "List background jobs for the current session, with status and command summary.".to_string(),
            parameters: json!({ "type": "object", "properties": {} }),
        }
    }
}

impl JobOutputTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "job_output".to_string(),
            description: "Read the tail of a background job's output, optionally waiting for it to \
                finish first. Input: {\"job_id\": string, \"wait\"?: boolean, \"timeout_ms\"?: number}."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "job_id": { "type": "string", "description": "Job id returned by a background shell call." },
                    "wait": { "type": "boolean", "description": "Wait for the job to settle before returning (default false)." },
                    "timeout_ms": { "type": "number", "description": "Max ms to wait when wait is true (default 30000, capped at 600000)." }
                },
                "required": ["job_id"]
            }),
        }
    }
}

/// `job_output` 等待任务终态时单次轮询间隔。
const WAIT_POLL_MS: u64 = 50;

impl JobKillTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "job_kill".to_string(),
            description: "Stop a running background job. Input: {\"job_id\": string, \"reason\"?: string}.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "job_id": { "type": "string", "description": "Job id to stop." },
                    "reason": { "type": "string", "description": "Optional reason for the stop (logged and forwarded)." }
                },
                "required": ["job_id"]
            }),
        }
    }
}

/// 任务状态渲染为模型可见文本。
fn status_text(status: &JobStatus) -> String {
    match status {
        JobStatus::Running => "running".to_string(),
        JobStatus::Settled(outcome) => match outcome {
            onion_jobs::JobOutcome::Exited { code } => format!("exited ({code})"),
            onion_jobs::JobOutcome::Killed => "killed".to_string(),
            onion_jobs::JobOutcome::Failed { error } => format!("failed: {error}"),
        },
    }
}

impl ToolPort for JobListTool {
    fn definition(&self) -> ToolDefinition {
        self.definition()
    }

    fn modes(&self) -> Option<Vec<String>> {
        Some(vec!["work".to_string()])
    }

    fn risk(&self) -> ToolRisk {
        ToolRisk::ReadOnly
    }

    fn execute(&self, _call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let registry = self.registry.clone();
        let owner = self.owner.clone();
        Box::pin(async move {
            let owner = current_owner(&owner)?;
            let views: Vec<JobView> = registry.list(&owner);
            if views.is_empty() {
                return Ok(vec![ContentBlock::Text { text: "No background jobs.".to_string() }]);
            }
            let mut lines = Vec::new();
            for view in views {
                lines.push(format!(
                    "{} · {} · {}",
                    view.spec.id.0,
                    status_text(&view.status),
                    view.spec.command
                ));
            }
            Ok(vec![ContentBlock::Text { text: lines.join("\n") }])
        })
    }
}

impl ToolPort for JobOutputTool {
    fn definition(&self) -> ToolDefinition {
        self.definition()
    }

    fn modes(&self) -> Option<Vec<String>> {
        Some(vec!["work".to_string()])
    }

    fn risk(&self) -> ToolRisk {
        ToolRisk::ReadOnly
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let registry = self.registry.clone();
        let input = call.input;
        Box::pin(async move {
            let job_id = input
                .get("job_id")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "job_id is required".to_string())?;
            let wait = input.get("wait").and_then(serde_json::Value::as_bool).unwrap_or(false);
            let timeout_ms = input
                .get("timeout_ms")
                .and_then(serde_json::Value::as_u64)
                .map(|n| n.min(600_000))
                .unwrap_or(30_000);
            if wait {
                // `wait`：前台等待任务终态（轮询注册表），逾限返回当前尾窗。
                let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
                loop {
                    if registry.get(job_id).map(|v| !v.status.is_running()).unwrap_or(true) {
                        break;
                    }
                    if tokio::time::Instant::now() >= deadline {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(WAIT_POLL_MS)).await;
                }
            }
            let Some(view) = registry.get(job_id) else {
                return Err(format!("job not found: {job_id}"));
            };
            let output = registry.read_output(job_id, 200).unwrap_or_default();
            let header = format!("{} · {}", job_id, status_text(&view.status));
            let text = if output.is_empty() {
                format!("{header} · (no output)")
            } else {
                format!("{header}\n{output}")
            };
            Ok(vec![ContentBlock::Text { text }])
        })
    }
}

impl ToolPort for JobKillTool {
    fn definition(&self) -> ToolDefinition {
        self.definition()
    }

    fn modes(&self) -> Option<Vec<String>> {
        Some(vec!["work".to_string()])
    }

    fn risk(&self) -> ToolRisk {
        ToolRisk::Escalating
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let registry = self.registry.clone();
        let input = call.input;
        Box::pin(async move {
            let job_id = input
                .get("job_id")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "job_id is required".to_string())?;
            let reason = input.get("reason").and_then(serde_json::Value::as_str).map(str::to_string);
            let result = registry.kill(job_id);
            let text = match result {
                KillResult::Requested => match reason {
                    Some(r) => format!("kill requested for {job_id} (reason: {r})"),
                    None => format!("kill requested for {job_id}"),
                },
                KillResult::NotFound => format!("job not found: {job_id}"),
                KillResult::AlreadySettled => format!("job already finished: {job_id}"),
            };
            Ok(vec![ContentBlock::Text { text }])
        })
    }
}

/// 构造三个 jobs 工具（共享 registry 与 owner cell）。
pub fn jobs_tools(registry: Arc<JobRegistry>, owner: Owner) -> (JobListTool, JobOutputTool, JobKillTool) {
    (
        JobListTool { registry: registry.clone(), owner: owner.clone() },
        JobOutputTool { registry: registry.clone(), owner: owner.clone() },
        JobKillTool { registry, owner },
    )
}
