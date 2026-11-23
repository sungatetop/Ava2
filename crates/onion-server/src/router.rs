//! axum 路由：/healthz /readyz /.well-known/agent-card.json /a2a /a2a/sse
//! 与 `/v1` RESTful API（见 `crate::rest`）。
//!
//! /a2a POST：JSON-RPC 2.0 request/response（message/send、tasks/get、tasks/cancel
//! 与 push config 方法）。
//! /a2a/sse GET：SSE 流（message/stream、tasks/resubscribe 经 query 参数协商）。
//!
//! 多 session：每个 session 一个独立 Runtime（SessionRegistry 装配）；A2A 的
//! context_id 即 session_id，REST 与 A2A 共用同一注册表与 Runtime。

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::response::sse::Event as SseFrame;
use axum::response::Sse;
use axum::Json;
use onion::session::{LiveEvent, StreamChunk};
use serde_json::{json, Value};

use crate::a2a::{
    Artifact, ContextId, Message, MessageSendParams, Part, Role, Task, TaskId, TaskIdParams,
    TaskPushNotificationConfig, TaskQueryParams, TaskState, TaskStatus,
    TaskArtifactUpdateEvent, TaskStatusUpdateEvent,
};
use crate::rest::dto::PeerInfo;
use crate::convert::{blocks_to_parts, message_to_agent_message};
use crate::jsonrpc::{
    codes, invalid_params, method_not_found, task_to_value, A2aMethod, JsonRpcRequest, JsonRpcResponse,
};
use crate::session::{
    PendingKind, ReplySender, ServerSession, SessionRegistry, TaskRecord,
};

/// SSE 帧流统一类型（提前 return 与 unfold 分支共用）。
type SseStream =
    Pin<Box<dyn futures_util::Stream<Item = Result<SseFrame, axum::Error>> + Send>>;

/// Router state：session registry + agent card + config（A2A 与 REST 共用）。
#[derive(Clone)]
pub(crate) struct RouterState {
    pub registry: SessionRegistry,
    pub agent_card: Arc<crate::a2a::AgentCard>,
    pub config: Arc<onion::config::Config>,
    /// 可选访问令牌（`ONION_SERVER_TOKEN`）；空 = `/v1` 不鉴权。
    pub token: Option<Arc<str>>,
    /// A2A mesh 令牌（`ONION_A2A_TOKEN`）：`/a2a*` 接受它与运维令牌任一；两者
    /// 都空则 `/a2a*` 匿名。与运维面分属两个信任域。
    pub a2a_token: Option<Arc<str>>,
    pub started_at_ms: u64,
    /// CORS 层（`ONION_SERVER_CORS_ORIGINS` 配置时启用）。
    pub cors: Option<tower_http::cors::CorsLayer>,
    /// 实例身份标识（env `ONION_INSTANCE_ID`，缺省 `default`）。
    pub instance_id: String,
    /// 对端实例表（env `ONION_A2A_PEERS` 指向 JSON 文件，缺省空表）。
    pub peers: Arc<Vec<PeerInfo>>,
    /// AppHost App 仓库根目录（env `ONION_APPS_DIR`，缺省 `<data_home>/apps`）。
    /// 安装即把 App 目录复制到此处，宿主经 `/v1/apps*` 发现与加载。
    pub apps_dir: Arc<std::path::PathBuf>,
}

/// 路由表。
pub(crate) fn build_router(state: RouterState) -> axum::Router {
    use axum::routing::{get, post};

    // 公开端点（无鉴权）：探活与发现文档。
    let public = axum::Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/.well-known/agent-card.json", get(agent_card));
    // 运维面（`/v1/*`）：令牌非空时只认运维令牌 `ONION_SERVER_TOKEN`。
    let operator = crate::rest::build_rest_router().layer(
        axum::middleware::from_fn_with_state(state.clone(), require_token),
    );
    // agent 面（`/a2a*`）：接受 mesh 令牌 `ONION_A2A_TOKEN` 或运维令牌任一，
    // 使出站工具可在不掌握运维令牌的情况下委派任务。
    let a2a = axum::Router::new()
        .route("/a2a", post(handle_a2a))
        .route("/a2a/sse", get(handle_a2a_sse))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            require_a2a_token,
        ));
    let mut app = public.merge(operator).merge(a2a);
    // 入站 webhook 面（`/webhooks/*`）：不认运维令牌，改由事件源签名验证
    // （GitHub HMAC-SHA256），body 上限防大包。
    let webhooks = axum::Router::new()
        .route("/webhooks/github", post(crate::webhook_github::handle))
        .layer(axum::extract::DefaultBodyLimit::max(
            crate::webhook_github::MAX_BODY_BYTES,
        ));
    app = app.merge(webhooks);
    if let Some(cors) = &state.cors {
        app = app.layer(cors.clone());
    }
    app.with_state(state)
}

