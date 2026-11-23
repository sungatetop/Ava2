//! schedule 工具面：schedule_create / schedule_list / schedule_update / schedule_delete。
//!
//! - 4 工具共享 Host 存储 ScheduleService；会话 id 经 `wire_session` 延迟注入
//!   （onion 单 agent，无 subagent 拒绝分支）；
//! - 每个调用作用于调用方会话的提醒（任务绑定当前会话 id）；
//! - 输出为模型可见 JSON（view：record + state + delivery_mode；错误：code + message）。

use std::sync::{Arc, OnceLock};

use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use crate::session::SessionLog;
use crate::tools::ToolPort;
use crate::types::{ContentBlock, ToolCall, ToolDefinition};
use crate::waterfall::BoxFuture;

use super::service::ScheduleService;
use super::types::{
    AtInput, CronInput, DailyInput, ScheduleCreateRequest, ScheduleTimingChange,
    ScheduleUpdateContent, WeeklyInput,
};

/// 工具族共享句柄。
#[derive(Clone)]
struct ScheduleHandles {
    service: Arc<ScheduleService>,
    session: Arc<OnceLock<Arc<SessionLog>>>,
}

impl ScheduleHandles {
    /// 当前会话 id（未接线时空串；onion 单 agent，装配后恒接线）。
    fn session_id(&self) -> String {
        self.session.get().map(|s| s.session_id().to_string()).unwrap_or_default()
    }
}

/// 工具族载体：注册 4 工具进注册表。
pub struct ScheduleToolSet {
    handles: ScheduleHandles,
}

impl ScheduleToolSet {
    pub fn new(service: Arc<ScheduleService>) -> Self {
        Self {
            handles: ScheduleHandles {
                service,
                session: Arc::new(OnceLock::new()),
            },
        }
    }

    /// 延迟接线：锚定当前会话（取 session id 作为任务绑定）。
    pub fn wire_session(&self, session: Arc<SessionLog>) {
        self.handles.session.set(session).ok();
    }

    pub fn register_all(&self, registry: &mut crate::tools::ToolRegistry) {
        let handles = self.handles.clone();
        registry.register(Arc::new(ScheduleCreateTool { handles: handles.clone() }));
        registry.register(Arc::new(ScheduleListTool { handles: handles.clone() }));
        registry.register(Arc::new(ScheduleUpdateTool { handles: handles.clone() }));
        registry.register(Arc::new(ScheduleDeleteTool { handles }));
    }
}

/// 构造模型可见 JSON 文本块。
fn text_block(value: impl Into<Value>) -> ContentBlock {
    ContentBlock::Text { text: value.into().to_string() }
}

/// 把视图序列化为模型可见 JSON（record flatten + state + delivery_mode）。
fn view_value(view: &super::types::ScheduleView) -> Value {
    serde_json::to_value(&view.record).unwrap_or(Value::Null)
}

/// schedule_create：创建提醒。
struct ScheduleCreateTool {
    handles: ScheduleHandles,
}

impl ToolPort for ScheduleCreateTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "schedule_create".to_string(),
            description: "Create a reminder in the current session that delivers prompt when it becomes due. \
                Supply exactly one timing parameter: after_seconds, at, every_seconds, daily, weekly, or cron. \
                Local times that do not exist in the zone are skipped; repeated local times fire once, at the earlier instant."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "prompt": { "type": "string", "description": "Reminder content to present when the target becomes due." },
                    "title": { "type": "string", "description": "Concise task name, at most 120 characters." },
                    "after_seconds": { "type": "number", "description": "Delay in whole seconds." },
                    "at": { "description": "Absolute target: RFC 3339 with offset, or {date, time, time_zone}." },
                    "every_seconds": { "type": "number", "description": "Fixed-rate interval in whole seconds, at least 60." },
                    "daily": { "type": "object", "description": "Every day at a local time: {time, time_zone}." },
                    "weekly": { "type": "object", "description": "On weekdays at a local time: {time, time_zone, weekdays}." },
                    "cron": { "type": "object", "description": "Five-field Vixie cron: {expression, time_zone}." }
                },
                "required": ["prompt", "title"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let handles = self.handles.clone();
        Box::pin(async move {
            let session_id = handles.session_id();
            let request = parse_create_request(&call)?;
            match handles.service.create(&session_id, request) {
                Ok(record) => {
                    let now = crate::session::epoch_millis() as i64;
                    let view = super::domain::schedule_view(&record, now);
                    let mut value = view_value(&view);
                    if let Some(obj) = value.as_object_mut() {
                        obj.insert("state".to_string(), json!(state_label(view.state)));
                        obj.insert("delivery_mode".to_string(), json!("host"));
                    }
                    Ok(vec![text_block(value)])
                }
                Err(e) => Ok(vec![text_block(json!({ "code": e.code(), "message": e.message() }))]),
            }
        })
    }
}

fn state_label(state: super::types::ScheduleState) -> &'static str {
    match state {
        super::types::ScheduleState::Scheduled => "scheduled",
        super::types::ScheduleState::Overdue => "overdue",
    }
}

