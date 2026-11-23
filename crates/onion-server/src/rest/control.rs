//! 会话控制面：agent / goal / plan / tasks / jobs。

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;

use crate::a2a::TaskId;
use crate::rest::dto::{
    AgentInfo, AgentView, GoalActionRequest, GoalInfo, JobDetail, JobInfo, JobKillAck,
    JobListResponse, PlanInfo, SessionInfo, SetAgentRequest, SetApprovalRequest, SetGoalRequest,
    SetModelRequest, SetPlanRequest, TaskInfo, TaskListResponse,
};
use crate::rest::error::ApiError;
use crate::rest::require_session;
use crate::rest::sessions::session_info;
use crate::router::RouterState;
use crate::session::{ServerSession, TaskRecord};

use onion::goal::types::{GoalBlockReason, GoalError, GoalRef};

/// goal 创建缺省轮次预算（与引擎教学子集示例一致）。
const DEFAULT_MAX_GOAL_ROUNDS: u32 = 5;

/// 作业输出缺省尾部行数。
const DEFAULT_OUTPUT_LIMIT: usize = 200;

// ---------------- agent ----------------

async fn agent_view(session: &ServerSession) -> AgentView {
    let catalog = &session.handle.agent_catalog;
    let agents = catalog
        .ids()
        .iter()
        .filter_map(|agent_id| catalog.get(agent_id))
        .map(|agent| AgentInfo {
            id: agent.id.clone(),
            name: agent.name.clone(),
            tools: agent.tools.clone(),
        })
        .collect();
    AgentView {
        active: session.handle.active_agent.lock().await.clone(),
        agents,
    }
}

/// `GET /v1/sessions/{id}/agent`。
pub(crate) async fn agent(
    State(state): State<RouterState>,
    Path(id): Path<String>,
) -> Result<Json<AgentView>, ApiError> {
    let session = require_session(&state, &id).await?;
    Ok(Json(agent_view(&session).await))
}

/// `PUT /v1/sessions/{id}/agent`：切换预设并重设工具面白名单。
pub(crate) async fn set_agent(
    State(state): State<RouterState>,
    Path(id): Path<String>,
    Json(body): Json<SetAgentRequest>,
) -> Result<Json<AgentView>, ApiError> {
    let session = require_session(&state, &id).await?;
    let catalog = &session.handle.agent_catalog;
    let agent = catalog.get(&body.agent_id).cloned().ok_or_else(|| {
        ApiError::bad_request(
            "unknown_agent",
            format!(
                "unknown agent `{}` (available: {})",
                body.agent_id,
                catalog.ids().join(", ")
            ),
        )
    })?;
    let mut tools = agent.tools.clone();
    tools.extend(session.handle.mcp_bridge.registered_tool_names());
    session.handle.tools_handle.state().set_agent(tools);
    *session.handle.active_agent.lock().await = agent.id;
    Ok(Json(agent_view(&session).await))
}

// ---------------- model / approval ----------------

/// `PUT /v1/sessions/{id}/model`：运行期切换模型目标（下一请求生效，不中断在飞 step）。
///
/// kernel 与 llm 共享同一 `ModelTargetRef`：`swap_current` 后 llm 读 assembled
/// 目标路由 base_url/api_key，kernel 在 prompt-assembly 边界快照并注入
/// modelSwitchNotice。审计轨迹落 durable `ModelSelection` 事件。
pub(crate) async fn set_model(
    State(state): State<RouterState>,
    Path(id): Path<String>,
    Json(body): Json<SetModelRequest>,
) -> Result<Json<SessionInfo>, ApiError> {
    let session = require_session(&state, &id).await?;
    let model = body.model.trim();
    let catalog = onion::provider_catalog::ProviderCatalog::load(
        &onion::paths::data_home(),
        &state.config.model,
    );
    let target = catalog.target_for(model).ok_or_else(|| {
        ApiError::bad_request(
            "unknown_model",
            format!("model `{model}` not in provider catalog"),
        )
    })?;
    session.handle.model_target.swap_current(target.clone());
    session.handle.runtime.session().append(
        onion::session::SessionEvent::ModelSelection {
            model: target.model.clone(),
            base_url: target.base_url.clone(),
            effort: target.effort.clone(),
        },
    );
    Ok(Json(session_info(&session).await))
}

