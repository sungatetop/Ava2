//! 交付物投影：`GET /v1/sessions/{id}/deliverables`。
//!
//! `present` 工具成功后写入 durable `SessionEvent::Presented`（log-only，模型上下文
//! 不消费）。本模块把该事件序列投影成「会话交付物清单」，供 Web / 脚本客户端读取：
//! 同一 `path` 只保留最后一次声明，按首次声明顺序输出；缺 `path` 或 `path` 为空的
//! 条目丢弃。

use axum::extract::{Path, State};
use axum::Json;

use crate::rest::dto::{DeliverableInfo, DeliverablesResponse};
use crate::rest::error::ApiError;
use crate::rest::require_session;
use crate::router::RouterState;

use onion::session::SessionEvent;

/// `GET /v1/sessions/{id}/deliverables`：会话内全部 `present` 声明。
pub(crate) async fn list(
    State(state): State<RouterState>,
    Path(id): Path<String>,
) -> Result<Json<DeliverablesResponse>, ApiError> {
    let session = require_session(&state, &id).await?;
    let events = session.handle.runtime.session().snapshot();
    Ok(Json(DeliverablesResponse {
        deliverables: project(&events),
    }))
}

/// 事件序列 → 交付物清单（同一 `path` 保留最后一次声明）。
fn project(events: &[SessionEvent]) -> Vec<DeliverableInfo> {
    let mut deliverables: Vec<DeliverableInfo> = Vec::new();
    for event in events {
        let SessionEvent::Presented {
            turn,
            call_id,
            files,
        } = event
        else {
            continue;
        };
        for file in files {
            let Some(path) = file.get("path").and_then(|value| value.as_str()) else {
                continue;
            };
            if path.trim().is_empty() {
                continue;
            }
            let description = file
                .get("description")
                .and_then(|value| value.as_str())
                .filter(|text| !text.trim().is_empty())
                .map(str::to_string);
            let entry = DeliverableInfo {
                turn: *turn,
                call_id: call_id.clone(),
                path: path.to_string(),
                description,
            };
            match deliverables
                .iter_mut()
                .find(|existing| existing.path == entry.path)
            {
                Some(existing) => *existing = entry,
                None => deliverables.push(entry),
            }
        }
    }
    deliverables
}

#[cfg(test)]
#[path = "deliverables_tests.rs"]
mod tests;
