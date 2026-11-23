//! SDK wire 协议（NDJSON JSON-RPC 2.0 over stdio）子集。
//!
//! SDK 宿主进程接线形态：宿主进程以子进程方式拉起本进程，按行交换 JSON-RPC 帧；`session/prompt` 只返回
//! **入队回执**（不等待轮次结束），实际进展经 `session.event` 通知按落盘顺序流式回推，
//! `session/wait` 用于等待既有会话跑完。
//!
//! 已实现方法（其余 `-32601`）：
//! - `initialize` → `{ serverInfo: { name, version } }`；`cwd` 记为该连接内懒建会话的
//!   权限根（缺省用服务兜底工作区）；
//! - `session/prompt` → `{ messageId }`（未知 sessionId 懒建会话并回执入队）；
//! - `session/wait` → `{}`（等该会话当前轮次结束）；
//! - `session/working-directory/get` → `{ cwd }`；
//! - `shutdown` → `{}` 并结束进程循环。
//!
//! 通知（服务端 → 客户端）：`session.event`（`{sessionId, event}`，事件本体与 REST
//! `/v1/sessions/<id>/events` 同源）与 `session.status`（`{sessionId, status}`）。
//!
//! 诚实降级：`provider` / `model` / `reasoningEffort` / `maxTokens` 入参被接受但
//! 不改变本进程路由（路由由服务端 Config 决定）；`session/query`（斜杠命令）、
//! `session/resume`、`session/working-directory/set`、审批请求回推
//! （`user-question/ask`）与子代理生命周期通知未实现。

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use onion::types::AgentMessage;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, Mutex};

use crate::session::{spawn_turn, ServerSession, SessionRegistry};

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

/// 事件泵轮询步长（durable 快照增量扫描）。
const POLL_INTERVAL: Duration = Duration::from_millis(100);

static MESSAGE_SEQ: AtomicU64 = AtomicU64::new(0);

/// 一个 SDK 会话的连接级句柄。
struct SdkSession {
    session: Arc<ServerSession>,
    /// 事件泵游标（已回推的事件条数）。
    seen: usize,
    /// 上次回推的 `session.status`（None = 尚未回推）。
    status: Option<&'static str>,
}

/// 连接级共享状态。
#[derive(Clone)]
struct SdkState {
    registry: SessionRegistry,
    sessions: Arc<Mutex<HashMap<String, SdkSession>>>,
    /// `initialize` 的 `cwd`（本连接懒建会话的权限根）。
    cwd: Arc<Mutex<Option<String>>>,
    /// 事件泵句柄（连接结束时统一 abort）。
    pumps: Arc<Mutex<Vec<tokio::task::AbortHandle>>>,
    tx: mpsc::Sender<Value>,
}

/// 以本进程 stdin/stdout 提供 SDK wire 服务（客户端关闭 stdin 即结束）。
pub(crate) async fn serve(registry: SessionRegistry) -> std::io::Result<()> {
    serve_io(registry, tokio::io::stdin(), tokio::io::stdout()).await
}

