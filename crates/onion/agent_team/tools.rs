//! agent-team 9 工具注册。
//!
//! 工具清单：
//! - `spawn_teammate`：创建具名 durable 队友（仅 Lead）；
//! - `send_message` / `list_agents` / `interrupt_agent`：共享名三件（装配期
//!   覆盖 subagent 控制三件，同名互斥由装配决定）；
//! - `wait_agent`：等待下一次团队变化；
//! - `team_task_create` / `team_task_list` / `team_task_get` / `team_task_update`：
//!   durable 共享任务板。
//!
//! 身份注入而非自报：所有工具经 `caller` 槽位（= Lead session id）解析调用者
//! 成员身份，模型不能伪造他人身份。装配完成后由 `wire_caller` 注入。
//!
//! 工具仅注册进 Lead 会话工具面；队友（subagent 会话）工具面不含本工具族
//! （child 不获得嵌套 Team 身份，防递归）。

use std::sync::{Arc, OnceLock};

use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::service::TeamService;
use super::types::{
    CreateTeamTaskRequest, SendTeamMessageRequest, SpawnTeammateRequest, TeamContext,
    TeamMemberStatus, TeamTaskAction, TeamTaskStatus, UpdateTeamTaskRequest,
};
use super::TeamError;
use crate::approval::ToolRisk;
use crate::tools::ToolPort;
use crate::types::{ContentBlock, ToolCall, ToolDefinition};
use crate::waterfall::BoxFuture;

/// 工具族共享句柄：TeamService + caller 槽位（= Lead session id）。
#[derive(Clone)]
struct TeamHandles {
    service: Arc<TeamService>,
    caller: Arc<OnceLock<String>>,
    /// fresh 队友 provider（默认 "spawn"）。
    fresh_provider: String,
    /// fork 队友 provider（默认 "fork"）。
    fork_provider: String,
}

impl TeamHandles {
    /// 解析调用者 id（未接线 → fail-closed）。
    fn caller_id(&self) -> Result<String, String> {
        self.caller
            .get()
            .cloned()
            .ok_or_else(|| "agent-team tools not wired (missing caller)".to_string())
    }
}

/// 工具族载体：持有共享句柄，注册 9 工具进注册表。
pub struct TeamToolSet {
    handles: TeamHandles,
}

impl TeamToolSet {
    /// 构造（装配期注册 9 工具进 Lead 会话注册表）。
    pub fn new(service: Arc<TeamService>) -> Self {
        Self::with_providers(service, "spawn".to_string(), "fork".to_string())
    }

    /// 构造（可指定 fresh/fork provider）。
    pub fn with_providers(service: Arc<TeamService>, fresh_provider: String, fork_provider: String) -> Self {
        Self {
            handles: TeamHandles {
                service,
                caller: Arc::new(OnceLock::new()),
                fresh_provider,
                fork_provider,
            },
        }
    }

    /// 延迟接线：注入 Lead session id（父会话 Runtime 装配完成后调用）。
    pub fn wire_caller(&self, lead_id: String) {
        let _ = self.handles.caller.set(lead_id);
    }

    /// 注册 9 工具进注册表。
    pub fn register_all(&self, registry: &mut crate::tools::ToolRegistry) {
        let handles = self.handles.clone();
        registry.register(Arc::new(SpawnTeammateTool { handles: handles.clone() }));
        registry.register(Arc::new(SendTeamMessageTool { handles: handles.clone() }));
        registry.register(Arc::new(ListAgentsTool { handles: handles.clone() }));
        registry.register(Arc::new(WaitAgentTool { handles: handles.clone() }));
        registry.register(Arc::new(InterruptAgentTool { handles: handles.clone() }));
        registry.register(Arc::new(TeamTaskCreateTool { handles: handles.clone() }));
        registry.register(Arc::new(TeamTaskListTool { handles: handles.clone() }));
        registry.register(Arc::new(TeamTaskGetTool { handles: handles.clone() }));
        registry.register(Arc::new(TeamTaskUpdateTool { handles }));
    }
}

