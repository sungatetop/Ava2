//! 定时任务管理（全局任务面）REST：列表 / 创建 / 启停 / 删除 / 立即执行。
//!
//! 与会话内 `schedule_create` 工具共用 `HostScheduleStore`（`<data_home>/
//! schedule/tasks.jsonl`）；全局 CRUD 直接操作 store（跨会话聚合视图），
//! 频率校验复用 [`ScheduleService::create`] 的单 selector 语义。
//! 递送由 [`crate::scheduler`] 后台循环驱动；`POST /run` 经
//! [`onion::schedule::runtime::deliver_task`] 复用同一记账逻辑。

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::rest::error::ApiError;
use crate::router::RouterState;
use crate::scheduler::SessionDeliver;

use onion::schedule::runtime;
use onion::schedule::service::ScheduleService;
use onion::schedule::storage::HostScheduleStore;
use onion::schedule::types::{
    AtInput, CronInput, DailyInput, InactivePolicy, ScheduleCreateRequest, ScheduleRecord,
    ScheduleTask, ScheduleTimingChange, ScheduleUpdateContent, ScheduleUpdateResult, TaskStatus,
    WeeklyInput,
};

/// 从 registry 取进程级 schedule 存储。
fn store(state: &RouterState) -> Arc<HostScheduleStore> {
    state.registry.schedule_store()
}

/// record 频率类型标签（对齐前端频率模式展示）。
fn kind_of(record: &ScheduleRecord) -> &'static str {
    match record {
        ScheduleRecord::At(_) => "at",
        ScheduleRecord::After(_) => "after",
        ScheduleRecord::Every(_) => "every",
        ScheduleRecord::Daily(_) => "daily",
        ScheduleRecord::Weekly(_) => "weekly",
        ScheduleRecord::Cron(_) => "cron",
    }
}

/// 任务视图（REST 出参；`SessionTask` 的扁平化）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TaskDto {
    pub id: String,
    pub title: String,
    pub prompt: String,
    /// `single` = 绑定会话；`independent` = 到期新建会话。
    pub session_mode: String,
    /// 绑定会话 id（independent 递送前为空）。
    pub session_id: String,
    pub enabled: bool,
    /// 频率类型：at / after / every / daily / weekly / cron。
    pub kind: String,
    /// 下次触发（RFC3339；与下次递送目标一致）。
    pub scheduled_at: String,
    /// 最近一次递送时间（无则 null）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_delivered_at: Option<String>,
}

impl TaskDto {
    fn of(task: &ScheduleTask) -> Self {
        Self {
            id: task.record.id().to_string(),
            title: task.record.title().to_string(),
            prompt: task.record.prompt().to_string(),
            session_mode: match task.session_mode.as_deref() {
                Some("independent") => "independent".to_string(),
                _ => "single".to_string(),
            },
            session_id: task.session_id.clone(),
            enabled: task.status == TaskStatus::Active,
            kind: kind_of(&task.record).to_string(),
            scheduled_at: task.record.scheduled_at().to_string(),
            last_delivered_at: task
                .last_delivery
                .as_ref()
                .map(|receipt| receipt.delivered_at.clone()),
        }
    }
}

/// `GET /v1/tasks`：全部任务（跨会话聚合，按下次触发时间升序）。
pub(crate) async fn list_tasks(
    State(state): State<RouterState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut tasks: Vec<TaskDto> = store(&state).all().iter().map(TaskDto::of).collect();
    tasks.sort_by(|a, b| a.scheduled_at.cmp(&b.scheduled_at));
    Ok(Json(serde_json::json!({ "tasks": tasks })))
}

/// `POST /v1/tasks` 请求体。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateTaskRequest {
    pub title: String,
    pub prompt: String,
    /// 缺省 `single`。
    #[serde(default)]
    pub session_mode: Option<String>,
    /// single 模式的绑定会话；independent 忽略（调度器到期新建）。
    #[serde(default)]
    pub session_id: Option<String>,
    /// 恰好一个频率（单 selector 语义对齐 ScheduleCreateRequest）。
    pub frequency: FrequencySpec,
}

/// 频率规格（REST 入参；时区缺省由前端给本地时区）。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub(crate) enum FrequencySpec {
    /// 一次性：RFC3339（`YYYY-MM-DDTHH:mm:ss(±HH:MM|Z)`）。
    At { instant: String },
    /// 间隔循环（秒，下限 60）。
    Every { seconds: i64 },
    /// 每天定点。
    Daily { time: String, time_zone: String },
    /// 每周定点（weekday 0=周日 … 6=周六，对齐 cron/JS getDay）。
    Weekly { time: String, time_zone: String, weekdays: Vec<u32> },
    /// cron 表达式。
    Cron { expression: String, time_zone: String },
}

