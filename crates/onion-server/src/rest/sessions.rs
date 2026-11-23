//! 会话生命周期与消息端点（`/v1/sessions*`）。

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;

use crate::a2a::{ContextId, TaskId, TaskState};
use crate::rest::dto::{
    CancelAck, CompactAck, CreateSessionRequest, ForkRequest, ForkResponse, GoalInfo, MessageAck,
    PlanInfo, PostMessageRequest, SessionInfo, SessionListItem, SessionListResponse, TodoResponse,
    TurnInfo, parts_to_agent_message,
};
use crate::rest::error::ApiError;
use crate::rest::require_session;
use crate::router::RouterState;
use crate::session::{ServerSession, SessionNotice, TaskRecord, spawn_turn};

/// 会话详情投影（REST 各端点共用）。
pub(crate) async fn session_info(session: &ServerSession) -> SessionInfo {
    let handle = &session.handle;
    let working_dir = handle.session_cwd.current().display().to_string();
    let approval_preset = handle
        .approval_config
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .preset
        .name
        .clone();
    let events = handle.runtime.session().snapshot();
    SessionInfo {
        id: session.id.clone(),
        agent: handle.active_agent.lock().await.clone(),
        interactive: session.interactive,
        busy: handle.gate.is_busy(),
        working_dir,
        root: session.root.display().to_string(),
        model: handle
            .model_target
            .current()
            .map(|target| target.model)
            .unwrap_or_default(),
        approval_preset,
        created_at_ms: session.created_at_ms,
        event_count: events.len(),
        context_tokens: context_tokens(&events),
        context_window_tokens: session.context_window_tokens,
        last_turn: session
            .channels
            .last_report()
            .map(TurnInfo::from_summary),
        goal: handle
            .goal_service
            .view()
            .as_ref()
            .map(GoalInfo::from_view),
        plan: PlanInfo {
            active: handle.plan_mode.active(),
            pending: handle.plan_mode.pending(),
        },
    }
}

/// 当前上下文占用（token）。
///
/// 取二者较大值：`chars/4` 事件估算（不含工具 schema，偏小）与最近一次请求实际
/// 发送的 prompt token 数（durable `AssistantMessage.usage`，OpenAI 兼容口径下
/// `input_tokens` 即完整 prompt，含缓存命中部分）。
fn context_tokens(events: &[onion::session::SessionEvent]) -> u64 {
    let reported = events
        .iter()
        .rev()
        .find_map(|event| match event {
            onion::session::SessionEvent::AssistantMessage { usage, .. } => {
                Some(usage.input_tokens)
            }
            _ => None,
        })
        .unwrap_or(0);
    onion::compaction::estimate::estimate_events(events).max(reported)
}

/// `GET /v1/sessions`：进程内 live 会话与磁盘会话合并（`?live=true` 仅 live）。
///
/// 磁盘侧聚合 `<data_home>/sessions/*` 全部工作目录（全项目枚举）；
/// `?cwd=<绝对路径>` 可只列该工作目录的会话。
pub(crate) async fn list(
    State(state): State<RouterState>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<SessionListResponse>, ApiError> {
    let live_only = query
        .get("live")
        .is_some_and(|value| value.as_str() == "true");
    let cwd_filter = query
        .get("cwd")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let cwd_dir = cwd_filter
        .as_deref()
        .map(|cwd| state.registry.sessions_dir_for(std::path::Path::new(cwd)));
    let live: Vec<_> = state
        .registry
        .all()
        .await
        .into_iter()
        .filter(|session| {
            cwd_dir.as_ref().is_none_or(|dir| &session.sessions_dir == dir)
        })
        .collect();
    let disk = if live_only {
        Vec::new()
    } else {
        let dirs = match &cwd_dir {
            Some(dir) => vec![dir.clone()],
            None => session_dirs(state.registry.sessions_root()),
        };
        dirs.iter()
            .flat_map(|dir| {
                let workspace = workspace_of(dir);
                onion::session::list_sessions(dir)
                    .into_iter()
                    .map(move |summary| {
                        let working_dir = workspace.clone().or_else(|| summary.working_dir.clone());
                        (working_dir, summary)
                    })
            })
            .collect::<Vec<_>>()
    };
    // 同一 id 只保留一条（跨工作目录重名时以先扫到的为准）。
    let mut disk: Vec<(Option<String>, onion::session::SessionSummary)> = disk;
    disk.sort_by(|a, b| a.1.id.cmp(&b.1.id));
    disk.dedup_by(|a, b| a.1.id == b.1.id);
    let mut items: Vec<SessionListItem> = live
        .iter()
        .map(|session| {
            let found = disk.iter().find(|(_, item)| item.id == session.id);
            SessionListItem {
                id: session.id.clone(),
                live: true,
                event_count: session.handle.runtime.session().snapshot().len(),
                title: found.map(|(_, item)| item.title.clone()).unwrap_or_default(),
                goal_phase: found
                    .map(|(_, item)| item.goal_phase.clone())
                    .unwrap_or_default(),
                created_at_ms: Some(session.created_at_ms),
                working_dir: found
                    .and_then(|(working_dir, _)| working_dir.clone())
                    .or_else(|| workspace_of(&session.sessions_dir)),
                updated_at_ms: Some(
                    found
                        .map(|(_, item)| item.updated_at_ms)
                        .unwrap_or(session.created_at_ms),
                ),
            }
        })
        .collect();
    for (working_dir, summary) in &disk {
        if items.iter().any(|item| item.id == summary.id) {
            continue;
        }
        items.push(SessionListItem {
            id: summary.id.clone(),
            live: false,
            event_count: summary.events,
            title: summary.title.clone(),
            goal_phase: summary.goal_phase.clone(),
            created_at_ms: None,
            working_dir: working_dir.clone(),
            updated_at_ms: Some(summary.updated_at_ms),
        });
    }
    items.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(Json(SessionListResponse { sessions: items }))
}

/// 容器 ↔ 工作区映射（`<container>/cwd`）：会话目录名为归一化路径（有损），
/// 分组读面依赖该标记；缺失时返回 `None`（旧会话回落到日志内 `working_dir`）。
fn workspace_of(dir: &std::path::Path) -> Option<String> {
    crate::session::read_workspace_marker(dir).map(|path| path.to_string_lossy().into_owned())
}

/// `POST /v1/sessions`：显式创建（缺省 `interactive=true`）。
pub(crate) async fn create(
    State(state): State<RouterState>,
    Json(request): Json<CreateSessionRequest>,
) -> Result<(StatusCode, Json<SessionInfo>), ApiError> {
    let interactive = request.interactive.unwrap_or(true);
    let session = state
        .registry
        .create(request.id, request.agent, interactive, request.cwd)
        .await
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::InvalidInput => {
                ApiError::bad_request("invalid_request", error.to_string())
            }
            _ => ApiError::internal(error.to_string()),
        })?;
    Ok((StatusCode::CREATED, Json(session_info(&session).await)))
}

