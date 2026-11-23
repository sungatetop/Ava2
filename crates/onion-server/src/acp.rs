//! ACP（Agent Client Protocol）stdio JSON-RPC 子集。
//!
//! ACP 宿主接线（`@zed-industries/agent-client-protocol`）的教学子集：编辑器把
//! 本进程作为 agent 子进程拉起，经 stdin/stdout 交换
//! **NDJSON 帧**的 JSON-RPC 2.0 消息，不起 HTTP 监听。
//!
//! 已实现方法（其余一律 `-32601 method not found`，便于客户端显式识别能力边界）：
//! - `initialize` → 协议版本 + agent 能力（`loadSession` 支持）；
//! - `authenticate` → 空结果（本实现不设鉴权方法，`authMethods` 为空）；
//! - `session/new` → 创建/复用 live 会话（`cwd` 为权限根，ACP 要求绝对路径）；
//! - `session/prompt` → 驱动一轮，期间以 `session/update` 通知流式回推
//!   `agent_message_chunk` / `agent_thought_chunk` / `tool_call` / `tool_call_update`，
//!   轮次结束后返回 `{ stopReason: end_turn | cancelled | max_tokens }`；
//! - `session/cancel`（通知）→ 取消在飞轮次（Runtime 取消令牌，与 REST/A2A 同一入口）。
//!
//! 未实现的能力（诚实降级）：不实现 `session/load`（历史会话经 `session/new` 的
//! 会话表或 REST 打开后仍可用于 `session/prompt`）、不做 MCP server 注入、
//! 不支持图片/音频等非文本内容块（跳过；全为非文本则 `-32602`）。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use onion::session::SessionEvent;
use onion::types::{AgentMessage, ContentBlock, ToolResult};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, Mutex};

use crate::session::{spawn_turn, ServerSession, SessionRegistry};

/// ACP 协议版本（对齐 agent-client-protocol v1）。
pub(crate) const PROTOCOL_VERSION: u64 = 1;

/// 单条更新文本上限（工具输出可能很大，流式帧保持可读）。
const MAX_UPDATE_TEXT_CHARS: usize = 20_000;

/// 轮询步长：驱动轮次期间按此间隔扫描 durable 事件并转成 `session/update`。
const POLL_INTERVAL: Duration = Duration::from_millis(100);

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
const INTERNAL_ERROR: i64 = -32603;

/// 连接级共享状态（会话表：`session/new` 建立，`session/prompt` 消费）。
type Sessions = Arc<Mutex<HashMap<String, Arc<ServerSession>>>>;

/// 以本进程 stdin/stdout 提供 ACP 服务（正常结束 = 客户端关闭 stdin）。
pub(crate) async fn serve(registry: SessionRegistry) -> std::io::Result<()> {
    serve_io(registry, tokio::io::stdin(), tokio::io::stdout()).await
}

/// 任意字节流上的 ACP 服务（测试与嵌入宿主共用；stdout 端由写任务独占）。
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
    // 写任务：单点串行化 stdout，避免并发响应/通知帧交错。
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

    let sessions: Sessions = Arc::new(Mutex::new(HashMap::new()));
    let mut handlers = tokio::task::JoinSet::new();
    let mut lines = BufReader::new(reader).lines();
    loop {
        let line = match lines.next_line().await {
            Ok(Some(line)) => line,
            Ok(None) => break,
            Err(error) => {
                eprintln!("[onion-server] acp stdin 读取失败: {error}");
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
                let _ = tx
                    .send(error_response(Value::Null, PARSE_ERROR, &format!("invalid JSON: {error}")))
                    .await;
                continue;
            }
        };
        let Some(method) = message.get("method").and_then(Value::as_str).map(str::to_string) else {
            let id = message.get("id").cloned().unwrap_or(Value::Null);
            let _ = tx
                .send(error_response(id, INVALID_REQUEST, "missing `method`"))
                .await;
            continue;
        };
        let id = message.get("id").cloned();
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        // 通知（无 id）在读取循环内就地处理：取消必须能在 prompt 在飞时插队。
        let Some(id) = id else {
            if method == "session/cancel" {
                cancel_turn(&sessions, &params).await;
            }
            continue;
        };
        let registry = registry.clone();
        let sessions = sessions.clone();
        let tx = tx.clone();
        handlers.spawn(async move {
            let response = match method.as_str() {
                "initialize" => success_response(id, initialize_result(&params)),
                "authenticate" => success_response(id, json!({})),
                "session/new" => match new_session(&registry, &sessions, &params).await {
                    Ok(session_id) => success_response(id, json!({ "sessionId": session_id })),
                    Err(error) => error_response(id, INVALID_PARAMS, &error),
                },
                "session/prompt" => prompt(&registry, &sessions, &params, &tx, id.clone()).await,
                other => error_response(
                    id,
                    METHOD_NOT_FOUND,
                    &format!("unsupported ACP method `{other}`"),
                ),
            };
            let _ = tx.send(response).await;
        });
    }
    // 客户端关闭输入：放弃在飞请求，等写任务把已排队帧刷完。
    handlers.shutdown().await;
    drop(tx);
    let _ = writer_task.await;
    Ok(())
}