/// 任意字节流上的 SDK wire 服务（测试与嵌入宿主共用）。
pub(crate) async fn serve_io<R, W>(
    registry: SessionRegistry,
    reader: R,
    writer: W,
) -> std::io::Result<()>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let (tx, rx) = mpsc::channel::<Value>(256);
    let writer_task = tokio::spawn(async move {
        let mut writer = tokio::io::BufWriter::new(writer);
        let mut rx = rx;
        while let Some(message) = rx.recv().await {
            let Ok(line) = serde_json::to_string(&message) else {
                continue;
            };
            if writer.write_all(line.as_bytes()).await.is_err()
                || writer.write_all(b"\n").await.is_err()
                || writer.flush().await.is_err()
            {
                break;
            }
        }
    });
    let state = SdkState {
        registry,
        sessions: Arc::new(Mutex::new(HashMap::new())),
        cwd: Arc::new(Mutex::new(None)),
        pumps: Arc::new(Mutex::new(Vec::new())),
        tx: tx.clone(),
    };
    let mut handlers = tokio::task::JoinSet::new();
    let mut lines = BufReader::new(reader).lines();
    loop {
        let line = match lines.next_line().await {
            Ok(Some(line)) => line,
            Ok(None) => break,
            Err(error) => {
                eprintln!("[onion-server] sdk stdin 读取失败: {error}");
                break;
            }
        };
        // Windows 客户端可能把 BOM 写进首行（PowerShell 重定向常见），先剥掉。
        let line = line.trim_start_matches('\u{feff}').trim().to_string();
        if line.is_empty() {
            continue;
        }
        let message: Value = match serde_json::from_str(&line) {
            Ok(message) => message,
            Err(error) => {
                let _ = state
                    .tx
                    .send(error_response(Value::Null, PARSE_ERROR, &format!("invalid JSON: {error}")))
                    .await;
                continue;
            }
        };
        let Some(method) = message.get("method").and_then(Value::as_str).map(str::to_string) else {
            let id = message.get("id").cloned().unwrap_or(Value::Null);
            let _ = state
                .tx
                .send(error_response(id, INVALID_REQUEST, "missing `method`"))
                .await;
            continue;
        };
        let id = message.get("id").cloned();
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        let Some(id) = id else {
            // 客户端通知：本子集无入站通知语义，忽略。
            continue;
        };
        let state = state.clone();
        if method == "shutdown" {
            let _ = state.tx.send(success_response(id, json!({}))).await;
            break;
        }
        handlers.spawn(async move {
            let response = handle(&state, &method, &params, id.clone()).await;
            let _ = state.tx.send(response).await;
        });
    }
    handlers.shutdown().await;
    for pump in state.pumps.lock().await.drain(..) {
        pump.abort();
    }
    // `SdkState` 自身持有一个 tx 克隆：不释放则写任务的 recv 永不结束。
    drop(state);
    drop(tx);
    let _ = writer_task.await;
    Ok(())
}

/// 单个请求分发（错误一律 JSON-RPC error 帧，不 panic）。
async fn handle(state: &SdkState, method: &str, params: &Value, id: Value) -> Value {
    match method {
        "initialize" => {
            if let Some(cwd) = params.get("cwd").and_then(Value::as_str).filter(|cwd| !cwd.trim().is_empty()) {
                *state.cwd.lock().await = Some(cwd.trim().to_string());
            }
            success_response(id, initialize_result())
        }
        "session/prompt" => match enqueue_prompt(state, params).await {
            Ok(message_id) => success_response(id, json!({ "messageId": message_id })),
            Err(error) => error_response(id, INVALID_PARAMS, &error),
        },
        "session/wait" => match resolve(state, params).await {
            Ok(session) => {
                while session.handle.gate.is_busy() {
                    tokio::time::sleep(POLL_INTERVAL).await;
                }
                success_response(id, json!({}))
            }
            Err(error) => error_response(id, INVALID_PARAMS, &error),
        },
        "session/working-directory/get" => match resolve(state, params).await {
            Ok(session) => success_response(id, json!({ "cwd": session.root.display().to_string() })),
            Err(error) => error_response(id, INVALID_PARAMS, &error),
        },
        other => error_response(
            id,
            METHOD_NOT_FOUND,
            &format!("unsupported SDK method `{other}`"),
        ),
    }
}

/// `initialize` 结果（wire-stable 服务身份 + 版本）。
pub(crate) fn initialize_result() -> Value {
    json!({
        "serverInfo": {
            "name": "onion-server",
            "version": env!("CARGO_PKG_VERSION")
        }
    })
}

/// `session/prompt`：懒建会话 → 入队用户消息 → 起事件泵 → 返回回执。
async fn enqueue_prompt(state: &SdkState, params: &Value) -> Result<String, String> {
    let Some(session_id) = params.get("sessionId").and_then(Value::as_str) else {
        return Err("`sessionId` is required".to_string());
    };
    let text = content_blocks_text(params)?;
    let session = session_for(state, session_id).await?;
    session.handle.gate.enqueue();
    session
        .handle
        .runtime
        .inbox()
        .followup(AgentMessage::user(text))
        .map_err(|error| format!("inbox 投递失败: {error}"))?;
    spawn_turn(&session.handle);
    let message_id = format!(
        "msg-{}-{}",
        onion::session::epoch_millis(),
        MESSAGE_SEQ.fetch_add(1, Ordering::SeqCst)
    );
    start_pump(state, session_id).await;
    // 回执先于事件：客户端拿到 messageId 后按 session.event 流消费。
    Ok(message_id)
}

