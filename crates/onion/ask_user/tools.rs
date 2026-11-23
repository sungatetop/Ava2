//! ask_user 工具面：ask_user_question（legacy blocking / timed 问询）。
//!
//! legacy 模式默认阻塞（省略 `timeout`
//! 即无 deadline，等待宿主呈示问题并回填人工答复）；提供 `timeout`（秒，`-1`=
//! 无限）时走 timed 模式——前台等待逾限返回 pending 结果（不中断 agent loop），
//! 问题保持可答，用户迟到答复经 `AnswerToPending` source 注入 agent inbox。
//! 答复作为普通工具结果回填 agent loop（答复 JSON 序列化为文本块）。

use std::sync::Arc;

use serde_json::json;
use tokio_util::sync::CancellationToken;

use crate::tools::ToolPort;
use crate::types::{ContentBlock, ToolCall, ToolDefinition};
use crate::waterfall::BoxFuture;

use super::service::{AskUserService, TimedOutcome};
use super::types::{AskUserOption, AskUserQuestion};

/// ask_user_question：向用户提问，等待人工答复。
pub struct AskUserQuestionTool {
    service: Arc<AskUserService>,
}

impl AskUserQuestionTool {
    pub fn new(service: Arc<AskUserService>) -> Self {
        Self { service }
    }
}

impl ToolPort for AskUserQuestionTool {
    /// 交互阻塞型工具：执行挂起等待宿主呈示问题并回填答复，不参与注册表统一
    /// tool_timeout（timed 前台等待可能远超 60s，且 legacy 模式无限阻塞），
    /// 避免等待答复时被误杀。timed 的逾限由工具自身（`tokio::time::timeout`）
    /// 控制，超时返回 pending 而非强杀。
    fn blocking(&self) -> bool {
        true
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "ask_user_question".to_string(),
            description: "Ask the user a concise question when you need confirmation, a choice, or missing information before proceeding. Input: {\"questions\": [{\"id\": string, \"question\": string, \"header\"?: string, \"options\"?: [{\"label\": string, \"description\"?: string}], \"multi_select\"?: boolean}], \"timeout\"?: integer}. Omit timeout to block until answered; set timeout to a positive number of seconds to continue with a pending result after the wait (the question remains answerable); use -1 to block indefinitely.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "questions": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string", "description": "Stable id for this question; echoed in the answer." },
                                "question": { "type": "string", "description": "The specific question to ask the user." },
                                "header": { "type": "string", "description": "Optional short heading for the question, such as \"Confirm\" or \"Choose Mode\"." },
                                "options": {
                                    "type": "array",
                                    "description": "Optional choices to show the user. If you recommend one, put it first and append \"(Recommended)\" to that label.",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "label": { "type": "string", "description": "Short user-facing option label." },
                                            "description": { "type": "string", "description": "One sentence explaining the tradeoff or impact." }
                                        },
                                        "required": ["label"]
                                    }
                                },
                                "multi_select": { "type": "boolean", "description": "Whether the user may select more than one option. Defaults to false." }
                            },
                            "required": ["id", "question"]
                        }
                    },
                    "timeout": { "type": "integer", "description": "Foreground wait in seconds before returning a pending result (default: block until answered). Use -1 only when an answer is required before proceeding." }
                },
                "required": ["questions"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let service = self.service.clone();
        Box::pin(async move {
            let questions = parse_questions(&call)?;
            let timeout = call
                .input
                .get("timeout")
                .and_then(serde_json::Value::as_i64);
            match service.ask(questions, timeout, call.id.clone()).await? {
                TimedOutcome::Answered(answer) => {
                    // 答复作为 JSON 文本块回填 agent loop。
                    Ok(vec![ContentBlock::Text {
                        text: json!({ "answers": answer.answers }).to_string(),
                    }])
                }
                TimedOutcome::Pending => {
                    // timed pending：返回 pending 结果，不中断 agent loop；
                    // 问题保持可答，迟到答复经 AnswerToPending 注入 agent inbox。
                    let message = "No answer batch arrived before the timeout. This is pending, not a skipped answer. \
Continue useful independent work. The user can still answer; their reply will be injected as a user message \
identified as answer_to_pending_question with this callId and the original questions. Do not treat this as permission.";
                    Ok(vec![ContentBlock::Text {
                        text: json!({
                            "pending": true,
                            "callId": call.id,
                            "message": message,
                        })
                        .to_string(),
                    }])
                }
            }
        })
    }
}

/// 解析并校验 `questions` 参数（参数 schema 校验）。
fn parse_questions(call: &ToolCall) -> Result<Vec<AskUserQuestion>, String> {
    let Some(questions) = call.input.get("questions").and_then(|v| v.as_array()) else {
        return Err("questions is required".to_string());
    };
    if questions.is_empty() {
        return Err("questions must not be empty".to_string());
    }
    let mut parsed = Vec::with_capacity(questions.len());
    for value in questions {
        let id = value.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let question = value.get("question").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if id.is_empty() {
            return Err("each question needs a non-empty id".to_string());
        }
        if question.is_empty() {
            return Err("each question needs non-empty question text".to_string());
        }
        let options = value.get("options").and_then(|v| v.as_array()).map(|arr| {
            arr.iter()
                .map(|opt| AskUserOption {
                    label: opt.get("label").and_then(|l| l.as_str()).unwrap_or("").to_string(),
                    description: opt.get("description").and_then(|d| d.as_str()).map(str::to_string),
                })
                .collect::<Vec<_>>()
        });
        parsed.push(AskUserQuestion {
            id,
            question,
            header: value.get("header").and_then(|h| h.as_str()).map(str::to_string),
            options,
            multi_select: value.get("multi_select").and_then(serde_json::Value::as_bool),
        });
    }
    Ok(parsed)
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
