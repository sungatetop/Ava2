//! `terminal_*` 6 件工具：open / send / read /
//! signal / close / list。薄壳：schema 校验 + seam 调用 + 渲染。
//!
//! 核心语义：
//! - owner 隔离：会话只对发起它的 agent 可见（`FOREIGN_SESSION` 拒绝）；
//! - waitReason 四值透出（stdin_read / inferred_idle / timeout / session_exit）；
//! - signal 白名单（SIGINT/SIGTERM/SIGKILL/SIGTSTP/SIGHUP）；
//! - `terminal_send` 支持 `run_in_background`（后台化复用 onion-jobs，S14b 接线）；
//! - 写/交互操作（open/send/signal/close）为 `Escalating`；工具可见性由当前 Agent
//!   工具面决定（coding 预设包含，work 预设不含）；
//!   只读操作（read/list）为 `ReadOnly`。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use serde_json::json;
use tokio_util::sync::CancellationToken;

use crate::approval::ToolRisk;
use crate::tools::ToolPort;
use crate::types::{ContentBlock, ToolCall, ToolDefinition};
use crate::waterfall::BoxFuture;

use super::render::{
    format_close, format_list, format_open, format_read, format_send, format_signal,
    DEFAULT_MAX_RESULT_CHARS,
};
use super::seam::{PtySignal, TerminalService, TerminalSessionId, TerminalStatus, WaitReason};

/// 稳定 system-prompt 指引：仅在需要持久终端状态或交互
/// stdin 时用 terminal；优先 shell/read/write/edit 做一次性操作。
pub const TERMINAL_PROMPT_TEXT: &str =
    "Use terminal_* only when you need a persistent terminal session or interactive stdin. \
Prefer bash/pwsh and read/write/edit for one-shot operations. Track each terminal session id \
and close sessions you no longer need. terminal_send foreground blocks until the command \
finishes; signals are limited to SIGINT/SIGTERM/SIGKILL/SIGTSTP/SIGHUP.";

/// owner 接线槽（会话 id；装配后 wire）。
fn owner_slot() -> &'static OnceLock<onion_jobs::SessionId> {
    static OWNER: OnceLock<onion_jobs::SessionId> = OnceLock::new();
    &OWNER
}

/// `terminal_*` 工具集：共享 [`TerminalService`] + owner 槽 + 后台注册表槽。
#[derive(Clone)]
pub struct TerminalToolSet {
    service: Arc<TerminalService>,
    owner: Arc<OnceLock<onion_jobs::SessionId>>,
    background: Arc<OnceLock<Arc<onion_jobs::JobRegistry>>>,
    enable_background: bool,
    max_result_chars: usize,
}

impl TerminalToolSet {
    pub fn new(service: Arc<TerminalService>) -> Self {
        Self {
            service,
            owner: Arc::new(OnceLock::new()),
            background: Arc::new(OnceLock::new()),
            enable_background: true,
            max_result_chars: DEFAULT_MAX_RESULT_CHARS,
        }
    }

    /// 延迟接线 owner（宿主装配完成后调用）。
    pub fn wire_owner(&self, owner: impl Into<String>) {
        let _ = self.owner.set(onion_jobs::SessionId(owner.into()));
    }

    /// 延迟接线后台注册表（`terminal_send run_in_background` 用；宿主装配后调用）。
    pub fn wire_background(&self, registry: Arc<onion_jobs::JobRegistry>) {
        let _ = self.background.set(registry);
    }

    /// 配置是否允许 `terminal_send` 后台化（对齐 `ONION_TERMINAL_ENABLE_BACKGROUND`）。
    pub fn with_background_enabled(mut self, enabled: bool) -> Self {
        self.enable_background = enabled;
        self
    }

    fn owner(&self) -> Result<onion_jobs::SessionId, String> {
        self.owner
            .get()
            .cloned()
            .ok_or_else(|| "terminal_* requires an owning agent session".to_string())
    }

    fn service(&self) -> Arc<TerminalService> {
        self.service.clone()
    }