/// `initialize` 结果：协议版本 + agent 能力 + 鉴权方法（本实现无鉴权方式）。
fn initialize_result(_params: &Value) -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "agentCapabilities": {
            "loadSession": false,
            "promptCapabilities": {
                "image": false,
                "audio": false,
                "embeddedContext": false
            },
            "mcpCapabilities": {
                "http": false,
                "sse": false
            }
        },
        "authMethods": []
    })
}

/// `session/new`：按 `cwd`（权限根）创建会话并登记到连接表。
async fn new_session(
    registry: &SessionRegistry,
    sessions: &Sessions,
    params: &Value,
) -> Result<String, String> {
    let cwd = params
        .get("cwd")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    // ACP 无审批通道：会话按非交互装配（工具准入自动放行，与 A2A 一致）。
    let session = registry
        .create(None, None, false, cwd)
        .await
        .map_err(|error| format!("create session failed: {error}"))?;
    let id = session.id.clone();
    sessions.lock().await.insert(id.clone(), session);
    Ok(id)
}

/// 会话定位：连接表优先，其次是已落盘/已在内存的历史会话（`open`）。
async fn resolve_session(
    registry: &SessionRegistry,
    sessions: &Sessions,
    session_id: &str,
) -> Result<Arc<ServerSession>, String> {
    if let Some(session) = sessions.lock().await.get(session_id) {
        return Ok(session.clone());
    }
    let session = registry
        .open(session_id, false)
        .await
        .map_err(|error| format!("open session failed: {error}"))?
        .ok_or_else(|| format!("unknown session `{session_id}`"))?;
    sessions
        .lock()
        .await
        .insert(session.id.clone(), session.clone());
    Ok(session)
}