/// `POST /v1/tasks`：创建任务。
pub(crate) async fn create_task(
    State(state): State<RouterState>,
    Json(body): Json<CreateTaskRequest>,
) -> Result<Json<TaskDto>, ApiError> {
    let title = body.title.trim();
    let prompt = body.prompt.trim();
    if title.is_empty() || prompt.is_empty() {
        return Err(ApiError::bad_request("task_fields_missing", "`title` 与 `prompt` 均不能为空"));
    }
    let mode = match body.session_mode.as_deref() {
        None | Some("single") => "single",
        Some("independent") => "independent",
        Some(other) => {
            return Err(ApiError::bad_request(
                "unknown_session_mode",
                format!("`sessionMode` 仅支持 `single` / `independent`，收到 `{other}`"),
            ));
        }
    };
    let session_id = match mode {
        "independent" => String::new(),
        // 绑定会话须存在（可恢复；磁盘 locate 失败 = 不存在）。
        _ => {
            let session_id = body
                .session_id
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .ok_or_else(|| {
                    ApiError::bad_request("session_id_missing", "single 模式须携带 `sessionId`")
                })?;
            match state.registry.open(session_id, true).await {
                Ok(Some(_)) => session_id.to_string(),
                Ok(None) => {
                    return Err(ApiError::not_found(
                        "session_not_found",
                        format!("会话 `{session_id}` 不存在"),
                    ));
                }
                Err(error) => {
                    return Err(ApiError::internal(format!("会话恢复失败: {error}")));
                }
            }
        }
    };
    // 单 selector 频率校验 + id 分配 + 落盘（复用 ScheduleService）。
    let service = ScheduleService::new(store(&state));
    let request = ScheduleCreateRequest {
        prompt: prompt.to_string(),
        title: title.to_string(),
        after_seconds: None,
        at: None,
        every_seconds: None,
        daily: None,
        weekly: None,
        cron: None,
    };
    let request = match &body.frequency {
        FrequencySpec::At { instant } => ScheduleCreateRequest {
            at: Some(AtInput::Instant(instant.clone())),
            ..request
        },
        FrequencySpec::Every { seconds } => ScheduleCreateRequest {
            every_seconds: Some(*seconds),
            ..request
        },
        FrequencySpec::Daily { time, time_zone } => ScheduleCreateRequest {
            daily: Some(DailyInput { time: time.clone(), time_zone: time_zone.clone() }),
            ..request
        },
        FrequencySpec::Weekly { time, time_zone, weekdays } => ScheduleCreateRequest {
            weekly: Some(WeeklyInput {
                time: time.clone(),
                time_zone: time_zone.clone(),
                weekdays: weekdays.clone(),
            }),
            ..request
        },
        FrequencySpec::Cron { expression, time_zone } => ScheduleCreateRequest {
            cron: Some(CronInput { expression: expression.clone(), time_zone: time_zone.clone() }),
            ..request
        },
    };
    let record = service
        .create(&session_id, request)
        .map_err(|error| ApiError::bad_request(error.code(), runtime::schedule_error_text(&error)))?;
    // 回填 session_mode（service.create 不知道全局任务面语义）。
    let store = store(&state);
    let task = store
        .get(record.id())
        .ok_or_else(|| ApiError::internal("任务创建后不可见"))?;
    let task = ScheduleTask {
        session_mode: Some(mode.to_string()),
        ..task
    };
    store.put(task.clone()).map_err(|error| ApiError::internal(format!("session_mode 写入失败: {error}")))?;
    Ok(Json(TaskDto::of(&task)))
}

/// `PATCH /v1/tasks/{id}` 请求体：启停 / 编辑（字段均可选，至少给一项）。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateTaskRequest {
    pub enabled: Option<bool>,
    pub title: Option<String>,
    pub prompt: Option<String>,
    /// 新频率；缺省表示保持原时序。
    pub frequency: Option<FrequencySpec>,
}

/// `FrequencySpec` → 领域时序变更（与创建同一套 selector 语义）。
fn timing_change_of(spec: &FrequencySpec) -> ScheduleTimingChange {
    match spec {
        FrequencySpec::At { instant } => ScheduleTimingChange::At {
            at: AtInput::Instant(instant.clone()),
        },
        FrequencySpec::Every { seconds } => ScheduleTimingChange::Every { every_seconds: *seconds },
        FrequencySpec::Daily { time, time_zone } => ScheduleTimingChange::Daily {
            daily: DailyInput { time: time.clone(), time_zone: time_zone.clone() },
        },
        FrequencySpec::Weekly { time, time_zone, weekdays } => ScheduleTimingChange::Weekly {
            weekly: WeeklyInput {
                time: time.clone(),
                time_zone: time_zone.clone(),
                weekdays: weekdays.clone(),
            },
        },
        FrequencySpec::Cron { expression, time_zone } => ScheduleTimingChange::Cron {
            cron: CronInput { expression: expression.clone(), time_zone: time_zone.clone() },
        },
    }
}