    /// 注册 6 工具进注册表。
    pub fn register_all(&self, registry: &mut crate::tools::ToolRegistry) {
        let shared = self.clone();
        registry.register(Arc::new(TerminalOpenTool::new(shared.clone())));
        registry.register(Arc::new(TerminalSendTool::new(shared.clone())));
        registry.register(Arc::new(TerminalReadTool::new(shared.clone())));
        registry.register(Arc::new(TerminalSignalTool::new(shared.clone())));
        registry.register(Arc::new(TerminalCloseTool::new(shared.clone())));
        registry.register(Arc::new(TerminalListTool::new(shared)));
    }

    /// `terminal_send run_in_background`：注册 PtySend 任务，后台派发 send，
    /// 把会话输出吸收进任务输出环（`job_output` 可读），完成后 settle。
    fn spawn_background_send(
        &self,
        owner: onion_jobs::SessionId,
        id: TerminalSessionId,
        text: &str,
        submit: bool,
    ) -> Result<Vec<ContentBlock>, String> {
        if !self.enable_background {
            return Err("run_in_background is disabled (ONION_TERMINAL_ENABLE_BACKGROUND=false)".to_string());
        }
        let registry = self
            .background
            .get()
            .cloned()
            .ok_or_else(|| "terminal background registry not wired".to_string())?;
        let job_id = mint_job_id();
        let job_spec = onion_jobs::JobSpec {
            id: job_id.clone(),
            owner: owner.clone(),
            kind: onion_jobs::JobKind::PtySend,
            command: text.to_string(),
        };
        registry.register(job_spec, onion_jobs::OutputRing::new());
        let service = self.service.clone();
        let text = text.to_string();
        let id2 = id;
        let job_id2 = job_id.clone();
        let registry_outer = registry;
        tokio::spawn(async move {
            let registry_inner = registry_outer.clone();
            let job_id_inner = job_id2.clone();
            let outcome = tokio::task::spawn_blocking(move || -> Result<(), String> {
                service.start_send(&owner, &id2, &text, submit)
                    .map_err(|e| format!("{} (code={:?})", e.message, e.code))?;
                // 后台：反复吸收输出进任务输出环，直到命令结束（简化：一轮 + inferred_idle）。
                let lines = service.read(&owner, &id2, 0, usize::MAX)
                    .map_err(|e| format!("{} (code={:?})", e.message, e.code))?;
                service.finish_send(&id2);
                let viewport = lines.join("\n");
                if !viewport.is_empty() {
                    registry_inner.push_output(&job_id_inner.0, &viewport);
                }
                Ok(())
            })
            .await
            .unwrap_or_else(|e| Err(format!("background send task panicked: {e}")));
            match outcome {
                Ok(()) => registry_outer.settle(&job_id2.0, onion_jobs::JobOutcome::Exited { code: 0 }),
                Err(error) => registry_outer.settle(&job_id2.0, onion_jobs::JobOutcome::Failed { error }),
            }
        });
        Ok(vec![ContentBlock::Text {
            text: format!("started background job {}", job_id.0),
        }])
    }
}

/// 校验 `sessionId` 参数。
fn session_param(call: &ToolCall) -> Result<TerminalSessionId, String> {
    let id = call
        .input
        .get("sessionId")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "sessionId is required".to_string())?;
    if id.trim().is_empty() {
        return Err("sessionId must be a non-empty string".to_string());
    }
    Ok(TerminalSessionId(id.to_string()))
}

/// 后台任务 id 分配器（进程级单调递增，避免同毫秒碰撞；与 shell 后台任务同前缀）。
static TERMINAL_JOB_SEQ: AtomicU64 = AtomicU64::new(0);

/// 铸造一个 pty_send 后台任务 id。
fn mint_job_id() -> onion_jobs::JobId {
    onion_jobs::JobId(format!(
        "job-{}-{}",
        crate::session::epoch_millis(),
        TERMINAL_JOB_SEQ.fetch_add(1, Ordering::SeqCst)
    ))
}

/// `terminal_open`：启动持久终端会话。
pub struct TerminalOpenTool {
    shared: TerminalToolSet,
}

impl TerminalOpenTool {
    pub fn new(shared: TerminalToolSet) -> Self {
        Self { shared }
    }
}

