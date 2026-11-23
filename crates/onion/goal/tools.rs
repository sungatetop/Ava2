//! goal 工具面：get_goal / create_goal / update_goal（工具即 API + 权威分级）。
//!
//! - get_goal：无参，返回当前目标或 {goal: null}；
//! - create_goal(objective, max_goal_rounds?)：需 direct-human 权威；
//! - update_goal(goal_id, revision, action, ...)：action ∈ edit/pause/resume/complete/blocked，
//!   edit/pause/resume 需 direct-human；complete/blocked 在 goal-round 权威下也允许。
//!
//! 权威判定经 GoalService::authority()（in_goal_round 进程态）。

use std::sync::Arc;

use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use crate::tools::ToolPort;
use crate::types::{ContentBlock, ToolCall, ToolDefinition};
use crate::waterfall::BoxFuture;

use super::service::{Authority, GoalService};
use super::types::{GoalBlockReason, GoalRef};

/// 工具族共享句柄：服务。
#[derive(Clone)]
struct GoalHandles {
    service: Arc<GoalService>,
}

/// 工具族载体：注册 3 工具进注册表。
pub struct GoalToolSet {
    handles: GoalHandles,
}

impl GoalToolSet {
    pub fn new(service: Arc<GoalService>) -> Self {
        Self { handles: GoalHandles { service } }
    }

    pub fn register_all(&self, registry: &mut crate::tools::ToolRegistry) {
        let handles = self.handles.clone();
        registry.register(Arc::new(GetGoalTool { handles: handles.clone() }));
        registry.register(Arc::new(CreateGoalTool { handles: handles.clone() }));
        registry.register(Arc::new(UpdateGoalTool { handles }));
    }
}

/// get_goal：返回当前目标快照（含 activation 观测值）。
struct GetGoalTool {
    handles: GoalHandles,
}

impl ToolPort for GetGoalTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "get_goal".to_string(),
            description: "Get the current goal snapshot. Input: {}.".to_string(),
            parameters: json!({ "type": "object", "properties": {}, "required": [] }),
        }
    }

    fn execute(&self, _call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let service = self.handles.service.clone();
        Box::pin(async move {
            let Some(view) = service.view() else {
                return Ok(vec![ContentBlock::Text { text: json!({ "goal": null }).to_string() }]);
            };
            let goal = json!({
                "id": view.snapshot.id,
                "revision": view.snapshot.revision,
                "objective": view.snapshot.objective,
                "phase": view.snapshot.phase.label(),
                "rounds_started": view.rounds_started,
                "max_goal_rounds": view.snapshot.max_goal_rounds,
                "blocked_reason": view.snapshot.blocked_reason.as_ref().map(|r| json!({
                    "code": r.code, "message": r.message
                })),
            });
            let activation = match view.activation {
                crate::goal::types::GoalActivation::Armed => "armed",
                crate::goal::types::GoalActivation::Disarmed => "disarmed",
            };
            Ok(vec![ContentBlock::Text {
                text: json!({ "goal": goal, "activation": activation }).to_string(),
            }])
        })
    }
}

/// create_goal(objective, max_goal_rounds?)：需 direct-human。
struct CreateGoalTool {
    handles: GoalHandles,
}

impl ToolPort for CreateGoalTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "create_goal".to_string(),
            description: "Create a long-running goal. Requires direct human authority. Input: {\"objective\": string, \"max_goal_rounds\"?: number}.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "objective": { "type": "string", "description": "the goal objective" },
                    "max_goal_rounds": { "type": "integer", "description": "optional round budget" }
                },
                "required": ["objective"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let handles = self.handles.clone();
        Box::pin(async move {
            let objective = call.input.get("objective").and_then(Value::as_str).unwrap_or("").to_string();
            if objective.is_empty() {
                return Err("objective is required".to_string());
            }
            if handles.service.authority() != Authority::DirectHuman {
                return Err(format!("create_goal requires direct human authority: {:?}", handles.service.authority()));
            }
            let max_rounds = call
                .input
                .get("max_goal_rounds")
                .and_then(Value::as_u64)
                .map(|v| v as u32)
                .unwrap_or(256);
            let snapshot = handles
                .service
                .create(objective, max_rounds)
                .map_err(|e| e.to_string())?;
            Ok(vec![ContentBlock::Text {
                text: format!("goal created: {} (revision {}, rounds 0/{})", snapshot.id, snapshot.revision, snapshot.max_goal_rounds),
            }])
        })
    }
}

/// update_goal(goal_id, revision, action, ...)：权威分级迁移。
struct UpdateGoalTool {
    handles: GoalHandles,
}