/// 把 TeamError 渲染为工具错误消息（保留错误码语义）。
fn team_err(e: &TeamError) -> String {
    e.to_string()
}

/// spawn_teammate：创建具名 durable 队友（仅 Lead）。
struct SpawnTeammateTool {
    handles: TeamHandles,
}

impl ToolPort for SpawnTeammateTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "spawn_teammate".to_string(),
            description: "Create one named, durable teammate. Only the Team Lead may call this tool. Input: {\"name\": string, \"description\": string, \"prompt\": string, \"context\": \"fresh\"|\"fork\"}.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Unique lower-kebab-case teammate name." },
                    "description": { "type": "string", "description": "Short description of the delegated responsibility." },
                    "prompt": { "type": "string", "description": "Complete initial task for the teammate." },
                    "context": { "type": "string", "enum": ["fresh", "fork"], "description": "fresh starts without Lead history; fork inherits completed Lead turns. Defaults to fresh." }
                },
                "required": ["name", "description", "prompt"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let handles = self.handles.clone();
        Box::pin(async move {
            let caller = handles.caller_id()?;
            let name = call.input.get("name").and_then(Value::as_str).unwrap_or("").to_string();
            let description = call.input.get("description").and_then(Value::as_str).unwrap_or("").to_string();
            let prompt = call.input.get("prompt").and_then(Value::as_str).unwrap_or("").to_string();
            if name.is_empty() || description.is_empty() || prompt.is_empty() {
                return Err("name, description and prompt are required".to_string());
            }
            let context = match call.input.get("context").and_then(Value::as_str).unwrap_or("fresh") {
                "fork" => TeamContext::Fork,
                _ => TeamContext::Fresh,
            };
            // provider 由装配配置决定（fresh/fork provider），不暴露给模型。
            let provider = match context {
                TeamContext::Fresh => handles.fresh_provider.clone(),
                TeamContext::Fork => handles.fork_provider.clone(),
            };
            let req = SpawnTeammateRequest {
                name,
                description,
                prompt,
                context,
                provider,
            };
            let result = handles.service.spawn_teammate(&caller, &req).await.map_err(|e| team_err(&e))?;
            let view = result.member;
            let json = json!({ "member": member_json(&view) });
            Ok(vec![ContentBlock::Text { text: serde_json::to_string(&json).unwrap_or_default() }])
        })
    }

    fn modes(&self) -> Option<Vec<String>> {
        Some(vec!["work".to_string()])
    }

    fn risk(&self) -> ToolRisk {
        ToolRisk::WorkspaceWrite
    }
}

/// send_message：向另一成员投递消息（target 为成员名或 "lead"）。
struct SendTeamMessageTool {
    handles: TeamHandles,
}

impl ToolPort for SendTeamMessageTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "send_message".to_string(),
            description: "Send a message to another team member. Input: {\"target\": string, \"message\": string}. target is a teammate name or \"lead\".".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "target": { "type": "string", "description": "teammate name or \"lead\"" },
                    "message": { "type": "string", "description": "message text to deliver" }
                },
                "required": ["target", "message"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let handles = self.handles.clone();
        Box::pin(async move {
            let caller = handles.caller_id()?;
            let target = call.input.get("target").and_then(Value::as_str).unwrap_or("").to_string();
            let message = call.input.get("message").and_then(Value::as_str).unwrap_or("").to_string();
            if target.is_empty() || message.is_empty() {
                return Err("target and message are required".to_string());
            }
            let req = SendTeamMessageRequest {
                target,
                content: vec![ContentBlock::Text { text: message }],
            };
            let result = handles.service.send_message(&caller, &req).await.map_err(|e| team_err(&e))?;
            let json = json!({ "sent": result.sent });
            Ok(vec![ContentBlock::Text { text: serde_json::to_string(&json).unwrap_or_default() }])
        })
    }

    fn modes(&self) -> Option<Vec<String>> {
        Some(vec!["work".to_string()])
    }

    fn risk(&self) -> ToolRisk {
        ToolRisk::WorkspaceWrite
    }
}

