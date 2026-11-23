//! `GET /v1/sessions/{id}/ws`：会话事件 WebSocket 订阅泵。
//!
//! 与 SSE（`/v1/sessions/{id}/events`）同构：首帧 `snapshot`（durable 事件
//! 全量，重连对齐），随后逐条推送 `SessionNotice` 帧。每 15s 服务端
//! Ping 保活；会话关闭（broadcast 通道清零）后发 Close 并断开。

use std::time::Duration;

use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::response::Response;

use crate::router::RouterState;
use crate::session::SessionNotice;

/// 服务端 Ping 间隔（对齐 SSE `KeepAlive::default()` 的 15s）。
const PING_INTERVAL: Duration = Duration::from_secs(15);

/// 会话事件 WS 流。
pub(crate) async fn subscribe(
    State(state): State<RouterState>,
    Path(id): Path<String>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |socket| pump_session(state, id, socket))
}

/// 升级后的事件泵：会话未在内存则按需恢复历史会话（回放日志）；磁盘也没有
/// 该会话则告知并断开（对齐 REST 404 语义）。
async fn pump_session(state: RouterState, id: String, mut socket: WebSocket) {
    let session = match state.registry.open(&id, true).await {
        Ok(Some(session)) => session,
        Ok(None) => {
            // 101 握手已完成，只能经 WS 帧告知并关闭。
            let _ = socket
                .send(WsMessage::Text(
                    axum::extract::ws::Utf8Bytes::from_static("session_not_found"),
                ))
                .await;
            return;
        }
        Err(error) => {
            let _ = socket
                .send(WsMessage::Text(axum::extract::ws::Utf8Bytes::from(
                    format!("session_open_failed: {error}"),
                )))
                .await;
            return;
        }
    };
    let rx = session.channels.subscribe();
    let snapshot = SessionNotice::Snapshot {
        events: session.handle.runtime.session().snapshot(),
    };
    drop(session); // 释放 Arc：否则 broadcast sender 常驻，关闭永远无法到达。
    pump(socket, rx, snapshot).await;
}

/// 事件泵：首帧 snapshot → 逐条 notice 转发；Ping 保活 + 关闭对齐。
async fn pump(
    mut socket: WebSocket,
    mut rx: tokio::sync::broadcast::Receiver<SessionNotice>,
    snapshot: SessionNotice,
) {
    // 首帧：snapshot（与 SSE 对齐）。
    if !send(&mut socket, &snapshot).await {
        return;
    }
    let mut ping = tokio::time::interval(PING_INTERVAL);
    ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    ping.tick().await; // interval 首次 tick 立即完成，消费掉。
    loop {
        tokio::select! {
            notice = rx.recv() => match notice {
                Ok(notice) => {
                    if matches!(notice, SessionNotice::SessionClosed { .. }) {
                        // 会话关闭：转发关闭 notice 后主动断开。
                        send(&mut socket, &notice).await;
                        let _ = socket.send(WsMessage::Close(None)).await;
                        return;
                    }
                    if !send(&mut socket, &notice).await {
                        return;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    eprintln!("[onion-server] /v1 ws 订阅滞后，跳过 {skipped} 个事件");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    // 会话已关闭：通知客户端后断开。
                    let _ = socket.send(WsMessage::Close(None)).await;
                    return;
                }
            },
            _ = ping.tick() => {
                if socket
                    .send(WsMessage::Ping(axum::body::Bytes::new()))
                    .await
                    .is_err()
                {
                    return;
                }
            }
        }
    }
}

/// 单帧发送；对端已断开（发送失败）即结束。
async fn send(socket: &mut WebSocket, notice: &SessionNotice) -> bool {
    let data = serde_json::to_string(notice).unwrap_or_else(|_| "{}".to_string());
    socket
        .send(WsMessage::Text(axum::extract::ws::Utf8Bytes::from(data)))
        .await
        .is_ok()
}