impl ToolPort for TerminalOpenTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "terminal_open".to_string(),
            description: "Start a persistent terminal session. type is required (e.g. \"shell\"). \
                Returns the session id and a banner; capture the id for later sends/reads."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "type": { "type": "string", "description": "Backend type, e.g. \"shell\"." },
                    "name": { "type": "string", "description": "Optional unique session name." },
                    "cwd": { "type": "string", "description": "Optional working directory for the session." }
                },
                "required": ["type"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let service = self.shared.service();
        let owner = self.shared.owner();
        Box::pin(async move {
            let owner = owner?;
            let type_ = call
                .input
                .get("type")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "type is required".to_string())?;
            let name = call.input.get("name").and_then(serde_json::Value::as_str);
            let cwd = call.input.get("cwd").and_then(serde_json::Value::as_str);
            let id = service.spawn(owner.clone(), type_, name, cwd).map_err(|e| format!("{} (code={:?})", e.message, e.code))?;
            let views = service.list(&owner);
            let view = views.iter().find(|v| v.id == id).ok_or_else(|| "spawned session not found".to_string())?;
            let text = format_open(&id.0, &view.name, &view.type_, &view.motd);
            Ok(vec![ContentBlock::Text { text }])
        })
    }

    fn modes(&self) -> Option<Vec<String>> {
        Some(vec!["work".to_string()])
    }

    fn risk(&self) -> ToolRisk {
        ToolRisk::Escalating
    }
}

/// `terminal_send`：向会话写输入并等待（或后台化）。
pub struct TerminalSendTool {
    shared: TerminalToolSet,
}

impl TerminalSendTool {
    pub fn new(shared: TerminalToolSet) -> Self {
        Self { shared }
    }
}

impl ToolPort for TerminalSendTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "terminal_send".to_string(),
            description: "Write input to a terminal session. Foreground by default: blocks until \
                the command finishes and reports the wait reason. Set run_in_background to return \
                a job id instead."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "sessionId": { "type": "string", "description": "The terminal session id." },
                    "text": { "type": "string", "description": "Input to write." },
                    "submit": { "type": "boolean", "description": "Append a newline to submit. Default true." },
                    "run_in_background": { "type": "boolean", "description": "Run as a background job, returning a job id. Default false." }
                },
                "required": ["sessionId", "text"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let shared = self.shared.clone();
        Box::pin(async move {
            let owner = shared.owner()?;
            let id = session_param(&call)?;
            let text = call
                .input
                .get("text")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "text is required".to_string())?;
            let submit = call.input.get("submit").and_then(serde_json::Value::as_bool).unwrap_or(true);
            let background = call.input.get("run_in_background").and_then(serde_json::Value::as_bool).unwrap_or(false);
            let service = shared.service();
            if background {
                return shared.spawn_background_send(owner, id, text, submit);
            }
            service.start_send(&owner, &id, text, submit).map_err(|e| format!("{} (code={:?})", e.message, e.code))?;
            // 前台：吸收输出并渲染（S14b 接 wait 判定；此处以 inferred_idle 代理）。
            let lines = service.read(&owner, &id, 0, usize::MAX).map_err(|e| format!("{} (code={:?})", e.message, e.code))?;
            service.finish_send(&id);
            let viewport = lines.join("\n");
            let text = format_send(&viewport, WaitReason::InferredIdle, &TerminalStatus::Running, DEFAULT_MAX_RESULT_CHARS);
            Ok(vec![ContentBlock::Text { text }])
        })
    }

    fn modes(&self) -> Option<Vec<String>> {
        Some(vec!["work".to_string()])
    }

    fn risk(&self) -> ToolRisk {
        ToolRisk::Escalating
    }
}

/// `terminal_read`：分页读取会话输出。
pub struct TerminalReadTool {
    shared: TerminalToolSet,
}

impl TerminalReadTool {
    pub fn new(shared: TerminalToolSet) -> Self {
        Self { shared }
    }
}

