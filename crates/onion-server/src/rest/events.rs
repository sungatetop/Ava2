//! `GET /v1/sessions/{id}/events`：会话事件 SSE 流。
//!
//! 首帧 `snapshot`（durable 事件全量，重连对齐），随后逐条转发会话帧
//! （`durable` / `live` / `*_requested` / `turn_report` / `session_closed`）。
//! 帧 `event:` 名与 payload 的 `type` 标签一致。

use axum::extract::{Path, State};
use axum::response::sse::{Event as SseFrame, KeepAlive, Sse};
use futures_util::StreamExt;

use crate::rest::error::ApiError;
use crate::rest::require_session;
use crate::router::RouterState;
use crate::session::SessionNotice;

/// 会话事件流。
pub(crate) async fn stream(
    State(state): State<RouterState>,
    Path(id): Path<String>,
) -> Result<
    Sse<impl futures_util::Stream<Item = Result<SseFrame, axum::Error>> + Send>,
    ApiError,
> {
    let session = require_session(&state, &id).await?;
    let rx = session.channels.subscribe();
    let snapshot = SessionNotice::Snapshot {
        events: session.handle.runtime.session().snapshot(),
    };
    let snapshot_frame = SseFrame::default()
        .event(snapshot.event_name())
        .data(serde_json::to_string(&snapshot).unwrap_or_else(|_| "{}".to_string()));
    let stream = futures_util::stream::once(async move { Ok::<_, axum::Error>(snapshot_frame) })
        .chain(futures_util::stream::unfold(rx, |mut rx| async move {
            loop {
                match rx.recv().await {
                    Ok(notice) => {
                        let data =
                            serde_json::to_string(&notice).unwrap_or_else(|_| "{}".to_string());
                        let frame = SseFrame::default().event(notice.event_name()).data(data);
                        return Some((Ok(frame), rx));
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        eprintln!("[onion-server] /v1 events 订阅滞后，跳过 {skipped} 个事件");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
                }
            }
        }));
    let framed = Sse::new(stream).keep_alive(KeepAlive::default());
    Ok(framed)
}