/// `PUT /v1/sessions/{id}/approval`：运行期切换审批预设（下一工具调用生效）。
///
/// 写 ApprovalGate 共享的 `Arc<RwLock<ApprovalConfig>>`；`auto` 预设经
/// `build_approval_config` 升级 LLM 复审裁决者（复用会话主 llm 与当前模型）。
pub(crate) async fn set_approval(
    State(state): State<RouterState>,
    Path(id): Path<String>,
    Json(body): Json<SetApprovalRequest>,
) -> Result<Json<SessionInfo>, ApiError> {
    let session = require_session(&state, &id).await?;
    let preset = body.preset.trim();
    if !crate::rest::commands::APPROVAL_PRESETS.contains(&preset) {
        return Err(ApiError::bad_request(
            "unknown_preset",
            format!(
                "unknown approval preset `{preset}` (available: {})",
                crate::rest::commands::APPROVAL_PRESETS.join(", ")
            ),
        ));
    }
    let model = session
        .handle
        .model_target
        .current()
        .map(|target| target.model)
        .unwrap_or_else(|| state.config.model.clone());
    let config = crate::session::build_approval_config(preset, &session.handle.llm, &model);
    *session
        .handle
        .approval_config
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = config;
    // durable 记录：与 `/permission` 命令同源落盘。
    session.handle.runtime.session().append(
        onion::session::SessionEvent::ApprovalPreset {
            preset: preset.to_string(),
            source: None,
        },
    );
    Ok(Json(session_info(&session).await))
}

// ---------------- goal ----------------

fn goal_error(error: GoalError) -> ApiError {
    match &error {
        GoalError::AlreadyExists(_)
        | GoalError::InvalidTransition { .. }
        | GoalError::RevisionMismatch { .. }
        | GoalError::RoundBudgetExhausted { .. } => {
            ApiError::conflict("goal_conflict", error.to_string())
        }
        _ => ApiError::bad_request("invalid_goal_action", error.to_string()),
    }
}

fn current_goal(session: &ServerSession) -> Result<GoalInfo, ApiError> {
    session
        .handle
        .goal_service
        .view()
        .as_ref()
        .map(GoalInfo::from_view)
        .ok_or_else(|| ApiError::not_found("goal_not_found", "goal not created"))
}

/// `GET /v1/sessions/{id}/goal`。
pub(crate) async fn goal(
    State(state): State<RouterState>,
    Path(id): Path<String>,
) -> Result<Json<Option<GoalInfo>>, ApiError> {
    let session = require_session(&state, &id).await?;
    Ok(Json(
        session
            .handle
            .goal_service
            .view()
            .as_ref()
            .map(GoalInfo::from_view),
    ))
}

/// `PUT /v1/sessions/{id}/goal`：无 `revision` 创建，有 `revision` 编辑。
pub(crate) async fn set_goal(
    State(state): State<RouterState>,
    Path(id): Path<String>,
    Json(body): Json<SetGoalRequest>,
) -> Result<Json<Option<GoalInfo>>, ApiError> {
    let session = require_session(&state, &id).await?;
    let service = &session.handle.goal_service;
    match body.revision {
        Some(revision) => {
            let current = current_goal(&session)?;
            service
                .edit(
                    GoalRef { id: current.id, revision },
                    body.objective,
                    body.max_goal_rounds,
                )
                .map_err(goal_error)?;
        }
        None => {
            let objective = body.objective.ok_or_else(|| {
                ApiError::bad_request("invalid_request", "objective is required to create a goal")
            })?;
            service
                .create(
                    objective,
                    body.max_goal_rounds.unwrap_or(DEFAULT_MAX_GOAL_ROUNDS),
                )
                .map_err(goal_error)?;
        }
    }
    Ok(Json(service.view().as_ref().map(GoalInfo::from_view)))
}

/// `POST /v1/sessions/{id}/goal/actions`。
pub(crate) async fn goal_action(
    State(state): State<RouterState>,
    Path(id): Path<String>,
    Json(body): Json<GoalActionRequest>,
) -> Result<Json<Option<GoalInfo>>, ApiError> {
    let session = require_session(&state, &id).await?;
    let reference = GoalRef { id: current_goal(&session)?.id, revision: body.revision };
    let service = &session.handle.goal_service;
    match body.action.as_str() {
        "pause" => service.pause(reference).map_err(goal_error)?,
        "resume" => service.resume(reference).map_err(goal_error)?,
        "complete" => service.complete(reference).map_err(goal_error)?,
        "block" => service
            .block(
                reference,
                GoalBlockReason::new(
                    "operator",
                    body.reason.unwrap_or_else(|| "blocked by operator".to_string()),
                ),
            )
            .map_err(goal_error)?,
        "clear" => {
            service.clear(reference).map_err(goal_error)?;
            return Ok(Json(None));
        }
        other => {
            return Err(ApiError::bad_request(
                "invalid_goal_action",
                format!("unknown action `{other}` (pause|resume|complete|block|clear)"),
            ));
        }
    };
    Ok(Json(service.view().as_ref().map(GoalInfo::from_view)))
}

// ---------------- plan ----------------

fn plan_info(session: &ServerSession) -> PlanInfo {
    PlanInfo {
        active: session.handle.plan_mode.active(),
        pending: session.handle.plan_mode.pending(),
    }
}

/// `GET /v1/sessions/{id}/plan`。
pub(crate) async fn plan(
    State(state): State<RouterState>,
    Path(id): Path<String>,
) -> Result<Json<PlanInfo>, ApiError> {
    let session = require_session(&state, &id).await?;
    Ok(Json(plan_info(&session)))
}