/// list_agents：列出 Lead + 全部 durable 队友。
struct ListAgentsTool {
    handles: TeamHandles,
}

impl ToolPort for ListAgentsTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "list_agents".to_string(),
            description: "List the team Lead and all durable teammates (id | name | role | status). Input: {}.".to_string(),
            parameters: json!({ "type": "object", "properties": {}, "required": [] }),
        }
    }

    fn execute(&self, _call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let handles = self.handles.clone();
        Box::pin(async move {
            let caller = handles.caller_id()?;
            // 校验 membership（fail-closed：非成员不可列团队）。
            let _ = handles.service.membership(&caller).await.map_err(|e| team_err(&e))?;
            let members = handles.service.list_members().await.map_err(|e| team_err(&e))?;
            let rows: Vec<Value> = members.iter().map(member_json).collect();
            let json = serde_json::to_string(&rows).unwrap_or_default();
            Ok(vec![ContentBlock::Text { text: json }])
        })
    }

    fn risk(&self) -> ToolRisk {
        ToolRisk::ReadOnly
    }
}

/// wait_agent：等待下一次团队变化（roster/task/消息/状态边）。
struct WaitAgentTool {
    handles: TeamHandles,
}

impl ToolPort for WaitAgentTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "wait_agent".to_string(),
            description: "Wait for the next team change (roster/task/message/status edge). Input: {\"timeout_ms\": number} (10s..1h).".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "timeout_ms": { "type": "number", "description": "max wait in milliseconds (10000..3600000)" }
                },
                "required": []
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let handles = self.handles.clone();
        Box::pin(async move {
            let caller = handles.caller_id()?;
            let _ = handles.service.membership(&caller).await.map_err(|e| team_err(&e))?;
            let timeout_ms = call.input.get("timeout_ms").and_then(Value::as_u64).unwrap_or(30_000);
            // 无活跃 peer 短路（运行中/筹备中状态都不算空闲）。
            // 在注册 waiter 前同步判定，避免丢失唯一的 peer-status 边。
            let has_active_peer = {
                let members = handles.service.list_members().await.map_err(|e| team_err(&e))?;
                members.iter().any(|m| {
                    m.id != caller
                        && (m.status == TeamMemberStatus::Running || m.status == TeamMemberStatus::Provisioning)
                })
            };
            if !has_active_peer {
                let json = json!({
                    "timedOut": false,
                    "noProgress": {
                        "reason": "no-active-peer",
                        "message": "No other Team member is running or provisioning. wait_agent cannot make progress or wake inactive teammates. Re-list with list_agents and team_task_list, then use send_message to wake each required inactive teammate before waiting again."
                    }
                });
                return Ok(vec![ContentBlock::Text { text: serde_json::to_string(&json).unwrap_or_default() }]);
            }
            let result = handles.service.wait_for_change(timeout_ms).await.map_err(|e| team_err(&e))?;
            let json = json!({ "timedOut": result.timed_out });
            Ok(vec![ContentBlock::Text { text: serde_json::to_string(&json).unwrap_or_default() }])
        })
    }

    fn risk(&self) -> ToolRisk {
        ToolRisk::ReadOnly
    }
}

/// interrupt_agent：中断队友当前 turn，保留 inbox（仅 Lead）。
struct InterruptAgentTool {
    handles: TeamHandles,
}

impl ToolPort for InterruptAgentTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "interrupt_agent".to_string(),
            description: "Interrupt one teammate's current turn while preserving its pending inbox. Team Lead only. Input: {\"target\": string}.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "target": { "type": "string", "description": "Teammate target returned by spawn_teammate or list_agents." }
                },
                "required": ["target"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let handles = self.handles.clone();
        Box::pin(async move {
            let caller = handles.caller_id()?;
            let target = call.input.get("target").and_then(Value::as_str).unwrap_or("").to_string();
            if target.is_empty() {
                return Err("target is required".to_string());
            }
            let previous = handles.service.interrupt(&caller, &target).await.map_err(|e| team_err(&e))?;
            let json = json!({ "previousStatus": previous });
            Ok(vec![ContentBlock::Text { text: serde_json::to_string(&json).unwrap_or_default() }])
        })
    }

    fn modes(&self) -> Option<Vec<String>> {
        Some(vec!["work".to_string()])
    }

    fn risk(&self) -> ToolRisk {
        ToolRisk::WorkspaceWrite
    }
}