/// 取既有会话；未知 sessionId 按 `initialize` 的 cwd 懒建。
async fn session_for(state: &SdkState, session_id: &str) -> Result<Arc<ServerSession>, String> {
    if let Some(entry) = state.sessions.lock().await.get(session_id) {
        return Ok(entry.session.clone());
    }
    let cwd = state.cwd.lock().await.clone();
    // SDK 无审批通道：会话按非交互装配（与 ACP/A2A 一致）。
    let session = state
        .registry
        .create(Some(session_id.to_string()), None, false, cwd)
        .await
        .map_err(|error| format!("create session failed: {error}"))?;
    state.sessions.lock().await.insert(
        session_id.to_string(),
        SdkSession { session: session.clone(), seen: 0, status: None },
    );
    Ok(session)
}

/// `session/wait` / `working-directory` 的会话定位（不懒建，未知即报错）。
async fn resolve(state: &SdkState, params: &Value) -> Result<Arc<ServerSession>, String> {
    let Some(session_id) = params.get("sessionId").and_then(Value::as_str) else {
        return Err("`sessionId` is required".to_string());
    };
    state
        .sessions
        .lock()
        .await
        .get(session_id)
        .map(|entry| entry.session.clone())
        .ok_or_else(|| format!("unknown session `{session_id}`"))
}

/// 起事件泵（每会话一个；已在跑则不重复起）。
async fn start_pump(state: &SdkState, session_id: &str) {
    let already = state
        .sessions
        .lock()
        .await
        .get(session_id)
        .is_some_and(|entry| entry.status.is_some());
    if already {
        return;
    }
    if let Some(entry) = state.sessions.lock().await.get_mut(session_id) {
        entry.status = Some("running");
    } else {
        return;
    }
    let state = state.clone();
    let session_id = session_id.to_string();
    let _ = state
        .tx
        .send(notification(
            "session.status",
            json!({ "sessionId": session_id, "status": "running" }),
        ))
        .await;
    let pump = (state.clone(), session_id.clone());
    let handle = tokio::spawn(async move {
        let (state, session_id) = pump;
        loop {
            let Some((frames, status_change)) = pump_once(&state, &session_id).await else {
                return;
            };
            for frame in frames {
                if state.tx.send(frame).await.is_err() {
                    return;
                }
            }
            if let Some(status) = status_change {
                let frame = notification(
                    "session.status",
                    json!({ "sessionId": session_id, "status": status }),
                );
                if state.tx.send(frame).await.is_err() {
                    return;
                }
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    });
    state.pumps.lock().await.push(handle.abort_handle());
}

/// 单次泵步：采集新事件帧 + 状态翻转（None = 会话已不在表内）。
///
/// 锁内只做同步快照读取与游标推进，发送一律在锁外完成（避免持锁跨 await）。
async fn pump_once(
    state: &SdkState,
    session_id: &str,
) -> Option<(Vec<Value>, Option<&'static str>)> {
    let mut sessions = state.sessions.lock().await;
    let entry = sessions.get_mut(session_id)?;
    let events = entry.session.handle.runtime.session().snapshot();
    let idle = !entry.session.handle.gate.is_busy();
    let mut frames = Vec::new();
    for event in events.iter().skip(entry.seen) {
        let Ok(value) = serde_json::to_value(event) else {
            continue;
        };
        frames.push(notification(
            "session.event",
            json!({ "sessionId": session_id, "event": value }),
        ));
    }
    entry.seen = events.len();
    let next_status = if idle { "idle" } else { "running" };
    let status_change = if entry.status == Some(next_status) {
        None
    } else {
        entry.status = Some(next_status);
        Some(next_status)
    };
    Some((frames, status_change))
}

/// 从 `contentBlocks` 提取用户文本（图片等非文本块跳过；全为空即报错）。
pub(crate) fn content_blocks_text(params: &Value) -> Result<String, String> {
    let Some(blocks) = params.get("contentBlocks").and_then(Value::as_array) else {
        return Err("`contentBlocks` must be an array".to_string());
    };
    let mut parts: Vec<&str> = Vec::new();
    for block in blocks {
        if let Some("text") = block.get("type").and_then(Value::as_str) {
            if let Some(text) = block.get("text").and_then(Value::as_str) {
                if !text.trim().is_empty() {
                    parts.push(text);
                }
            }
        }
    }
    if parts.is_empty() {
        return Err("`contentBlocks` contains no text content".to_string());
    }
    Ok(parts.join("\n"))
}

/// JSON-RPC 通知帧。
pub(crate) fn notification(method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "method": method, "params": params })
}

/// JSON-RPC 成功响应。
pub(crate) fn success_response(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// JSON-RPC 错误响应。
pub(crate) fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
}

#[cfg(test)]
#[path = "sdk_wire_tests.rs"]
mod sdk_wire_tests;