/// 从工具调用解析创建请求（校验恰好一个 selector）。
fn parse_create_request(call: &ToolCall) -> Result<ScheduleCreateRequest, String> {
    let input = &call.input;
    let prompt = input.get("prompt").and_then(Value::as_str).unwrap_or("").to_string();
    let title = input.get("title").and_then(Value::as_str).unwrap_or("").to_string();
    let selectors = [
        input.get("after_seconds").is_some(),
        input.get("at").is_some(),
        input.get("every_seconds").is_some(),
        input.get("daily").is_some(),
        input.get("weekly").is_some(),
        input.get("cron").is_some(),
    ]
    .into_iter()
    .filter(|b| *b)
    .count();
    if selectors != 1 {
        return Err("schedule_create accepts exactly one of after_seconds, at, every_seconds, daily, weekly, or cron."
            .to_string());
    }
    let after_seconds = input.get("after_seconds").and_then(Value::as_i64);
    let at = input.get("at").and_then(parse_at_value);
    let every_seconds = input.get("every_seconds").and_then(Value::as_i64);
    let daily = input.get("daily").and_then(parse_daily_value);
    let weekly = input.get("weekly").and_then(parse_weekly_value);
    let cron = input.get("cron").and_then(parse_cron_value);
    Ok(ScheduleCreateRequest {
        prompt,
        title,
        after_seconds,
        at,
        every_seconds,
        daily,
        weekly,
        cron,
    })
}

fn parse_at_value(v: &Value) -> Option<AtInput> {
    match v {
        Value::String(s) => Some(AtInput::Instant(s.clone())),
        Value::Object(map) => Some(AtInput::Local {
            date: map.get("date").and_then(Value::as_str)?.to_string(),
            time: map.get("time").and_then(Value::as_str)?.to_string(),
            time_zone: map.get("time_zone").and_then(Value::as_str)?.to_string(),
        }),
        _ => None,
    }
}

fn parse_daily_value(v: &Value) -> Option<DailyInput> {
    let map = v.as_object()?;
    Some(DailyInput {
        time: map.get("time")?.as_str()?.to_string(),
        time_zone: map.get("time_zone")?.as_str()?.to_string(),
    })
}

fn parse_weekly_value(v: &Value) -> Option<WeeklyInput> {
    let map = v.as_object()?;
    let weekdays = map
        .get("weekdays")?
        .as_array()?
        .iter()
        .filter_map(Value::as_u64)
        .map(|n| n as u32)
        .collect();
    Some(WeeklyInput {
        time: map.get("time")?.as_str()?.to_string(),
        time_zone: map.get("time_zone")?.as_str()?.to_string(),
        weekdays,
    })
}

fn parse_cron_value(v: &Value) -> Option<CronInput> {
    let map = v.as_object()?;
    Some(CronInput {
        expression: map.get("expression")?.as_str()?.to_string(),
        time_zone: map.get("time_zone")?.as_str()?.to_string(),
    })
}

/// schedule_list：列出 active 提醒。
struct ScheduleListTool {
    handles: ScheduleHandles,
}

impl ToolPort for ScheduleListTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "schedule_list".to_string(),
            description: "List the active reminders in the current session.".to_string(),
            parameters: json!({ "type": "object", "properties": {}, "required": [] }),
        }
    }

    fn execute(&self, _call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let handles = self.handles.clone();
        Box::pin(async move {
            let views = handles.service.list_views(&handles.session_id());
            let values: Vec<Value> = views
                .iter()
                .map(|view| {
                    let mut value = view_value(view);
                    if let Some(obj) = value.as_object_mut() {
                        obj.insert("state".to_string(), json!(state_label(view.state)));
                        obj.insert("delivery_mode".to_string(), json!("host"));
                    }
                    value
                })
                .collect();
            Ok(vec![text_block(Value::Array(values))])
        })
    }
}

/// schedule_delete：删除提醒。
struct ScheduleDeleteTool {
    handles: ScheduleHandles,
}

impl ToolPort for ScheduleDeleteTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "schedule_delete".to_string(),
            description: "Delete a reminder in the current session, active or inactive. \
                Deletion does not retract a reminder message that is already queued."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": { "id": { "type": "string", "description": "Exact schedule id." } },
                "required": ["id"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let handles = self.handles.clone();
        Box::pin(async move {
            let id = call.input.get("id").and_then(Value::as_str).unwrap_or("").to_string();
            if id.trim().is_empty() {
                return Ok(vec![text_block(json!({
                    "code": "invalid_rule",
                    "message": "schedule_delete id must be non-empty without surrounding whitespace."
                }))]);
            }
            match handles.service.delete(&handles.session_id(), &id) {
                super::types::ScheduleDeleteResult::Deleted { id } => {
                    Ok(vec![text_block(json!({ "id": id, "deleted": true }))])
                }
                super::types::ScheduleDeleteResult::NotFound { id } => Ok(vec![text_block(json!({
                    "id": id, "deleted": false, "code": "schedule_not_found"
                }))]),
            }
        })
    }
}

/// schedule_update：compare-and-update。
struct ScheduleUpdateTool {
    handles: ScheduleHandles,
}