impl ToolPort for TerminalReadTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "terminal_read".to_string(),
            description: "Read output from a terminal session, paginated by line offset/count."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "sessionId": { "type": "string", "description": "The terminal session id." },
                    "offset": { "type": "integer", "description": "Line offset into the buffer. Default 0." },
                    "count": { "type": "integer", "description": "Max lines to read. Default 500." }
                },
                "required": ["sessionId"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let service = self.shared.service();
        let owner = self.shared.owner();
        Box::pin(async move {
            let owner = owner?;
            let id = session_param(&call)?;
            let offset = call.input.get("offset").and_then(serde_json::Value::as_u64).unwrap_or(0) as usize;
            let count = call.input.get("count").and_then(serde_json::Value::as_u64).unwrap_or(500) as usize;
            let lines = service.read(&owner, &id, offset, count).map_err(|e| format!("{} (code={:?})", e.message, e.code))?;
            let text = format_read(&lines, offset, offset + lines.len(), DEFAULT_MAX_RESULT_CHARS);
            Ok(vec![ContentBlock::Text { text }])
        })
    }

    fn risk(&self) -> ToolRisk {
        ToolRisk::ReadOnly
    }
}

/// `terminal_signal`：向会话前台进程组送信号。
pub struct TerminalSignalTool {
    shared: TerminalToolSet,
}

impl TerminalSignalTool {
    pub fn new(shared: TerminalToolSet) -> Self {
        Self { shared }
    }
}

impl ToolPort for TerminalSignalTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "terminal_signal".to_string(),
            description: "Deliver a signal to a terminal session's foreground process group. \
                Allowed: SIGINT, SIGTERM, SIGKILL, SIGTSTP, SIGHUP."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "sessionId": { "type": "string", "description": "The terminal session id." },
                    "signal": { "type": "string", "enum": ["SIGINT", "SIGTERM", "SIGKILL", "SIGTSTP", "SIGHUP"], "description": "Signal to deliver." }
                },
                "required": ["sessionId", "signal"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let service = self.shared.service();
        let owner = self.shared.owner();
        Box::pin(async move {
            let owner = owner?;
            let id = session_param(&call)?;
            let signal = call
                .input
                .get("signal")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "signal is required".to_string())?;
            let sig = PtySignal::parse(signal)
                .ok_or_else(|| "signal must be one of SIGINT, SIGTERM, SIGKILL, SIGTSTP, SIGHUP".to_string())?;
            service.signal(&owner, &id, sig).map_err(|e| format!("{} (code={:?})", e.message, e.code))?;
            let text = format_signal(sig.as_str(), "session");
            Ok(vec![ContentBlock::Text { text }])
        })
    }

    fn modes(&self) -> Option<Vec<String>> {
        Some(vec!["work".to_string()])
    }

    fn risk(&self) -> ToolRisk {
        ToolRisk::Escalating
    }
}

/// `terminal_close`：关闭会话。
pub struct TerminalCloseTool {
    shared: TerminalToolSet,
}

impl TerminalCloseTool {
    pub fn new(shared: TerminalToolSet) -> Self {
        Self { shared }
    }
}

impl ToolPort for TerminalCloseTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "terminal_close".to_string(),
            description: "Close a terminal session, killing its process tree."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "sessionId": { "type": "string", "description": "The terminal session id." }
                },
                "required": ["sessionId"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let service = self.shared.service();
        let owner = self.shared.owner();
        Box::pin(async move {
            let owner = owner?;
            let id = session_param(&call)?;
            service.kill(&owner, &id).map_err(|e| format!("{} (code={:?})", e.message, e.code))?;
            let text = format_close(&id.0);
            Ok(vec![ContentBlock::Text { text }])
        })
    }

    fn modes(&self) -> Option<Vec<String>> {
        Some(vec!["work".to_string()])
    }

    fn risk(&self) -> ToolRisk {
        ToolRisk::Escalating
    }
}

/// `terminal_list`：列出 owner 的会话。
pub struct TerminalListTool {
    shared: TerminalToolSet,
}

impl TerminalListTool {
    pub fn new(shared: TerminalToolSet) -> Self {
        Self { shared }
    }
}

impl ToolPort for TerminalListTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "terminal_list".to_string(),
            description: "List the terminal sessions owned by this agent."
                .to_string(),
            parameters: json!({ "type": "object", "properties": {} }),
        }
    }

    fn execute(&self, _call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let service = self.shared.service();
        let owner = self.shared.owner();
        Box::pin(async move {
            let owner = owner?;
            let views = service.list(&owner);
            let text = format_list(&views, DEFAULT_MAX_RESULT_CHARS);
            Ok(vec![ContentBlock::Text { text }])
        })
    }

    fn risk(&self) -> ToolRisk {
        ToolRisk::ReadOnly
    }
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
