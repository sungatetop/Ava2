//! Ralph 工具。
//!
//! 固定循环：对单一不可变目标，每轮开启一个全新 fresh child（无父会话上下文，
//! 仅携带目标 + 前轮有界报告），子代理返回文本报告，固定脚本解析出
//! `{ status, summary, next_steps, blocker }`（文本约定）。当 status=complete/blocked 或达到 maxRounds 时停止。

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;
use tokio_util::sync::CancellationToken;

use crate::subagent::runtime::SubagentRuntime;
use crate::tools::ToolPort;
use crate::types::{ContentBlock, ToolCall, ToolDefinition};
use crate::waterfall::BoxFuture;

/// 部署轮数上限（缺省 256）。
const DEFAULT_MAX_ROUNDS: u32 = 256;
/// 跨轮报告最大字符数（缺省 16384）。
const MAX_HANDOFF_CHARS: usize = 16_384;
/// 每轮 child 的目标 prompt 模板。
const ROUND_PROMPT_TEMPLATE: &str = r#"You are one round of a Ralph loop. Your immutable objective is:

{objective}

{prior_context}Produce a report in exactly this text format (no other output):

STATUS: <continue|complete|blocked>
SUMMARY: <one-line summary of progress toward the objective>
NEXT_STEPS: <comma-separated next steps, or "none" if complete>
BLOCKER: <the blocking obstacle, or "none">

Rules:
- If the objective is fully achieved, STATUS: complete and NEXT_STEPS: none.
- If you are permanently stuck, STATUS: blocked and describe the blocker.
- Otherwise STATUS: continue with concrete next steps.
"#;

/// Ralph 工具：固定循环 fresh-agent 迭代。
pub struct RalphTool {
    runtime: Arc<SubagentRuntime>,
    max_rounds: u32,
}

impl RalphTool {
    pub fn new(runtime: Arc<SubagentRuntime>, max_rounds: u32) -> Self {
        Self { runtime, max_rounds: max_rounds.max(1) }
    }

    /// 生成一轮 report 的 prompt（目标 + 前轮报告）。
    fn round_prompt(objective: &str, prior_report: Option<&str>) -> String {
        let prior_context = match prior_report {
            Some(report) => format!(
                "The previous round's report was:\n{report}\n\nContinue from there.\n\n"
            ),
            None => "This is the first round.\n\n".to_string(),
        };
        ROUND_PROMPT_TEMPLATE
            .replace("{objective}", objective)
            .replace("{prior_context}", &prior_context)
    }

    /// 从子代理文本中解析出 `{ status, summary, next_steps, blocker }`。
    /// 解析失败返回 `status=continue, summary=<raw>`（尽力而为，不中断循环）。
    fn parse_report(text: &str) -> RalphReport {
        let mut status = "continue".to_string();
        let mut summary = text.chars().take(MAX_HANDOFF_CHARS).collect::<String>();
        let mut next_steps = "none".to_string();
        let mut blocker = "none".to_string();
        for line in text.lines() {
            if let Some(v) = line.strip_prefix("STATUS:") {
                status = v.trim().to_lowercase();
            } else if let Some(v) = line.strip_prefix("SUMMARY:") {
                summary = v.trim().chars().take(MAX_HANDOFF_CHARS).collect::<String>();
            } else if let Some(v) = line.strip_prefix("NEXT_STEPS:") {
                next_steps = v.trim().to_string();
            } else if let Some(v) = line.strip_prefix("BLOCKER:") {
                blocker = v.trim().to_string();
            }
        }
        // 合法性校验：仅认可三个状态，否则视为 continue。
        if !matches!(status.as_str(), "continue" | "complete" | "blocked") {
            status = "continue".to_string();
        }
        RalphReport { status, summary, next_steps, blocker }
    }
}

/// 一轮解析出的报告（文本约定）。
struct RalphReport {
    status: String,
    summary: String,
    next_steps: String,
    blocker: String,
}

impl RalphReport {
    /// 序列化为跨轮传递文本（有界）。
    fn render(&self) -> String {
        format!(
            "STATUS: {}\nSUMMARY: {}\nNEXT_STEPS: {}\nBLOCKER: {}",
            self.status, self.summary, self.next_steps, self.blocker
        )
    }
}

impl ToolPort for RalphTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "ralph".to_string(),
            description: "Run a foreground fresh-agent Ralph loop toward one immutable objective. \
                Use only when the direct human explicitly asks for Ralph or fresh-agent iteration. \
                Each round opens a new child with no parent conversation or prior child session; \
                only a bounded structured report crosses rounds. Input: {\"objective\": string, \
                \"maxRounds\": number}."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "objective": {
                        "type": "string",
                        "description": "The immutable completion objective for every fresh Ralph round."
                    },
                    "maxRounds": {
                        "type": "number",
                        "description": "Optional positive safe-integer round cap, bounded by the deployment ceiling."
                    }
                },
                "required": ["objective"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let runtime = self.runtime.clone();
        let objective = call.input.get("objective").and_then(serde_json::Value::as_str).unwrap_or("").to_string();
        let cap = self.max_rounds;
        let max_rounds = call
            .input
            .get("maxRounds")
            .and_then(serde_json::Value::as_u64)
            .map(|r| (r as u32).clamp(1, cap))
            .unwrap_or(cap);
        Box::pin(async move {
            if objective.trim().is_empty() {
                return Err("objective must be a non-empty string".to_string());
            }
            let run_id = format!("ralph-{}", SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0));
            let mut agents_started: u64 = 0;
            let mut prior_report: Option<String> = None;
            let mut rounds_started: u32 = 0;
            let mut final_status = "budget-limited".to_string();
            let mut final_report: Option<String> = None;

            while rounds_started < max_rounds {
                let prompt = Self::round_prompt(&objective, prior_report.as_deref());
                // 每轮 fresh child（depth=1，固定一层）。
                match runtime.run_round(prompt, 1).await {
                    Ok(text) => {
                        agents_started += 1;
                        rounds_started += 1;
                        let report = Self::parse_report(&text);
                        final_report = Some(report.render());
                        match report.status.as_str() {
                            "complete" => {
                                final_status = "complete".to_string();
                                break;
                            }
                            "blocked" => {
                                final_status = "blocked".to_string();
                                break;
                            }
                            _ => {
                                // continue：传递报告进入下一轮。
                                prior_report = final_report.clone();
                            }
                        }
                    }
                    Err(e) => {
                        final_status = "round-failed".to_string();
                        final_report = Some(format!("round failed: {e}"));
                        break;
                    }
                }
            }

            let result = match final_status.as_str() {
                "complete" | "blocked" | "budget-limited" => json!({
                    "status": final_status,
                    "roundsStarted": rounds_started,
                    "report": final_report,
                }),
                _ => json!({
                    "status": "round-failed",
                    "roundsStarted": rounds_started,
                    "lastReport": final_report,
                }),
            };
            Ok(vec![ContentBlock::Text {
                text: format!(
                    "ralph completed ({} agent(s)).\nReturn value:\n{}",
                    agents_started,
                    serde_json::to_string_pretty(&json!({
                        "runId": run_id,
                        "agentsStarted": agents_started,
                        "result": result,
                    }))
                    .unwrap_or_else(|_| result.to_string())
                ),
            }])
        })
    }
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
