//! plan 工具面：exit_plan_mode（plan 审批闭环）。
//!
//! 工具任何状态下保持注册（只改 prompt 分节，
//! 不改工具目录）。plan 必须以 `#` 标题开头（`^#\s+\S`）→ 发起 plan-review 审批。

use std::sync::Arc;

use serde_json::json;
use tokio_util::sync::CancellationToken;

use crate::tools::ToolPort;
use crate::types::{ContentBlock, ToolCall, ToolDefinition};
use crate::waterfall::BoxFuture;

use super::service::PlanMode;

/// plan 标题校验：`#` 后须有空白且随后紧跟非空白字符（`^#\s+\S`）。
fn valid_heading(plan: &str) -> bool {
    let rest = plan.strip_prefix('#').unwrap_or_default();
    let mut chars = rest.chars();
    let mut saw_ws = false;
    for c in chars.by_ref() {
        if c.is_whitespace() {
            saw_ws = true;
        } else {
            // 首个非空白字符之前至少消费过一个空白（`\s+` 后紧跟 `\S`）。
            return saw_ws;
        }
    }
    false
}

/// exit_plan_mode：模型产出计划并请求退出 plan 模式（审批闭环）。
pub struct ExitPlanModeTool {
    plan_mode: Arc<PlanMode>,
}

impl ExitPlanModeTool {
    pub fn new(plan_mode: Arc<PlanMode>) -> Self {
        Self { plan_mode }
    }
}

impl ToolPort for ExitPlanModeTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "exit_plan_mode".to_string(),
            description: "Submit a complete plan (markdown starting with a # heading) to exit plan mode for user approval. Input: {\"plan\": string}.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "plan": { "type": "string", "description": "the complete plan as markdown, starting with a # heading" }
                },
                "required": ["plan"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let plan_mode = self.plan_mode.clone();
        Box::pin(async move {
            if !plan_mode.active() {
                return Err("not in plan mode".to_string());
            }
            let plan = call.input.get("plan").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if plan.is_empty() {
                return Err("plan is required".to_string());
            }
            if !valid_heading(&plan) {
                return Err("plan must start with a # heading".to_string());
            }
            match plan_mode.review(plan).await {
                Ok(true) => Ok(vec![ContentBlock::Text {
                    text: json!({ "approved": true }).to_string(),
                }]),
                Ok(false) => Ok(vec![ContentBlock::Text {
                    text: json!({ "approved": false }).to_string(),
                }]),
                Err(feedback) => Err(feedback),
            }
        })
    }

    /// plan-review 等待宿主复决：阻断型，不参与统一 tool_timeout
    ///（交互工具省略 timeoutMs = 默认阻塞；取消走 ASK_CANCELLED 语义）。
    fn blocking(&self) -> bool {
        true
    }
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