impl ToolPort for UpdateGoalTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "update_goal".to_string(),
            description: "Update the current goal. action ∈ {edit,pause,resume,complete,blocked}. edit/pause/resume require direct human; complete/blocked also allowed in a goal round. Input: {\"goal_id\": string, \"revision\": number, \"action\": string, \"objective\"?: string, \"max_goal_rounds\"?: number, \"blocked_reason\"?: string}.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "goal_id": { "type": "string" },
                    "revision": { "type": "integer" },
                    "action": { "type": "string", "enum": ["edit", "pause", "resume", "complete", "blocked"] },
                    "objective": { "type": "string" },
                    "max_goal_rounds": { "type": "integer" },
                    "blocked_reason": { "type": "string" }
                },
                "required": ["goal_id", "revision", "action"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let handles = self.handles.clone();
        Box::pin(async move {
            let goal_id = call.input.get("goal_id").and_then(Value::as_str).unwrap_or("").to_string();
            let revision = call.input.get("revision").and_then(Value::as_u64).unwrap_or(0) as u32;
            let action = call.input.get("action").and_then(Value::as_str).unwrap_or("").to_string();
            if goal_id.is_empty() || action.is_empty() {
                return Err("goal_id and action are required".to_string());
            }
            let reference = GoalRef { id: goal_id, revision };
            let authority = handles.service.authority();

            let result = match action.as_str() {
                "edit" => {
                    require_human(authority, "edit")?;
                    let objective = call.input.get("objective").and_then(Value::as_str).map(str::to_string);
                    let max = call.input.get("max_goal_rounds").and_then(Value::as_u64).map(|v| v as u32);
                    if objective.is_none() && max.is_none() {
                        return Err("edit requires objective or max_goal_rounds".to_string());
                    }
                    handles.service.edit(reference, objective, max).map_err(|e| e.to_string())
                }
                "pause" => {
                    require_human(authority, "pause")?;
                    handles.service.pause(reference).map_err(|e| e.to_string())
                }
                "resume" => {
                    require_human(authority, "resume")?;
                    // 模型不能 resume durable paused 目标
                    if let Some(view) = handles.service.view() {
                        if view.snapshot.phase == crate::goal::types::GoalPhase::Paused {
                            return Err("cannot resume a durable paused goal via model".to_string());
                        }
                    }
                    handles.service.resume(reference).map_err(|e| e.to_string())
                }
                "complete" => {
                    match authority {
                        Authority::DirectHuman | Authority::GoalRound { .. } => {
                            let snapshot = handles.service.complete(reference).map_err(|e| e.to_string())?;
                            // goal-round 权威下 complete 成功 → 注入 wrap-up 引导
                            if matches!(authority, Authority::GoalRound { .. }) {
                                handles.service.inject_wrapup("complete");
                            }
                            Ok(snapshot)
                        }
                        Authority::None => return Err("update_goal requires authority".to_string()),
                    }
                }
                "blocked" => {
                    let reason = call.input.get("blocked_reason").and_then(Value::as_str).unwrap_or("").to_string();
                    if reason.is_empty() {
                        return Err("blocked requires blocked_reason".to_string());
                    }
                    match authority {
                        Authority::GoalRound { round } => {
                            // 模型自阻塞阈值：round < 阈值拒绝（direct-human 绕过）
                            handles.service.check_block_threshold(round).map_err(|e| e.to_string())?;
                            let snapshot = handles
                                .service
                                .block(reference, GoalBlockReason::new("model-reported", reason))
                                .map_err(|e| e.to_string())?;
                            // goal-round 权威下 blocked 成功 → 注入 wrap-up 引导
                            handles.service.inject_wrapup("blocked");
                            Ok(snapshot)
                        }
                        Authority::DirectHuman => handles
                            .service
                            .block(reference, GoalBlockReason::new("user", reason))
                            .map_err(|e| e.to_string()),
                        Authority::None => return Err("update_goal requires authority".to_string()),
                    }
                }
                other => return Err(format!("unknown action: {other}")),
            };

            let snapshot = result?;
            Ok(vec![ContentBlock::Text {
                text: format!(
                    "goal updated: {} (revision {}, phase {})",
                    snapshot.id,
                    snapshot.revision,
                    snapshot.phase.label()
                ),
            }])
        })
    }
}

fn require_human(authority: Authority, action: &str) -> Result<(), String> {
    if authority != Authority::DirectHuman {
        return Err(format!("update_goal({action}) requires direct human authority: {authority:?}"));
    }
    Ok(())
}