/// 运维面令牌校验：仅认 `ONION_SERVER_TOKEN`。
async fn require_token(
    State(state): State<RouterState>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let Some(expected) = state.token.as_deref() else {
        return next.run(request).await;
    };
    if provided_token(request.headers()).as_deref() != Some(expected) {
        return crate::rest::error::ApiError::unauthorized().into_response();
    }
    next.run(request).await
}

/// A2A 面令牌校验：接受 mesh 令牌（`ONION_A2A_TOKEN`）或运维令牌任一；
/// 两者都未配置时 `/a2a*` 匿名。
async fn require_a2a_token(
    State(state): State<RouterState>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let accepted: Vec<&str> = [state.a2a_token.as_deref(), state.token.as_deref()]
        .into_iter()
        .flatten()
        .collect();
    if accepted.is_empty() {
        return next.run(request).await;
    }
    let allowed = provided_token(request.headers())
        .is_some_and(|provided| accepted.contains(&provided.as_str()));
    if !allowed {
        return crate::rest::error::ApiError::unauthorized().into_response();
    }
    next.run(request).await
}

/// 从请求头取令牌：`X-Onion-Token: <token>` 或 `Authorization: Bearer <token>`。
fn provided_token(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get("x-onion-token")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
        .or_else(|| {
            headers
                .get(axum::http::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .and_then(|raw| raw.strip_prefix("Bearer ").map(str::to_string))
        })
}

async fn healthz() -> &'static str {
    "ok"
}

async fn readyz() -> &'static str {
    "ready"
}

async fn agent_card(State(state): State<RouterState>) -> Json<crate::a2a::AgentCard> {
    Json((*state.agent_card).clone())
}

/// /a2a POST：JSON-RPC 2.0 分发（非流式方法）。
async fn handle_a2a(
    State(state): State<RouterState>,
    Json(request): Json<JsonRpcRequest>,
) -> Json<JsonRpcResponse> {
    let id = request.id.clone().unwrap_or(Value::Null);
    let Some(method) = A2aMethod::parse(&request.method) else {
        return Json(method_not_found(id));
    };
    let params = request.params.unwrap_or(Value::Null);
    let response = match method {
        A2aMethod::MessageSend => send_message(&state, id, params).await,
        A2aMethod::TasksGet => tasks_get(&state, id, params).await,
        A2aMethod::TasksProvideInput => provide_input(&state, id, params).await,
        A2aMethod::TasksCancel => tasks_cancel(&state, id, params).await,
        A2aMethod::PushConfigSet => push_config_set(&state, id, params).await,
        A2aMethod::PushConfigGet => push_config_get(&state, id, params).await,
        A2aMethod::MessageStream | A2aMethod::TasksResubscribe => JsonRpcResponse::error(
            id,
            codes::INVALID_REQUEST,
            "streaming methods must use GET /a2a/sse",
        ),
    };
    Json(response)
}

