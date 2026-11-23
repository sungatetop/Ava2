//! onion-sdk-client：onion SDK 层的 JSON-RPC client。
//!
//! **不依赖引擎**：纯协议/网络库，拥有 `onion-sdk-server` 子进程，经 stdio
//! JSON-RPC 帧通信。`LiveEvent` 等类型与引擎 `onion::session` 同构但独立
//! （仅靠 serde 互通），避免引入引擎编译。
//!
//! 核心流程：spawn server 子进程 → 握手
//! `initialize` → 投递 `session/prompt`（回执式）→ 订阅 `session.event` 流
//! → 观察 turn 结束 → 需要时 `approval/reply` → EOF→SIGTERM→SIGKILL 关闭梯队。

mod rpc;

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio::sync::broadcast;

pub use rpc::{RpcError, RpcMessage};

/// 会话事件（与引擎 `onion::session::SessionEvent` 同构，独立定义）。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    SessionStarted { session_id: String, model: String },
    TurnStart { turn: u32 },
    SystemMessage { turn: u32, step: u32, message: String },
    UserMessage { turn: u32, step: u32, message: Value },
    AssistantMessage { turn: u32, step: u32, message: Value, usage: Usage },
    AssistantAttempt { turn: u32, step: u32, error: String, cancelled: bool },
    ToolCall { turn: u32, step: u32, call: Value },
    ToolResult { turn: u32, step: u32, result: Value },
    StepStart { turn: u32, step: u32 },
    StepEnd { turn: u32, step: u32 },
    TurnEnd { turn: u32, reason: String },
    SessionEnd { reason: String },
    CompactionStart { turn: u32, compaction_id: String, trigger: String },
    CompactionSummary {
        turn: u32,
        compaction_id: String,
        summary: String,
        shadowed_count: usize,
        shadowed_tokens: u64,
        usage: Usage,
    },
    CompactionEnd { turn: u32, compaction_id: String, error: Option<String> },
    CompactionPruned { pruned: u32, saved_tokens: u64 },
    HookInvoked { point: String, handler_id: String, matcher: Option<String> },
    HookResult {
        point: String,
        handler_id: String,
        decision: String,
        exit_code: Option<i32>,
        stderr_summary: Option<String>,
        duration_ms: u64,
    },
    WorkspaceInstructions { files: Vec<String>, bytes: usize, version: String },
    WorkspaceChanged { added: Vec<String>, removed: Vec<String>, changed: Vec<String> },
    GoalChange {
        operation: String,
        snapshot: Option<Value>,
        rounds_started: u32,
        created_at_ms: u64,
        updated_at_ms: u64,
    },
    PlanMode { active: bool },
    PlanWanted { active: bool },
    PlanRunDone {},
    ApprovalAudited { tool: String, decision: String, via: String },
    /// 子代理描述符（同构；log-only 元信息）。
    SubagentDescriptor {
        version: u32,
        mode: String,
        provider: String,
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        agent_model: Option<String>,
        #[serde(default)]
        agent_reasoning_effort: Option<String>,
        #[serde(default)]
        cwd: Option<String>,
        parent_session_id: String,
        depth: u32,
    },
}

/// 用量（同构）。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_read_tokens: u64,
    #[serde(default)]
    pub cache_write_tokens: u64,
}

/// live 瞬时事件（同构）。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum LiveEvent {
    StateChanged { from: String, to: String },
    Durable(SessionEvent),
    InboxInserted { partition: String, message: Value },
    InboxClaimed { partition: String, messages: Vec<Value> },
    AssistantStream(StreamChunk),
    /// subagent 生命周期开始（同构；瞬时、不落盘）。
    SubagentStart {
        run_id: String,
        agent_id: String,
        name: String,
        depth: u32,
        parent_session_id: String,
    },
    /// subagent 生命周期结束（同构；与 start 以 run_id 配对）。
    SubagentEnd {
        run_id: String,
        agent_id: String,
        name: String,
        depth: u32,
        parent_session_id: String,
        stop_reason: String,
        #[serde(default)]
        last_assistant_message: Option<String>,
    },
}

/// assistant 流式帧（同构）。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum StreamChunk {
    Start { turn: u32, step: u32 },
    Text(String),
    Thinking(String),
    ToolCallStart { index: usize, id: String, name: String },
    ToolCallInput { index: usize, partial_json: String },
    End,
}

/// 会话事件通知载荷。
#[derive(Clone, Debug, Deserialize)]
pub struct SessionEventNotification {
    pub session_id: String,
    pub event: LiveEvent,
}

/// 持有 server 子进程的 SDK client。
pub struct OnionSdkClient {
    child: Child,
    stdin: ChildStdin,
    /// stdout 读取线程 → 事件广播通道。
    event_tx: broadcast::Sender<LiveEvent>,
    /// 请求响应收集（id → oneshot）。
    pending: Arc<Mutex<std::collections::HashMap<u64, mpsc::UnboundedSender<Value>>>>,
    next_id: u64,
    session_id: String,
}