impl ToolPort for ScheduleUpdateTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "schedule_update".to_string(),
            description: "Change a reminder in place, keeping its id. Supply a new title, prompt, \
                or at most one timing parameter; omitted fields keep their stored values."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Schedule id returned by schedule_list." },
                    "title": { "type": "string", "description": "New task name." },
                    "prompt": { "type": "string", "description": "New reminder content." },
                    "at": { "description": "Absolute target: RFC 3339 with offset, or {date, time, time_zone}." },
                    "every_seconds": { "type": "number", "description": "Fixed-rate interval in whole seconds, at least 60." },
                    "daily": { "type": "object", "description": "Every day at a local time: {time, time_zone}." },
                    "weekly": { "type": "object", "description": "On weekdays at a local time: {time, time_zone, weekdays}." },
                    "cron": { "type": "object", "description": "Five-field Vixie cron: {expression, time_zone}." }
                },
                "required": ["id"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let handles = self.handles.clone();
        Box::pin(async move {
            let input = &call.input;
            let id = input.get("id").and_then(Value::as_str).unwrap_or("").to_string();
            if id.trim().is_empty() {
                return Ok(vec![text_block(json!({
                    "code": "invalid_rule",
                    "message": "schedule_update id must be non-empty without surrounding whitespace."
                }))]);
            }
            // 校验至多一个时序参数。
            let selectors = [
                input.get("at").is_some(),
                input.get("every_seconds").is_some(),
                input.get("daily").is_some(),
                input.get("weekly").is_some(),
                input.get("cron").is_some(),
            ]
            .into_iter()
            .filter(|b| *b)
            .count();
            if selectors > 1 {
                return Ok(vec![text_block(json!({
                    "code": "invalid_selector",
                    "message": "schedule_update accepts at most one of at, every_seconds, daily, weekly, or cron."
                }))]);
            }
            let has_content = {
                let title = input.get("title").and_then(Value::as_str).map(str::trim).filter(|t| !t.is_empty());
                let prompt = input.get("prompt").and_then(Value::as_str).map(str::trim).filter(|p| !p.is_empty());
                title.is_some() || prompt.is_some()
            };
            if selectors == 0 && !has_content {
                return Ok(vec![text_block(json!({
                    "code": "invalid_selector",
                    "message": "schedule_update needs a new title, prompt, or one of at, every_seconds, daily, weekly, or cron."
                }))]);
            }

            let session_id = handles.session_id();
            let current = match handles.service.find(&id) {
                Some(r) => r,
                None => {
                    return Ok(vec![text_block(json!({
                        "id": id, "updated": false, "code": "schedule_not_found"
                    }))]);
                }
            };
            let change = timing_change_from(input);
            let content = ScheduleUpdateContent {
                title: input.get("title").and_then(Value::as_str).map(str::to_string),
                prompt: input.get("prompt").and_then(Value::as_str).map(str::to_string),
            };
            match handles.service.update(
                &session_id,
                &id,
                &current,
                change,
                content,
                super::types::InactivePolicy::Reject,
            ) {
                super::types::ScheduleUpdateResult::Updated { record } => {
                    let now = crate::session::epoch_millis() as i64;
                    let view = super::domain::schedule_view(&record, now);
                    let mut value = view_value(&view);
                    if let Some(obj) = value.as_object_mut() {
                        obj.insert("state".to_string(), json!(state_label(view.state)));
                        obj.insert("delivery_mode".to_string(), json!("host"));
                    }
                    Ok(vec![text_block(value)])
                }
                super::types::ScheduleUpdateResult::NotFound { id } => {
                    Ok(vec![text_block(json!({ "id": id, "updated": false, "code": "schedule_not_found" }))])
                }
                super::types::ScheduleUpdateResult::Conflict { id } => {
                    Ok(vec![text_block(json!({ "id": id, "updated": false, "code": "schedule_conflict" }))])
                }
                super::types::ScheduleUpdateResult::Ended { id } => {
                    Ok(vec![text_block(json!({ "id": id, "updated": false, "code": "schedule_ended" }))])
                }
                super::types::ScheduleUpdateResult::Error(e) => {
                    Ok(vec![text_block(json!({ "code": e.code(), "message": e.message() }))])
                }
            }
        })
    }
}

/// 从更新调用提取时序变更（至多一个）。
fn timing_change_from(input: &Value) -> Option<ScheduleTimingChange> {
    if let Some(v) = input.get("at") {
        return parse_at_value(v).map(|at| ScheduleTimingChange::At { at });
    }
    if let Some(every) = input.get("every_seconds").and_then(Value::as_i64) {
        return Some(ScheduleTimingChange::Every { every_seconds: every });
    }
    if let Some(v) = input.get("daily") {
        return parse_daily_value(v).map(|daily| ScheduleTimingChange::Daily { daily });
    }
    if let Some(v) = input.get("weekly") {
        return parse_weekly_value(v).map(|weekly| ScheduleTimingChange::Weekly { weekly });
    }
    if let Some(v) = input.get("cron") {
        return parse_cron_value(v).map(|cron| ScheduleTimingChange::Cron { cron });
    }
    None
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