/// message/send：投递消息到 session inbox，同步等待轮次结束，返回 Task 快照。
async fn send_message(state: &RouterState, id: Value, params: Value) -> JsonRpcResponse {
    let Ok(params) = serde_json::from_value::<MessageSendParams>(params) else {
        return invalid_params(id, "invalid messageSendParams");
    };
    let task_id = derive_task_id(&params.message);
    let context_id = derive_context_id(&params.message);
    let session = match state.registry.get_or_create(&context_id).await {
        Ok(s) => s,
        Err(error) => {
            return JsonRpcResponse::error(
                id,
                codes::INTERNAL_ERROR,
                format!("session 装配失败: {error}"),
            );
        }
    };
    // `reportTo` 透传落库：发起方在 `message.metadata.reportTo` 声明回传地址
    // （{instanceId, sessionId}），B 侧记录后供跨实例审批桥路由。
    if let Some(report_to) = params
        .message
        .metadata
        .as_ref()
        .and_then(|meta| meta.get("reportTo"))
    {
        *session.report_to.lock().await = Some(report_to.clone());
    }
    // 服务端记录初始 TaskRecord（tasks/get 反查依据）。
    {
        let record = TaskRecord {
            id: task_id.clone(),
            context_id: context_id.clone(),
            state: TaskState::Working,
            history: vec![params.message.clone()],
            artifacts: Vec::new(),
            status_message: None,
        };
        session.tasks.lock().await.insert(task_id.clone(), record);
    }
    // 本会话最近一次在飞 task（跨实例确认请求携带，`tasks/provideInput` 反查）。
    *session.current_task.lock().await = Some(task_id.clone());
    // 入队：返回本次请求序号，同步等待它对应的轮次结束（非全局 idle）。
    let seq = session.handle.gate.enqueue();
    // 投递 user 消息到 inbox（next-turn 分区）+ kick 驱动。
    let message = message_to_agent_message(&params.message);
    if let Err(error) = session.handle.runtime.inbox().followup(message) {
        return JsonRpcResponse::error(
            id,
            codes::INTERNAL_ERROR,
            format!("inbox 投递失败: {error}"),
        );
    }
    // 挂起基线：本轮新增的人机交互项据此识别。
    let before = pending_ids(&session.channels);
    crate::session::spawn_turn(&session.handle);
    // 同步等待：等本次轮次结束；期间出现人机交互挂起项则提前返回 `input-required`
    // （裁决由委派方经 `tasks/provideInput` 或 REST 端点回填）。
    loop {
        if let Some((request_id, kind)) = new_confirmation(&session.channels, &before) {
            let mut tasks = session.tasks.lock().await;
            let Some(record) = tasks.get_mut(&task_id) else {
                return JsonRpcResponse::error(id, codes::INTERNAL_ERROR, "task record missing");
            };
            record.state = TaskState::InputRequired;
            record.status_message = Some(confirm_message(&request_id, &kind));
            let task = task_from_record(record);
            return JsonRpcResponse::success(id, task_to_value(&task));
        }
        if session.handle.gate.is_done(seq) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    // 更新终态并组装 Task 快照。
    let task = {
        let mut tasks = session.tasks.lock().await;
        match tasks.get_mut(&task_id) {
            Some(record) => {
                record.state = TaskState::Completed;
                task_from_record(record)
            }
            None => Task {
                id: task_id,
                context_id,
                status: TaskStatus {
                    state: TaskState::Completed,
                    message: None,
                    timestamp: Some(onion::session::epoch_millis().to_string()),
                },
                artifacts: Vec::new(),
                history: Some(vec![params.message]),
                metadata: None,
            },
        }
    };
    JsonRpcResponse::success(id, task_to_value(&task))
}

/// tasks/provideInput：回填跨实例挂起的人机交互项（plan-review / ask-user）。
/// 不支持工具审批（`approval` 在 A2A 会话中实例内自动放行）。
async fn provide_input(state: &RouterState, id: Value, params: Value) -> JsonRpcResponse {
    let Ok(params) = serde_json::from_value::<ProvideInputParams>(params) else {
        return invalid_params(id, "invalid provideInputParams");
    };
    let Some(session) = find_session_by_task(state, &params.task_id).await else {
        return JsonRpcResponse::error(id, codes::INVALID_PARAMS, "task not found");
    };
    let Some((_, kind, _)) = session
        .channels
        .pending_views()
        .into_iter()
        .find(|(request_id, ..)| request_id == &params.request_id)
    else {
        // 先查已解答窗口：区分「重复回填」与「不存在」。
        return match session.channels.resolved_decision(&params.request_id) {
            Some(decision) => JsonRpcResponse::error(
                id,
                codes::INVALID_PARAMS,
                format!("request already resolved: {decision}"),
            ),
            None => JsonRpcResponse::error(id, codes::INVALID_PARAMS, "request not found"),
        };
    };
    // 按类别组装答复并记录幂等标签。
    let (reply, label) = match kind {
        PendingKind::Approval { .. } => {
            return JsonRpcResponse::error(
                id,
                codes::INVALID_PARAMS,
                "tool approval is auto-allowed in A2A sessions",
            );
        }
        PendingKind::PlanReview { .. } => {
            let verdict = match params.decision.as_deref() {
                Some("approve") => onion::plan::service::PlanVerdict::Approve,
                Some("deny") => onion::plan::service::PlanVerdict::KeepPlanning,
                _ => {
                    return invalid_params(id, "plan review requires decision: approve | deny");
                }
            };
            let label = verdict_label(verdict);
            (InputReply::PlanReview(verdict), label)
        }
        PendingKind::Question { .. } => {
            let Some(answers) = params.answers else {
                return invalid_params(id, "question requires answers");
            };
            let answer = onion::ask_user::types::AskUserAnswer {
                answers: answers
                    .into_iter()
                    .map(|item| onion::ask_user::types::AskUserAnswerItem {
                        id: item.id,
                        selected: item.selected,
                        custom: item.custom,
                    })
                    .collect(),
            };
            let label = format!("answers: {}", answer.answers.len());
            (InputReply::Question(answer), label)
        }
    };
    let Ok(input) = session.channels.resolve(&params.request_id, &label) else {
        return JsonRpcResponse::error(id, codes::INVALID_PARAMS, "request not found");
    };
    match (input.reply, reply) {
        (ReplySender::PlanReview(sender), InputReply::PlanReview(verdict)) => {
            let _ = sender.send(verdict);
        }
        (ReplySender::Question(sender), InputReply::Question(answer)) => {
            let _ = sender.send(answer);
        }
        _ => return JsonRpcResponse::error(id, codes::INTERNAL_ERROR, "reply kind mismatch"),
    }
    // 等轮次结束：把挂起态收敛为终态快照（清 `status_message`）。
    while session.handle.gate.is_busy() {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let mut tasks = session.tasks.lock().await;
    let Some(record) = tasks.get_mut(&params.task_id) else {
        return JsonRpcResponse::error(id, codes::INTERNAL_ERROR, "task record missing");
    };
    if record.state == TaskState::InputRequired {
        record.state = TaskState::Completed;
        record.status_message = None;
    }
    let task = task_from_record(record);
    JsonRpcResponse::success(id, task_to_value(&task))
}

/// `tasks/provideInput` 参数。
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProvideInputParams {
    task_id: TaskId,
    request_id: String,
    #[serde(default)]
    decision: Option<String>,
    #[serde(default)]
    answers: Option<Vec<crate::rest::dto::AnswerInput>>,
}

/// provideInput 的答复载荷（与 [`ReplySender`] 的类别一一对应）。
enum InputReply {
    PlanReview(onion::plan::service::PlanVerdict),
    Question(onion::ask_user::types::AskUserAnswer),
}

/// plan 裁决的幂等标签（已解答窗口记录）。
fn verdict_label(verdict: onion::plan::service::PlanVerdict) -> String {
    match verdict {
        onion::plan::service::PlanVerdict::Approve => "approve".to_string(),
        onion::plan::service::PlanVerdict::KeepPlanning => "deny".to_string(),
        onion::plan::service::PlanVerdict::Dismissed => "dismissed".to_string(),
    }
}

/// 当前待回填项的 id 快照。
fn pending_ids(channels: &crate::session::SessionChannels) -> Vec<String> {
    channels
        .pending_views()
        .into_iter()
        .map(|(request_id, ..)| request_id)
        .collect()
}

/// 基线之外新增的人机交互挂起项（approval 已在实例内自动放行，不参与识别）。
fn new_confirmation(
    channels: &crate::session::SessionChannels,
    before: &[String],
) -> Option<(String, PendingKind)> {
    channels
        .pending_views()
        .into_iter()
        .find(|(request_id, kind, _)| {
            !before.contains(request_id) && !matches!(kind, PendingKind::Approval { .. })
        })
        .map(|(request_id, kind, _)| (request_id, kind))
}

/// 挂起态 `status.message`：携带 `confirmRequest` 元数据，供裁决方回填。
fn confirm_message(request_id: &str, kind: &PendingKind) -> Message {
    Message {
        role: Role::Agent,
        parts: vec![Part::text(kind.summary())],
        kind: Some("message".to_string()),
        message_id: Some(format!("confirm-{request_id}")),
        task_id: None,
        context_id: None,
        metadata: Some(json!({
            "confirmRequest": {
                "requestId": request_id,
                "item": kind.label(),
                "payload": kind,
            }
        })),
    }
}

/// TaskRecord → A2A Task 快照。
fn task_from_record(record: &TaskRecord) -> Task {
    Task {
        id: record.id.clone(),
        context_id: record.context_id.clone(),
        status: TaskStatus {
            state: record.state,
            message: record.status_message.clone(),
            timestamp: Some(onion::session::epoch_millis().to_string()),
        },
        artifacts: record.artifacts.clone(),
        history: Some(record.history.clone()),
        metadata: None,
    }
}

/// tasks/get：按 id 查询任务快照。
async fn tasks_get(state: &RouterState, id: Value, params: Value) -> JsonRpcResponse {
    let Ok(params) = serde_json::from_value::<TaskQueryParams>(params) else {
        return invalid_params(id, "invalid taskQueryParams");
    };
    let Some(session) = find_session_by_task(state, &params.id).await else {
        // 非本地 Task：按委派链接（`tasks/links.jsonl`）合成远端任务快照，
        // `metadata.remoteTask` 标注对端来源（无映射时保持 task not found）。
        return match find_delegated_task(state, &params.id).await {
            Some(task) => JsonRpcResponse::success(id, task_to_value(&task)),
            None => JsonRpcResponse::error(id, codes::INVALID_PARAMS, "task not found"),
        };
    };
    let tasks = session.tasks.lock().await;
    let Some(record) = tasks.get(&params.id) else {
        return JsonRpcResponse::error(id, codes::INVALID_PARAMS, "task not found");
    };
    let task = task_from_record(record);
    JsonRpcResponse::success(id, task_to_value(&task))
}

/// tasks/cancel：取消任务（cancel_handle + 终态标记）。
async fn tasks_cancel(state: &RouterState, id: Value, params: Value) -> JsonRpcResponse {
    let Ok(params) = serde_json::from_value::<TaskIdParams>(params) else {
        return invalid_params(id, "invalid taskIdParams");
    };
    let Some(session) = find_session_by_task(state, &params.id).await else {
        return JsonRpcResponse::error(id, codes::INVALID_PARAMS, "task not found");
    };
    session.handle.runtime.cancel_handle().cancel();
    let mut tasks = session.tasks.lock().await;
    if let Some(record) = tasks.get_mut(&params.id) {
        record.state = TaskState::Canceled;
    }
    JsonRpcResponse::success(id, Value::Null)
}

/// pushNotificationConfig/set。
async fn push_config_set(state: &RouterState, id: Value, params: Value) -> JsonRpcResponse {
    let Ok(params) = serde_json::from_value::<TaskPushNotificationConfig>(params) else {
        return invalid_params(id, "invalid taskPushNotificationConfig");
    };
    let Some(session) = find_session_by_task(state, &params.id).await else {
        return JsonRpcResponse::error(id, codes::INVALID_PARAMS, "task not found");
    };
    *session.push_config.lock().await = Some(params.push_notification_config);
    JsonRpcResponse::success(id, Value::Null)
}

/// pushNotificationConfig/get。
async fn push_config_get(state: &RouterState, id: Value, params: Value) -> JsonRpcResponse {
    let Ok(params) = serde_json::from_value::<TaskIdParams>(params) else {
        return invalid_params(id, "invalid taskIdParams");
    };
    let Some(session) = find_session_by_task(state, &params.id).await else {
        return JsonRpcResponse::error(id, codes::INVALID_PARAMS, "task not found");
    };
    let config = session.push_config.lock().await;
    match &*config {
        Some(config) => JsonRpcResponse::success(
            id,
            serde_json::to_value(config).unwrap_or(Value::Null),
        ),
        None => JsonRpcResponse::error(id, codes::INVALID_PARAMS, "push config not found"),
    }
}

/// /a2a/sse GET：SSE 流式端点（message/stream / tasks/resubscribe）。
async fn handle_a2a_sse(
    State(state): State<RouterState>,
    Query(query): Query<HashMap<String, String>>,
) -> Sse<SseStream> {
    let method = query.get("method").cloned().unwrap_or_default();
    let params: Value = query
        .get("params")
        .and_then(|raw| serde_json::from_str(raw).ok())
        .unwrap_or(Value::Null);
    let Some(method) = A2aMethod::parse(&method) else {
        return sse_error("unsupported or missing method");
    };
    // 解析 session / task / context 标识。
    match method {
        A2aMethod::MessageStream => {
            let Ok(params) = serde_json::from_value::<MessageSendParams>(params) else {
                return sse_error("invalid messageSendParams");
            };
            let context_id = derive_context_id(&params.message);
            let task_id = derive_task_id(&params.message);
            let session = match state.registry.get_or_create(&context_id).await {
                Ok(s) => s,
                Err(error) => return sse_error(format!("session 装配失败: {error}")),
            };
            // 记录 TaskRecord + 投递 + kick 驱动。
            {
                let record = TaskRecord {
                    id: task_id.clone(),
                    context_id: context_id.clone(),
                    state: TaskState::Working,
                    history: vec![params.message.clone()],
                    artifacts: Vec::new(),
                    status_message: None,
                };
                session.tasks.lock().await.insert(task_id.clone(), record);
            }
            *session.current_task.lock().await = Some(task_id.clone());
            session.handle.gate.enqueue();
            let message = message_to_agent_message(&params.message);
            if let Err(error) = session.handle.runtime.inbox().followup(message) {
                return sse_error(format!("inbox 投递失败: {error}"));
            }
            crate::session::spawn_turn(&session.handle);
            sse_run(&session, task_id, context_id)
        }
        A2aMethod::TasksResubscribe => {
            let Ok(params) = serde_json::from_value::<TaskIdParams>(params) else {
                return sse_error("invalid taskIdParams");
            };
            let Some(session) = find_session_by_task(&state, &params.id).await else {
                return sse_error("task not found");
            };
            let task_id = params.id;
            let context_id = ContextId::new(session.id.clone());
            sse_run(&session, task_id, context_id)
        }
        _ => sse_error("streaming must use method=message/stream|tasks/resubscribe"),
    }
}

/// SSE 常驻流：订阅 Runtime 事件 → A2A status-update / artifact-update。
fn sse_run(session: &Arc<ServerSession>, task_id: TaskId, context_id: ContextId) -> Sse<SseStream> {
    let events = session.handle.runtime.subscribe();
    // state 用 Option 承载终态：is_final 时下一轮 state 为 None，闭包据此关流。
    let stream = futures_util::stream::unfold(
        Some((task_id, context_id, events)),
        |state| async move {
            let (task_id, context_id, mut events) = state?;
            match events.recv().await {
                Ok(event) => match live_event_to_sse(&event, &task_id, &context_id) {
                    Some((name, data, is_final)) => {
                        let frame = SseFrame::default().event(name).data(data.to_string());
                        if is_final {
                            Some((Ok(frame), None))
                        } else {
                            Some((Ok(frame), Some((task_id, context_id, events))))
                        }
                    }
                    // 过滤事件：keep-alive 帧继续。
                    None => Some((
                        Ok(SseFrame::default().event("ping").data("{}")),
                        Some((task_id, context_id, events)),
                    )),
                },
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    eprintln!("[onion-server] SSE lagged, skipped {skipped} events");
                    Some((
                        Ok(SseFrame::default().event("ping").data("{}")),
                        Some((task_id, context_id, events)),
                    ))
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => None,
            }
        },
    );
    Sse::new(Box::pin(stream))
}

/// 从 Message 提取或派生 task_id（无显式 task_id 时派生）。
fn derive_task_id(message: &Message) -> TaskId {
    message
        .task_id
        .clone()
        .unwrap_or_else(|| TaskId::new(format!("task-{}", onion::session::epoch_millis())))
}

/// 从 A2A Message 提取或派生 context_id。
fn derive_context_id(message: &Message) -> ContextId {
    message
        .context_id
        .clone()
        .unwrap_or_else(|| ContextId::new(format!("ctx-{}", onion::session::epoch_millis())))
}

/// 按 task_id 反查 session（遍历 registry）。
async fn find_session_by_task(
    state: &RouterState,
    task_id: &TaskId,
) -> Option<Arc<ServerSession>> {
    for session in state.registry.all().await {
        if session.tasks.lock().await.contains_key(task_id) {
            return Some(session);
        }
    }
    None
}

/// 按远端 task_id 反查委派链接（本实例只持有链接，不持有远端 Task 本体）。
async fn find_delegated_task(state: &RouterState, task_id: &TaskId) -> Option<Task> {
    for session in state.registry.all().await {
        let links = onion::tools::read_links(&session.sessions_dir, &session.id);
        let Some(link) = links
            .into_iter()
            .find(|link| link.remote_task_id == task_id.as_str())
        else {
            continue;
        };
        return Some(Task {
            id: TaskId::new(link.remote_task_id),
            context_id: ContextId::new(link.remote_context_id),
            status: TaskStatus {
                state: link_state(&link.remote_state),
                message: None,
                timestamp: Some(link.ts.to_string()),
            },
            artifacts: Vec::new(),
            history: None,
            metadata: Some(json!({
                "remoteTask": {
                    "peerUrl": link.peer_url,
                    "messageId": link.message_id,
                    "reportTo": link.report_to,
                }
            })),
        });
    }
    None
}

/// 委派链接中的状态标签（kebab-case）→ [`TaskState`]。
fn link_state(label: &str) -> TaskState {
    serde_json::from_value(Value::String(label.to_string())).unwrap_or(TaskState::Unknown)
}

/// LiveEvent → (event_name, payload, is_final) 映射。
/// - StateChanged → 忽略（与 TurnStart/TurnEnd 重复）
/// - Durable(TurnStart) → status-update Working
/// - Durable(TurnEnd) → status-update Completed（终态）
/// - Durable(AssistantMessage) → artifact-update（整条 assistant 消息）
/// - AssistantStream(Text) → artifact-update（append 流式块）
/// - AssistantStream(End) → artifact-update last_chunk=true
fn live_event_to_sse(
    event: &LiveEvent,
    task_id: &TaskId,
    context_id: &ContextId,
) -> Option<(String, Value, bool)> {
    match event {
        LiveEvent::Durable(onion::session::SessionEvent::TurnStart { .. }) => {
            Some(status_update(task_id, context_id, TaskState::Working, None))
        }
        LiveEvent::Durable(onion::session::SessionEvent::TurnEnd { .. }) => {
            Some(status_update(task_id, context_id, TaskState::Completed, None))
        }
        LiveEvent::Durable(onion::session::SessionEvent::AssistantMessage { message, .. }) => {
            let artifact = Artifact {
                artifact_id: format!(
                    "artifact-{}",
                    onion::session::epoch_millis()
                ),
                name: None,
                parts: blocks_to_parts(&message.blocks),
                description: None,
                metadata: None,
            };
            Some(artifact_update(task_id, context_id, artifact, None, None))
        }
        LiveEvent::AssistantStream(StreamChunk::Start { .. }) => {
            Some(status_update(task_id, context_id, TaskState::Working, None))
        }
        LiveEvent::AssistantStream(StreamChunk::Text(text)) => {
            let artifact = Artifact {
                artifact_id: "stream".to_string(),
                name: None,
                parts: vec![Part::text(text.clone())],
                description: None,
                metadata: None,
            };
            Some(artifact_update(task_id, context_id, artifact, Some(true), None))
        }
        LiveEvent::AssistantStream(StreamChunk::End) => {
            let artifact = Artifact {
                artifact_id: "stream".to_string(),
                name: None,
                parts: Vec::new(),
                description: None,
                metadata: None,
            };
            Some(artifact_update(task_id, context_id, artifact, None, Some(true)))
        }
        _ => None,
    }
}

/// status-update 事件构造。
fn status_update(
    task_id: &TaskId,
    context_id: &ContextId,
    state: TaskState,
    message: Option<Message>,
) -> (String, Value, bool) {
    let event = TaskStatusUpdateEvent {
        task_id: task_id.clone(),
        context_id: context_id.clone(),
        status: TaskStatus {
            state,
            message,
            timestamp: Some(onion::session::epoch_millis().to_string()),
        },
        is_final: state.is_terminal(),
        metadata: None,
    };
    let data = serde_json::to_value(&event).unwrap_or(Value::Null);
    ("status-update".to_string(), data, event.is_final)
}

/// artifact-update 事件构造。
fn artifact_update(
    task_id: &TaskId,
    context_id: &ContextId,
    artifact: Artifact,
    append: Option<bool>,
    last_chunk: Option<bool>,
) -> (String, Value, bool) {
    let event = TaskArtifactUpdateEvent {
        task_id: task_id.clone(),
        context_id: context_id.clone(),
        artifact,
        append,
        last_chunk,
        metadata: None,
    };
    let data = serde_json::to_value(&event).unwrap_or(Value::Null);
    ("artifact-update".to_string(), data, false)
}

/// 一次性 SSE 错误帧。
fn sse_error(message: impl Into<String>) -> Sse<SseStream> {
    let frame = Err(axum::Error::new(std::io::Error::other(message.into())));
    Sse::new(Box::pin(futures_util::stream::iter(vec![frame])))
}
