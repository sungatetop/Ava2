//! todo_write 工具：模型侧整表替换。
//!
//! 核心语义：
//! - 输入 `todos: [{content, status}]`，**整体替换**当前清单；
//! - 校验：content 去空白后非空、content 唯一、状态枚举（schema 层）；
//! - `allowParallelInProgress=false` 时，`in_progress` 项至多一个，否则拒绝；
//! - 每次调用 append 一条 `todo/write` durable 事件（session-owned）；
//! - 无 owning session（非 agent 调用方）拒绝；
//! - 返回整表 + counts（pending / inProgress / completed）。
//!
//! 工具本身 session-owned：State 经 OnceLock 延迟接线（Runtime 装配后）。

use std::sync::{Arc, OnceLock};

use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::types::{TodoCounts, TodoItem, TodoSnapshot, TodoStatus};
use crate::session::SessionLog;
use crate::tools::ToolPort;
use crate::types::{ContentBlock, ToolCall, ToolDefinition};
use crate::waterfall::BoxFuture;

/// todo 工具共享句柄：部署策略 + 会话接线槽。
#[derive(Clone)]
pub struct TodoWriteTool {
    /// 是否允许多个 `in_progress`（部署决策，`allowParallelInProgress`）。
    pub allow_parallel_in_progress: bool,
    /// 会话接线槽（Runtime 装配后 wire）。
    session: Arc<OnceLock<Arc<SessionLog>>>,
}

impl TodoWriteTool {
    pub fn new(allow_parallel_in_progress: bool) -> Self {
        Self {
            allow_parallel_in_progress,
            session: Arc::new(OnceLock::new()),
        }
    }

    /// 延迟接线会话（宿主装配完成后调用）。
    pub fn wire_session(&self, session: Arc<SessionLog>) {
        let _ = self.session.set(session);
    }

    fn session(&self) -> Result<Arc<SessionLog>, String> {
        self.session
            .get()
            .cloned()
            .ok_or_else(|| "todo_write requires an owning agent session".to_string())
    }
}

/// 组装模型可见 description（active 状态子句随并行策略变化）。
fn describe(allow_parallel: bool) -> String {
    let head = "Record and update a task list to plan multi-step work and show progress; \
        skip it for trivial single-step tasks. Add one todo per concrete step before you start. ";
    let active = if allow_parallel {
        "While work remains, keep the todos being worked on `in_progress`, several only when work runs in parallel. "
    } else {
        "While work remains, keep exactly one todo `in_progress`. "
    };
    let tail = "Mark each todo `completed` as soon as it is done.";
    format!("{head}{active}{tail}")
}

impl ToolPort for TodoWriteTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "todo_write".to_string(),
            description: describe(self.allow_parallel_in_progress),
            parameters: json!({
                "type": "object",
                "properties": {
                    "todos": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "additionalProperties": false,
                            "properties": {
                                "content": { "type": "string", "description": "What the task is — a short imperative line." },
                                "status": {
                                    "type": "string",
                                    "enum": ["pending", "in_progress", "completed"],
                                    "description": "pending (not started) | in_progress (now) | completed (done)."
                                }
                            },
                            "required": ["content", "status"]
                        }
                    }
                },
                "required": ["todos"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let allow_parallel = self.allow_parallel_in_progress;
        let session = self.session();
        Box::pin(async move {
            let session = session?;
            let todos = call
                .input
                .get("todos")
                .and_then(Value::as_array)
                .ok_or_else(|| "todo_write: missing 'todos' array".to_string())?;
            let items = to_todo_list(todos, allow_parallel)?;
            let snapshot = TodoSnapshot { todos: items.clone() };
            // session-owned durable 事件：整表 last-write-wins（todo/write）。
            crate::todo::store::append_todo_write(&session, items.clone());
            let counts = snapshot.counts();
            Ok(vec![ContentBlock::Text {
                text: json!({
                    "todos": items.iter().map(|t| json!({ "content": t.content, "status": t.status.label() })).collect::<Vec<_>>(),
                    "counts": counts_json(&counts),
                })
                .to_string(),
            }])
        })
    }
}

/// 校验并构造规范化清单（`toTodoList` 语义）：content 去空白非空、唯一；
/// `allowParallel=false` 时至多一个 `in_progress`。
fn to_todo_list(raw: &[Value], allow_parallel: bool) -> Result<Vec<TodoItem>, String> {
    let mut items = Vec::with_capacity(raw.len());
    let mut seen = std::collections::HashSet::new();
    let mut active = 0usize;
    for item in raw {
        let content = item.get("content").and_then(Value::as_str).unwrap_or("").trim().to_string();
        if content.is_empty() {
            return Err("invalid todo: `content` must be a non-empty string".to_string());
        }
        if !seen.insert(content.clone()) {
            return Err(format!("invalid todos: duplicate content {content:?}"));
        }
        let status = match item.get("status").and_then(Value::as_str).unwrap_or("") {
            "in_progress" => {
                active += 1;
                TodoStatus::InProgress
            }
            "completed" => TodoStatus::Completed,
            _ => TodoStatus::Pending,
        };
        items.push(TodoItem { content, status });
    }
    if !allow_parallel && active > 1 {
        return Err(format!("invalid todos: at most one task may be in_progress (got {active})"));
    }
    Ok(items)
}

/// counts 序列化（pending / inProgress / completed）。
fn counts_json(counts: &TodoCounts) -> Value {
    json!({
        "pending": counts.pending,
        "inProgress": counts.in_progress,
        "completed": counts.completed,
    })
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