/// team_task_create：创建无主 pending 任务。
struct TeamTaskCreateTool {
    handles: TeamHandles,
}

impl ToolPort for TeamTaskCreateTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "team_task_create".to_string(),
            description: "Create an unowned pending team task. Input: {\"subject\": string, \"description\": string, \"blocked_by\": string[], \"write_scopes\": string[]}.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "subject": { "type": "string", "description": "task subject (<=200)" },
                    "description": { "type": "string", "description": "task description (<=16384)" },
                    "blocked_by": { "type": "array", "items": { "type": "string" }, "description": "blocking task ids" },
                    "write_scopes": { "type": "array", "items": { "type": "string" }, "description": "write scopes claimed by this task" }
                },
                "required": ["subject"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let handles = self.handles.clone();
        Box::pin(async move {
            let caller = handles.caller_id()?;
            let subject = call.input.get("subject").and_then(Value::as_str).unwrap_or("").to_string();
            if subject.is_empty() {
                return Err("subject is required".to_string());
            }
            let description = call.input.get("description").and_then(Value::as_str).unwrap_or("").to_string();
            let blocked_by = call.input.get("blocked_by").and_then(Value::as_array)
                .map(|arr| arr.iter().filter_map(Value::as_str).map(str::to_string).collect())
                .unwrap_or_default();
            let write_scopes = call.input.get("write_scopes").and_then(Value::as_array)
                .map(|arr| arr.iter().filter_map(Value::as_str).map(str::to_string).collect())
                .unwrap_or_default();
            let req = CreateTeamTaskRequest { subject, description, blocked_by, write_scopes };
            let view = handles.service.create_task(&caller, &req).await.map_err(|e| team_err(&e))?;
            let json = task_view_json(&view);
            Ok(vec![ContentBlock::Text { text: serde_json::to_string(&json).unwrap_or_default() }])
        })
    }

    fn modes(&self) -> Option<Vec<String>> {
        Some(vec!["work".to_string()])
    }

    fn risk(&self) -> ToolRisk {
        ToolRisk::WorkspaceWrite
    }
}

/// team_task_list：分页列出任务（status/owner/ready 过滤）。
struct TeamTaskListTool {
    handles: TeamHandles,
}

impl ToolPort for TeamTaskListTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "team_task_list".to_string(),
            description: "List team tasks (filter by status/owner/ready, paginated). Input: {\"status\": string, \"owner\": string, \"ready\": boolean, \"cursor\": number, \"limit\": number}.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "status": { "type": "string", "enum": ["pending", "in_progress", "completed"], "description": "filter by status" },
                    "owner": { "type": "string", "description": "filter by owner name; use \"unowned\" for tasks without an owner" },
                    "ready": { "type": "boolean", "description": "filter by readiness" },
                    "cursor": { "type": "integer", "description": "zero-based result offset. Defaults to 0." },
                    "limit": { "type": "integer", "description": "number of rows, 1 through 100. Defaults to 50." }
                },
                "required": []
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let handles = self.handles.clone();
        Box::pin(async move {
            let caller = handles.caller_id()?;
            let tasks = handles.service.list_tasks(&caller).await.map_err(|e| team_err(&e))?;
            let status_filter = call.input.get("status").and_then(Value::as_str).map(status_from_str);
            let owner_filter = call.input.get("owner").and_then(Value::as_str);
            let ready_filter = call.input.get("ready").and_then(Value::as_bool);
            let filtered: Vec<Value> = tasks
                .iter()
                .filter(|t| status_filter.is_none_or(|s| Some(s) == t.status_opt()))
                .filter(|t| owner_filter.is_none_or(|o| {
                    if o == "unowned" {
                        t.owner_name.is_none()
                    } else {
                        t.owner_name.as_deref() == Some(o)
                    }
                }))
                .filter(|t| ready_filter.is_none_or(|r| t.ready == r))
                .map(task_view_json)
                .collect();
            let cursor = call.input.get("cursor").and_then(Value::as_u64).unwrap_or(0) as usize;
            let limit = call.input.get("limit").and_then(Value::as_u64).unwrap_or(50) as usize;
            let limit = limit.clamp(1, 100);
            let end = cursor.saturating_add(limit);
            let page: Vec<Value> = filtered.iter().skip(cursor).take(limit).cloned().collect();
            let mut out = json!({ "tasks": page });
            if end < filtered.len() {
                out["nextCursor"] = json!(end);
            }
            Ok(vec![ContentBlock::Text { text: serde_json::to_string(&out).unwrap_or_default() }])
        })
    }

    fn risk(&self) -> ToolRisk {
        ToolRisk::ReadOnly
    }
}