/// `PATCH /v1/tasks/{id}`：启停与编辑（标题 / 指令 / 频率）。
///
/// 编辑走 [`ScheduleService::update`]（id 稳定、时序变化按当前时刻重算）；
/// 停用任务允许编辑且保持停用状态。启停语义：停用保留原时序（重新启用即
/// 恢复原计划）；一次性任务过期后重新启用会在下一轮 tick 立即触发。
pub(crate) async fn update_task(
    State(state): State<RouterState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateTaskRequest>,
) -> Result<Json<TaskDto>, ApiError> {
    let store = store(&state);
    let task = store
        .get(&id)
        .ok_or_else(|| ApiError::not_found("task_not_found", format!("任务 `{id}` 不存在")))?;
    let has_edit = body.title.is_some() || body.prompt.is_some() || body.frequency.is_some();
    let task = if has_edit {
        let service = ScheduleService::new(store.clone());
        let content = ScheduleUpdateContent { title: body.title.clone(), prompt: body.prompt.clone() };
        let change = body.frequency.as_ref().map(timing_change_of);
        match service.update(
            &task.session_id,
            &id,
            &task.record,
            change,
            content,
            InactivePolicy::Allow,
        ) {
            ScheduleUpdateResult::Updated { record } => ScheduleTask { record, ..task },
            ScheduleUpdateResult::NotFound { .. } => {
                return Err(ApiError::not_found("task_not_found", format!("任务 `{id}` 不存在")));
            }
            ScheduleUpdateResult::Conflict { .. } => {
                return Err(ApiError::conflict("task_conflict", "任务在此期间已被修改，请刷新后重试"));
            }
            ScheduleUpdateResult::Ended { .. } => {
                return Err(ApiError::conflict("task_ended", "任务已结束，无法编辑"));
            }
            ScheduleUpdateResult::Error(error) => {
                return Err(ApiError::bad_request(
                    error.code(),
                    runtime::schedule_error_text(&error),
                ));
            }
        }
    } else {
        task
    };
    let task = match body.enabled {
        Some(enabled) => {
            let task = ScheduleTask {
                status: if enabled { TaskStatus::Active } else { TaskStatus::Inactive },
                ..task
            };
            store
                .put(task.clone())
                .map_err(|error| ApiError::internal(format!("任务写入失败: {error}")))?;
            task
        }
        None => task,
    };
    Ok(Json(TaskDto::of(&task)))
}

/// `DELETE /v1/tasks/{id}`。
pub(crate) async fn delete_task(
    State(state): State<RouterState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let deleted = store(&state)
        .delete(&id)
        .map_err(|error| ApiError::internal(format!("任务删除失败: {error}")))?;
    if !deleted {
        return Err(ApiError::not_found("task_not_found", format!("任务 `{id}` 不存在")));
    }
    Ok(Json(serde_json::json!({ "id": id })))
}

/// `POST /v1/tasks/{id}/run`：立即执行（独立记账，语义等同到期递送——
/// 循环任务推进到下次触发，一次性任务转 inactive）。
pub(crate) async fn run_task(
    State(state): State<RouterState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let store = store(&state);
    let mut task = store
        .get(&id)
        .ok_or_else(|| ApiError::not_found("task_not_found", format!("任务 `{id}` 不存在")))?;
    // 目标会话：independent 未物化 → 新建并回填；single → 恢复绑定会话。
    let session = if task.session_mode.as_deref() == Some("independent") && task.session_id.is_empty() {
        let session = state
            .registry
            .create(None, None, true, Some(state.registry.workspace_root().display().to_string()))
            .await
            .map_err(|error| ApiError::internal(format!("会话创建失败: {error}")))?;
        store
            .put(ScheduleTask { session_id: session.id.clone(), ..task.clone() })
            .map_err(|error| ApiError::internal(format!("session_id 回填失败: {error}")))?;
        // 重读最新 task：deliver 记账以旧快照展开会覆盖 session_id 回填。
        task = store
            .get(&id)
            .ok_or_else(|| ApiError::internal("任务回填后不可见"))?;
        session
    } else {
        state
            .registry
            .open(&task.session_id, true)
            .await
            .map_err(|error| ApiError::internal(format!("会话恢复失败: {error}")))?
            .ok_or_else(|| {
                ApiError::not_found(
                    "session_not_found",
                    format!("绑定会话 `{}` 不存在", task.session_id),
                )
            })?
    };
    let message_id =
        runtime::deliver_task_now(&store, &task, Default::default(), &SessionDeliver(session))
            .await
            .map_err(|error| ApiError::internal(format!("任务递送失败: {error}")))?;
    Ok(Json(serde_json::json!({ "id": id, "messageId": message_id })))
}