/// `session/prompt`：投递用户文本 → 驱动一轮 → 流式回推更新 → 返回 stopReason。
async fn prompt(
    registry: &SessionRegistry,
    sessions: &Sessions,
    params: &Value,
    tx: &mpsc::Sender<Value>,
    id: Value,
) -> Value {
    let Some(session_id) = params.get("sessionId").and_then(Value::as_str) else {
        return error_response(id, INVALID_PARAMS, "`sessionId` is required");
    };
    let session = match resolve_session(registry, sessions, session_id).await {
        Ok(session) => session,
        Err(error) => return error_response(id, INVALID_PARAMS, &error),
    };
    let text = match prompt_text(params) {
        Ok(text) => text,
        Err(error) => return error_response(id, INVALID_PARAMS, &error),
    };
    // 基线：本轮新增的 durable 事件据此识别（含投递前的历史，不重放）。
    let mut seen = session.handle.runtime.session().snapshot().len();
    let seq = session.handle.gate.enqueue();
    if let Err(error) = session.handle.runtime.inbox().followup(AgentMessage::user(text)) {
        return error_response(id, INTERNAL_ERROR, &format!("inbox 投递失败: {error}"));
    }
    spawn_turn(&session.handle);
    loop {
        seen = flush_updates(&session, session_id, seen, tx).await;
        if session.handle.gate.is_done(seq) {
            break;
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    // 终态补扫：`TurnEnd` 前后可能还有尾随的 assistant/tool 事件。
    flush_updates(&session, session_id, seen, tx).await;
    let reason = stop_reason(&session);
    success_response(id, json!({ "stopReason": reason }))
}

/// 把 `seen` 之后的新事件转成 `session/update` 通知；返回新的游标。
async fn flush_updates(
    session: &Arc<ServerSession>,
    session_id: &str,
    seen: usize,
    tx: &mpsc::Sender<Value>,
) -> usize {
    let events = session.handle.runtime.session().snapshot();
    for event in events.iter().skip(seen) {
        for update in session_updates(event) {
            let frame = json!({
                "jsonrpc": "2.0",
                "method": "session/update",
                "params": { "sessionId": session_id, "update": update }
            });
            if tx.send(frame).await.is_err() {
                return events.len();
            }
        }
    }
    events.len()
}

/// 轮次终态 → ACP `stopReason`。
fn stop_reason(session: &Arc<ServerSession>) -> &'static str {
    match session.channels.last_report() {
        Some(summary) if summary.reason.contains("cancel") => "cancelled",
        Some(summary) if summary.reason.contains("max_tokens") => "max_tokens",
        _ => "end_turn",
    }
}

/// `session/cancel` 通知：取消在飞轮次（未知会话幂等忽略）。
async fn cancel_turn(sessions: &Sessions, params: &Value) {
    let Some(session_id) = params.get("sessionId").and_then(Value::as_str) else {
        return;
    };
    let session = sessions.lock().await.get(session_id).cloned();
    if let Some(session) = session {
        session.handle.runtime.cancel_handle().cancel();
    }
}

/// 从 `session/prompt` 的 `prompt` 数组提取用户文本（非文本内容块跳过）。
pub(crate) fn prompt_text(params: &Value) -> Result<String, String> {
    let Some(blocks) = params.get("prompt").and_then(Value::as_array) else {
        return Err("`prompt` must be an array of content blocks".to_string());
    };
    let mut parts: Vec<&str> = Vec::new();
    for block in blocks {
        // 非文本内容块（图片/音频/嵌入上下文）本实现不支持：跳过而非报错。
        if let Some("text") = block.get("type").and_then(Value::as_str) {
            if let Some(text) = block.get("text").and_then(Value::as_str) {
                if !text.trim().is_empty() {
                    parts.push(text);
                }
            }
        }
    }
    if parts.is_empty() {
        return Err("`prompt` contains no text content".to_string());
    }
    Ok(parts.join("\n"))
}

/// durable 事件 → ACP `session/update` 负载（其余事件不投影）。
pub(crate) fn session_updates(event: &SessionEvent) -> Vec<Value> {
    match event {
        SessionEvent::AssistantMessage { message, .. } => message
            .blocks
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } if !text.trim().is_empty() => Some(json!({
                    "sessionUpdate": "agent_message_chunk",
                    "content": { "type": "text", "text": text }
                })),
                ContentBlock::Thinking { thinking } if !thinking.trim().is_empty() => Some(json!({
                    "sessionUpdate": "agent_thought_chunk",
                    "content": { "type": "text", "text": thinking }
                })),
                _ => None,
            })
            .collect(),
        SessionEvent::ToolCall { call, .. } => vec![json!({
            "sessionUpdate": "tool_call",
            "toolCallId": call.id,
            "title": call.name,
            "kind": "other",
            "status": "pending",
            "rawInput": call.input
        })],
        SessionEvent::ToolResult { result, .. } => vec![json!({
            "sessionUpdate": "tool_call_update",
            "toolCallId": result.tool_call_id,
            "status": if result.is_error { "failed" } else { "completed" },
            "content": [{
                "type": "content",
                "content": { "type": "text", "text": tool_result_text(result) }
            }]
        })],
        _ => Vec::new(),
    }
}

/// 工具结果文本（多个文本块换行拼接，超长截断）。
fn tool_result_text(result: &ToolResult) -> String {
    let joined = result
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    if joined.chars().count() <= MAX_UPDATE_TEXT_CHARS {
        return joined;
    }
    let mut truncated: String = joined.chars().take(MAX_UPDATE_TEXT_CHARS).collect();
    truncated.push_str("\n… [truncated]");
    truncated
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
#[path = "acp_tests.rs"]
mod acp_tests;