/// team_task_get：读取单任务当前值。
struct TeamTaskGetTool {
    handles: TeamHandles,
}

impl ToolPort for TeamTaskGetTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "team_task_get".to_string(),
            description: "Get a single team task by id. Input: {\"task_id\": string}.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "task_id": { "type": "string", "description": "task id" }
                },
                "required": ["task_id"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let handles = self.handles.clone();
        Box::pin(async move {
            let caller = handles.caller_id()?;
            let task_id = call.input.get("task_id").and_then(Value::as_str).unwrap_or("").to_string();
            if task_id.is_empty() {
                return Err("task_id is required".to_string());
            }
            let view = handles.service.get_task(&caller, &task_id).await.map_err(|e| team_err(&e))?;
            let json = task_view_json(&view);
            Ok(vec![ContentBlock::Text { text: serde_json::to_string(&json).unwrap_or_default() }])
        })
    }

    fn risk(&self) -> ToolRisk {
        ToolRisk::ReadOnly
    }
}

/// team_task_update：CAS 任务动作（claim/release/edit/…）。
struct TeamTaskUpdateTool {
    handles: TeamHandles,
}

impl ToolPort for TeamTaskUpdateTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "team_task_update".to_string(),
            description: "Update a team task with CAS. Input: {\"task_id\": string, \"expected_revision\": number, \"action\": string, ...}.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "task_id": { "type": "string", "description": "task id" },
                    "expected_revision": { "type": "number", "description": "expected current revision (CAS)" },
                    "action": { "type": "string", "enum": ["claim", "release", "edit", "set_dependencies", "complete", "reopen", "reassign", "delete"], "description": "task action" },
                    "subject": { "type": "string", "description": "new subject (edit)" },
                    "description": { "type": "string", "description": "new description (edit)" },
                    "blocked_by": { "type": "array", "items": { "type": "string" }, "description": "new blockers (set_dependencies)" },
                    "owner": { "type": "string", "description": "new owner name (reassign, Lead only)" }
                },
                "required": ["task_id", "expected_revision"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let handles = self.handles.clone();
        Box::pin(async move {
            let caller = handles.caller_id()?;
            let task_id = call.input.get("task_id").and_then(Value::as_str).unwrap_or("").to_string();
            let expected_revision = call.input.get("expected_revision").and_then(Value::as_u64).unwrap_or(0);
            let action = call.input.get("action").and_then(Value::as_str).map(action_from_str);
            if task_id.is_empty() {
                return Err("task_id is required".to_string());
            }
            let subject = call.input.get("subject").and_then(Value::as_str).map(str::to_string);
            let description = call.input.get("description").and_then(Value::as_str).map(str::to_string);
            let blocked_by = call.input.get("blocked_by").and_then(Value::as_array)
                .map(|arr| arr.iter().filter_map(Value::as_str).map(str::to_string).collect());
            let owner = call.input.get("owner").and_then(Value::as_str).map(str::to_string);
            let req = UpdateTeamTaskRequest {
                task_id,
                expected_revision,
                action,
                subject,
                description,
                blocked_by,
                write_scopes: None,
                owner,
            };
            let view = handles.service.update_task(&caller, &req).await.map_err(|e| team_err(&e))?;
            let json = task_view_json(&view);
            Ok(vec![ContentBlock::Text { text: serde_json::to_string(&json).unwrap_or_default() }])
        })
    }

    fn modes(&self) -> Option<Vec<String>> {
        Some(vec!["work".to_string()])
    }

    fn risk(&self) -> ToolRisk {
        ToolRisk::WorkspaceWrite
    }
}