impl OnionSdkClient {
    /// spawn `onion-sdk-server` 子进程并握手 `initialize`。
    pub fn spawn(server_bin: &str) -> Result<Self, String> {
        let mut child = std::process::Command::new(server_bin)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("spawn {server_bin} 失败: {e}"))?;
        let stdin = child.stdin.take().ok_or("无法获取子进程 stdin")?;
        let stdout = child.stdout.take().ok_or("无法获取子进程 stdout")?;

        let (event_tx, _) = broadcast::channel(256);
        let pending: Arc<Mutex<std::collections::HashMap<u64, mpsc::UnboundedSender<Value>>>> =
            Arc::new(Mutex::new(std::collections::HashMap::new()));

        // stdout 读取线程：解析帧 → 通知进事件总线 / 响应回 pending。
        let reader_tx = event_tx.clone();
        let reader_pending = pending.clone();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) => {
                        let line = line.trim_end_matches(['\r', '\n']);
                        let Ok(msg) = serde_json::from_str::<RpcMessage>(line) else {
                            continue;
                        };
                        if let Some(method) = &msg.method {
                            if method == "session.event" {
                                if let Ok(notif) =
                                    serde_json::from_value::<SessionEventNotification>(msg.params.unwrap_or_default())
                                {
                                    let _ = reader_tx.send(notif.event);
                                }
                                continue;
                            }
                        }
                        if let Some(id) = msg.id {
                            let tx = reader_pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(&id);
                            if let Some(tx) = tx {
                                if let Some(error) = msg.error {
                                    // 错误响应透传（不再吞成 Null）。
                                    let _ = tx.send(json!({
                                        "__rpc_error": { "code": error.code, "message": error.message }
                                    }));
                                } else if let Some(result) = msg.result {
                                    let _ = tx.send(result);
                                }
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        let mut client = Self {
            child,
            stdin,
            event_tx,
            pending,
            next_id: 1,
            session_id: String::new(),
        };
        let init = client.call("initialize", json!({}))?;
        client.session_id = init
            .get("session_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        Ok(client)
    }

    /// 发送请求并等待响应（同步阻塞；首版简化，后续可改 async）。
    fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        let (tx, mut rx) = mpsc::unbounded_channel();
        self.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(id, tx);
        let line = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let frame = serde_json::to_string(&line).map_err(|e| e.to_string())?;
        writeln!(self.stdin, "{frame}").map_err(|e| e.to_string())?;
        self.stdin.flush().map_err(|e| e.to_string())?;
        let value = rx.blocking_recv().ok_or_else(|| "响应丢失".to_string())?;
        // 服务端错误响应 → Err(String)（透传错误码/消息，不再吞成 Null）。
        if let Some(err) = value.get("__rpc_error") {
            return Err(err.to_string());
        }
        Ok(value)
    }

    /// 投递消息（回执式）。
    pub fn prompt(&mut self, message: &str, partition: &str) -> Result<String, String> {
        let result = self.call(
            "session/prompt",
            json!({ "message": message, "partition": partition }),
        )?;
        let message_id = result
            .get("message_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        Ok(message_id)
    }

    /// 订阅事件流。
    pub fn events(&self) -> broadcast::Receiver<LiveEvent> {
        self.event_tx.subscribe()
    }

    /// 等待空闲。
    pub fn wait_idle(&mut self, timeout_ms: Option<u64>) -> Result<bool, String> {
        let result = self.call("session/wait", json!({ "timeout_ms": timeout_ms }))?;
        Ok(result.get("idle").and_then(serde_json::Value::as_bool).unwrap_or(false))
    }

    /// 审批回填。
    pub fn approve(&mut self, approve: bool) -> Result<bool, String> {
        let result = self.call("approval/reply", json!({ "approve": approve }))?;
        Ok(result.get("ok").and_then(serde_json::Value::as_bool).unwrap_or(false))
    }

    /// 切换 Agent（工具面预设）。
    pub fn set_agent(&mut self, agent: &str) -> Result<(), String> {
        self.call("session/agent", json!({ "agent": agent }))?;
        Ok(())
    }

    /// 查询当前 Agent 与可选 id。
    pub fn current_agent(&mut self) -> Result<serde_json::Value, String> {
        self.call("session/agent", json!({}))
    }

    /// 请求压缩。
    pub fn compact(&mut self) -> Result<(), String> {
        self.call("session/compact", json!({}))?;
        Ok(())
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// 优雅关闭。
    pub fn shutdown(mut self) -> Result<(), String> {
        let _ = self.call("shutdown", json!({}));
        let _ = self.child.wait();
        Ok(())
    }
}

impl Drop for OnionSdkClient {
    fn drop(&mut self) {
        // EOF → 子进程退出；等待回收避免僵尸。
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