/// `GET /v1/sessions/{id}`。
pub(crate) async fn get(
    State(state): State<RouterState>,
    Path(id): Path<String>,
) -> Result<Json<SessionInfo>, ApiError> {
    let session = require_session(&state, &id).await?;
    Ok(Json(session_info(&session).await))
}

/// `DELETE /v1/sessions/{id}`：结束会话 + 清理其后台作业 + 删除落盘目录
/// （不删则历史列表仍可见、且会被按需恢复成「假删除」）。
pub(crate) async fn remove(
    State(state): State<RouterState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let session = require_session(&state, &id).await?;
    session.channels.notice(SessionNotice::SessionClosed {
        reason: "session deleted".to_string(),
    });
    let _ = session.handle.runtime.end("client delete").await;
    session
        .handle
        .job_registry
        .cleanup_owner(&session.handle.job_owner);
    state.registry.remove(&id).await;
    let session_dir = onion::paths::session_dir(&session.sessions_dir, &id);
    // 先关日志句柄再删目录：Windows 上未关闭的句柄会让 remove_dir_all 报
    // 「目录不是空的」（os error 145）。
    session.handle.runtime.session().close();
    drop(session);
    match std::fs::remove_dir_all(&session_dir) {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(StatusCode::NO_CONTENT),
        Err(error) => Err(ApiError::internal(format!(
            "清理会话目录失败 {}: {error}",
            session_dir.display()
        ))),
    }
}

/// `POST /v1/sessions/{id}/fork`：复制源会话 `[1..=boundary]` 前缀为子会话日志，
/// 追加 ForkSeed 播种标记与合成关闭事件。子会话与
/// 源同容器（继承工作目录），落盘直写 JSONL（新 id 无并发写者，不取写租约）。
pub(crate) async fn fork(
    State(state): State<RouterState>,
    Path(id): Path<String>,
    Json(request): Json<ForkRequest>,
) -> Result<(StatusCode, Json<ForkResponse>), ApiError> {
    let session = require_session(&state, &id).await?;
    let source_log =
        onion::paths::session_log_file(&onion::paths::session_dir(&session.sessions_dir, &id));
    let source = onion::session_query::read_session_events(&source_log)
        .map_err(|error| ApiError::internal(format!("读取源会话日志失败 {}: {error}", source_log.display())))?;
    let child_id = request
        .child_session_id
        .clone()
        .unwrap_or_else(onion::session::fresh_session_id);
    let child_dir = onion::paths::session_dir(&session.sessions_dir, &child_id);
    if child_dir.exists() {
        return Err(ApiError::conflict("session_exists", format!("会话目录已存在: {child_id}")));
    }
    let forked = onion::session_fork::build_fork_seed(&source, request.boundary_seq, &id)
        .map_err(|error| ApiError::bad_request("invalid_boundary", error.to_string()))?;
    onion::session_fork::write_forked_log(&child_dir, &forked.records).map_err(|error| {
        ApiError::internal(format!("写入子会话日志失败 {}: {error}", child_dir.display()))
    })?;
    Ok((
        StatusCode::CREATED,
        Json(ForkResponse {
            child_session_id: child_id,
            inherited_events: forked.inherited_events,
            events: forked.records.len(),
        }),
    ))
}