/// 渲染成员视图为模型可读 JSON（target=name）。
fn member_json(m: &super::types::TeamMemberView) -> Value {
    let mut v = json!({
        "target": m.name,
        "role": role_label(m.role),
        "status": status_label(m.status),
        "diagnostics": m.diagnostics,
    });
    if let Some(d) = &m.description {
        v["description"] = json!(d);
    }
    if let Some(p) = &m.provider {
        v["provider"] = json!(p);
    }
    if let Some(c) = &m.context {
        v["context"] = json!(context_label(*c));
    }
    v
}

/// 渲染任务视图为模型可读 JSON。
fn task_view_json(view: &super::types::TeamTaskView) -> Value {
    let mut v = json!({
        "id": view.id,
        "revision": view.revision,
        "subject": view.subject,
        "description": view.description,
        "status": task_status_label(view.status),
        "blockedBy": view.blocked_by,
        "writeScopes": view.write_scopes,
        "ready": view.ready,
        "writeScopeWarnings": view.write_scope_warnings,
    });
    if let Some(o) = &view.owner_name {
        v["ownerName"] = json!(o);
    }
    v
}

fn status_label(s: TeamMemberStatus) -> &'static str {
    match s {
        TeamMemberStatus::Running => "running",
        TeamMemberStatus::Inactive => "inactive",
        TeamMemberStatus::Provisioning => "provisioning",
        TeamMemberStatus::Failed => "failed",
    }
}

fn role_label(r: super::types::TeamRole) -> &'static str {
    match r {
        super::types::TeamRole::Lead => "lead",
        super::types::TeamRole::Teammate => "teammate",
    }
}

fn context_label(c: TeamContext) -> &'static str {
    match c {
        TeamContext::Fresh => "fresh",
        TeamContext::Fork => "fork",
    }
}

fn task_status_label(s: TeamTaskStatus) -> &'static str {
    match s {
        TeamTaskStatus::Pending => "pending",
        TeamTaskStatus::InProgress => "in_progress",
        TeamTaskStatus::Completed => "completed",
        TeamTaskStatus::Deleted => "deleted",
    }
}

fn status_from_str(s: &str) -> TeamTaskStatus {
    match s {
        "in_progress" => TeamTaskStatus::InProgress,
        "completed" => TeamTaskStatus::Completed,
        _ => TeamTaskStatus::Pending,
    }
}

fn action_from_str(s: &str) -> TeamTaskAction {
    match s {
        "claim" => TeamTaskAction::Claim,
        "release" => TeamTaskAction::Release,
        "edit" => TeamTaskAction::Edit,
        "set_dependencies" => TeamTaskAction::SetDependencies,
        "complete" => TeamTaskAction::Complete,
        "reopen" => TeamTaskAction::Reopen,
        "reassign" => TeamTaskAction::Reassign,
        _ => TeamTaskAction::Delete,
    }
}

impl super::types::TeamTaskView {
    /// 供 `team_task_list` 过滤用的状态 Option（deleted 视为 None，不入列表）。
    fn status_opt(&self) -> Option<TeamTaskStatus> {
        match self.status {
            TeamTaskStatus::Deleted => None,
            s => Some(s),
        }
    }
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