/// `POST /v1/sessions/{id}/plan`：空闲时立即生效，在飞轮次下挂起到 step 边界。
pub(crate) async fn set_plan(
    State(state): State<RouterState>,
    Path(id): Path<String>,
    Json(body): Json<SetPlanRequest>,
) -> Result<Json<PlanInfo>, ApiError> {
    let session = require_session(&state, &id).await?;
    session.handle.plan_mode.set(body.active);
    if !session.handle.gate.is_busy() {
        session.handle.plan_mode.flush_pending();
    }
    Ok(Json(plan_info(&session)))
}

// ---------------- tasks ----------------

/// `GET /v1/sessions/{id}/tasks`。
pub(crate) async fn tasks(
    State(state): State<RouterState>,
    Path(id): Path<String>,
) -> Result<Json<TaskListResponse>, ApiError> {
    let session = require_session(&state, &id).await?;
    let mut tasks: Vec<TaskInfo> = session
        .tasks
        .lock()
        .await
        .values()
        .map(TaskInfo::from_record)
        .collect();
    // 追加委派链接（`<session>/tasks/links.jsonl`）：远端任务摘要带 `remote` 标注。
    tasks.extend(
        onion::tools::read_links(&session.sessions_dir, &id)
            .iter()
            .map(TaskInfo::from_link),
    );
    Ok(Json(TaskListResponse { tasks }))
}

/// `GET /v1/sessions/{id}/tasks/{taskId}`。
pub(crate) async fn task(
    State(state): State<RouterState>,
    Path((id, task_id)): Path<(String, String)>,
) -> Result<Json<TaskInfo>, ApiError> {
    let session = require_session(&state, &id).await?;
    let tasks = session.tasks.lock().await;
    let record: &TaskRecord = tasks.get(&TaskId::new(task_id.clone())).ok_or_else(|| {
        ApiError::not_found("task_not_found", format!("task `{task_id}` not found"))
    })?;
    Ok(Json(TaskInfo::from_record(record)))
}

/// `POST /v1/sessions/{id}/tasks/{taskId}/cancel`。
pub(crate) async fn cancel_task(
    State(state): State<RouterState>,
    Path((id, task_id)): Path<(String, String)>,
) -> Result<Json<TaskInfo>, ApiError> {
    let session = require_session(&state, &id).await?;
    session.handle.runtime.cancel_handle().cancel();
    let mut tasks = session.tasks.lock().await;
    let record = tasks
        .get_mut(&TaskId::new(task_id.clone()))
        .ok_or_else(|| {
            ApiError::not_found("task_not_found", format!("task `{task_id}` not found"))
        })?;
    record.state = crate::a2a::TaskState::Canceled;
    Ok(Json(TaskInfo::from_record(record)))
}

// ---------------- jobs ----------------

/// `GET /v1/sessions/{id}/jobs`。
pub(crate) async fn jobs(
    State(state): State<RouterState>,
    Path(id): Path<String>,
) -> Result<Json<JobListResponse>, ApiError> {
    let session = require_session(&state, &id).await?;
    let jobs = session
        .handle
        .job_registry
        .list(&session.handle.job_owner)
        .iter()
        .map(JobInfo::from_view)
        .collect();
    Ok(Json(JobListResponse { jobs }))
}

/// `GET /v1/sessions/{id}/jobs/{jobId}`：详情 + 输出尾部（`?limit=`）。
pub(crate) async fn job(
    State(state): State<RouterState>,
    Path((id, job_id)): Path<(String, String)>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<JobDetail>, ApiError> {
    let session = require_session(&state, &id).await?;
    let view = session
        .handle
        .job_registry
        .get(&job_id)
        .ok_or_else(|| ApiError::not_found("job_not_found", format!("job `{job_id}` not found")))?;
    let limit = query
        .get("limit")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_OUTPUT_LIMIT);
    let output = session
        .handle
        .job_registry
        .read_output(&job_id, limit)
        .unwrap_or_default();
    Ok(Json(JobDetail { job: JobInfo::from_view(&view), output }))
}

/// `POST /v1/sessions/{id}/jobs/{jobId}/kill`。
pub(crate) async fn kill_job(
    State(state): State<RouterState>,
    Path((id, job_id)): Path<(String, String)>,
) -> Result<(StatusCode, Json<JobKillAck>), ApiError> {
    use onion_jobs::KillResult;
    let session = require_session(&state, &id).await?;
    match session.handle.job_registry.kill(&job_id) {
        KillResult::Requested => Ok((
            StatusCode::OK,
            Json(JobKillAck { job_id, result: "requested" }),
        )),
        KillResult::AlreadySettled => Err(ApiError::conflict(
            "already_settled",
            format!("job `{job_id}` already settled"),
        )),
        KillResult::NotFound => Err(ApiError::not_found(
            "job_not_found",
            format!("job `{job_id}` not found"),
        )),
    }
}