/// `<data_home>/sessions/` 下的全部工作目录容器。
fn session_dirs(root: std::path::PathBuf) -> Vec<std::path::PathBuf> {
    let Ok(entries) = std::fs::read_dir(&root) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect()
}

/// `GET /v1/sessions/{id}/messages`：durable 事件全量（回放重建依据）。
pub(crate) async fn messages(
    State(state): State<RouterState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<onion::session::SessionEvent>>, ApiError> {
    let session = require_session(&state, &id).await?;
    Ok(Json(session.handle.runtime.session().snapshot()))
}

/// `POST /v1/sessions/{id}/messages`：缺省 `202` 异步，`wait=true` 同步等轮次结束。
pub(crate) async fn post_message(
    State(state): State<RouterState>,
    Path(id): Path<String>,
    Json(request): Json<PostMessageRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let session = require_session(&state, &id).await?;
    let message = parts_to_agent_message(
        &request.parts,
        &crate::rest::attachments::attachment_store(),
    )?;
    let wait = request.wait.unwrap_or(false);
    let task_id = TaskId::new(
        request
            .task_id
            .unwrap_or_else(|| format!("task-{}", onion::session::epoch_millis())),
    );
    {
        let record = TaskRecord {
            id: task_id.clone(),
            context_id: ContextId::new(session.id.clone()),
            state: TaskState::Working,
            history: Vec::new(),
            artifacts: Vec::new(),
            status_message: None,
        };
        session.tasks.lock().await.insert(task_id.clone(), record);
    }
    // 入队：返回本次请求的序号，`wait=true` 等它对应的轮次结束（非全局 idle）。
    let seq = session.handle.gate.enqueue();
    session
        .handle
        .runtime
        .inbox()
        .followup(message)
        .map_err(|error| ApiError::internal(format!("inbox 投递失败: {error}")))?;
    spawn_turn(&session.handle);
    let ack = MessageAck {
        session_id: session.id.clone(),
        task_id: task_id.as_str().to_string(),
        completed: wait,
        last_turn: None,
    };
    if !wait {
        return Ok((StatusCode::ACCEPTED, Json(ack)));
    }
    session.handle.gate.wait_for(seq).await;
    if let Some(record) = session.tasks.lock().await.get_mut(&task_id) {
        record.state = TaskState::Completed;
    }
    Ok((
        StatusCode::OK,
        Json(MessageAck {
            last_turn: session
                .channels
                .last_report()
                .map(TurnInfo::from_summary),
            ..ack
        }),
    ))
}

/// `POST /v1/sessions/{id}/interrupt`：取消在飞轮次。
pub(crate) async fn interrupt(
    State(state): State<RouterState>,
    Path(id): Path<String>,
) -> Result<Json<CancelAck>, ApiError> {
    let session = require_session(&state, &id).await?;
    let busy = session.handle.gate.is_busy();
    // 先撤回待回填输入：drop 答复端 → 内核等待端 recv Err → 审计落 `cancelled`。
    // 否则中断后审批卡仍挂着，用户回填也只能拿到 409。
    session.channels.cancel_all("cancelled");
    session.handle.runtime.cancel_handle().cancel();
    // 清空 inbox：否则残留消息
    // 会立刻开新一轮，用户观感是「没停住」。
    session.handle.runtime.inbox().clear();
    Ok(Json(CancelAck { cancelled: busy }))
}

/// `POST /v1/sessions/{id}/compact`：投递压缩信号；空闲时立即直驱压缩事务。
pub(crate) async fn compact(
    State(state): State<RouterState>,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<CompactAck>), ApiError> {
    let session = require_session(&state, &id).await?;
    session
        .handle
        .compact_tx
        .send(())
        .map_err(|_| ApiError::conflict("session_closed", "session compaction channel closed"))?;
    if !session.handle.gate.is_busy() {
        let log = session.handle.runtime.session();
        session.handle.compaction_listener.compact_now(&log).await;
    }
    Ok((StatusCode::ACCEPTED, Json(CompactAck { queued: true })))
}

/// `GET /v1/sessions/{id}/todos`：最近一次 `todo/write` 快照。
pub(crate) async fn todos(
    State(state): State<RouterState>,
    Path(id): Path<String>,
) -> Result<Json<TodoResponse>, ApiError> {
    let session = require_session(&state, &id).await?;
    let snapshot = session.handle.runtime.session().snapshot();
    let todos = snapshot
        .iter()
        .rev()
        .find_map(|event| match event {
            onion::session::SessionEvent::TodoWrite { todos } => Some(todos.clone()),
            _ => None,
        })
        .unwrap_or_default();
    Ok(Json(TodoResponse { todos }))
}
