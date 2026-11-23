//! 文件编辑工具：`edit`（唯一 literal replace）+ `str_replace_editor`（四动作编辑器）。
//!
//! 两个工具并存，不互相裁剪：
//! - `edit`：模型直接改一处字面量。`old_string` 默认必须唯一命中，否则拒绝并要求
//!   提供更多上下文或显式 `replace_all`（避免误改多处）。
//! - `str_replace_editor`：`view` / `create` / `str_replace` / `insert` 四动作，
//!   适合「先看片段再精确插入」的编辑流。
//!
//! read-before-write：改写既有文件前须先经
//! `read` 等读路径取得观测；缺失也算有效观察（可创建）。
//! 自读取以来被改动则 fail-closed（`FS_STALE_VERSION`），要求重读后重试。

use std::sync::Arc;

use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::ToolPort;
use crate::approval::ToolRisk;
use crate::fs_observation::{key_of, version_of, FsObservation, Observed};
use crate::sandbox::Sandbox;
use crate::types::{ContentBlock, ToolCall, ToolDefinition};
use crate::waterfall::BoxFuture;

/// 写面工具共享的观测门禁辅助：按当前文件内容算出过期凭证。
///
/// 返回 `Err` 表示未观察或与观察冲突（模型可见拒绝信息）；`Ok(Some(token))`
/// 表示过期凭证（自读取以来已被改动），`Ok(None)` 表示可放行。
async fn guard_edit(
    observation: &FsObservation,
    sandbox: &Sandbox,
    relative: &str,
) -> Result<Option<String>, String> {
    let key = key_of(std::path::Path::new(relative));
    let current = match sandbox.read_bytes(relative).await {
        Ok(data) => Some(version_of(&data)),
        Err(_) => None,
    };
    observation.guard(&key, current.as_deref())
}

/// 改动成功后刷新观测（本会话已持有新事实）。
async fn refresh_observation(observation: &FsObservation, sandbox: &Sandbox, relative: &str) {
    let key = key_of(std::path::Path::new(relative));
    match sandbox.read_bytes(relative).await {
        Ok(data) => observation.refresh(&key, version_of(&data)),
        Err(_) => observation.record(&key, Observed::Absent),
    }
}

/// edit：唯一 literal replace（WorkspaceWrite；read-before-write 门在工具内部）。
pub struct EditTool {
    pub sandbox: Arc<Sandbox>,
    pub observation: Arc<FsObservation>,
}

impl ToolPort for EditTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "edit".to_string(),
            description: "Edit an existing UTF-8 text file by replacing literal text. Read the file \
                before editing it. Input: {\"file_path\": string, \"old_string\": string, \
                \"new_string\": string, \"replace_all\": boolean?}."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "Path to edit, relative to sandbox root." },
                    "old_string": { "type": "string", "description": "Literal text to replace. Must be non-empty and unique unless replace_all." },
                    "new_string": { "type": "string", "description": "Literal replacement text. Empty string deletes the match." },
                    "replace_all": { "type": "boolean", "description": "Replace all matches. Defaults to false." }
                },
                "required": ["file_path", "old_string", "new_string"]
            }),
        }
    }

    fn modes(&self) -> Option<Vec<String>> {
        Some(vec!["work".to_string()])
    }

    fn risk(&self) -> ToolRisk {
        ToolRisk::WorkspaceWrite
    }

    fn defer_loading(&self) -> bool {
        // 2026-10-20：deferred 保留代码但不使用——工具默认完整披露。
        false
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let sandbox = self.sandbox.clone();
        let observation = self.observation.clone();
        Box::pin(async move {
            let path = arg(&call.input, "file_path");
            let old = arg(&call.input, "old_string");
            let new = arg(&call.input, "new_string");
            let replace_all = call.input.get("replace_all").and_then(Value::as_bool).unwrap_or(false);
            if path.trim().is_empty() {
                return Err("file_path must be a non-empty string".to_string());
            }
            let token = guard_edit(&observation, &sandbox, &path).await?;
            let message = sandbox
                .edit_text(&path, &old, &new, replace_all, token.as_deref())
                .await?;
            refresh_observation(&observation, &sandbox, &path).await;
            Ok(vec![ContentBlock::Text { text: message }])
        })
    }
}

/// str_replace_editor：四动作编辑器（view / create / str_replace / insert）。
pub struct StrReplaceEditorTool {
    pub sandbox: Arc<Sandbox>,
    pub observation: Arc<FsObservation>,
}

impl ToolPort for StrReplaceEditorTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "str_replace_editor".to_string(),
            description: "View, create, or edit a text file. `command` is one of `view`, `create`, \
                `str_replace`, `insert`. Read existing files before modifying them."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "enum": ["view", "create", "str_replace", "insert"],
                        "description": "The command to run."
                    },
                    "path": { "type": "string", "description": "Path relative to sandbox root." },
                    "file_text": { "type": "string", "description": "Required for `create`: the content of the file to create." },
                    "insert_line": { "type": "integer", "description": "Required for `insert`: insert AFTER this 1-based line number." },
                    "new_str": { "type": "string", "description": "Replacement (str_replace) or inserted (insert) text." },
                    "old_str": { "type": "string", "description": "Literal text to replace for `str_replace`." },
                    "view_range": {
                        "type": "array",
                        "items": { "type": "integer" },
                        "description": "Optional [start, end] 1-based line range for `view`; end -1 means to end of file."
                    }
                },
                "required": ["command", "path"]
            }),
        }
    }

    fn modes(&self) -> Option<Vec<String>> {
        Some(vec!["work".to_string()])
    }

    fn risk(&self) -> ToolRisk {
        ToolRisk::WorkspaceWrite
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let sandbox = self.sandbox.clone();
        let observation = self.observation.clone();
        Box::pin(async move {
            let command = arg(&call.input, "command");
            let path = arg(&call.input, "path");
            match command.as_str() {
                "view" => {
                    // view 是读动作：顺带登记观测（缺失也记 Absent）。
                    let key = key_of(std::path::Path::new(&path));
                    match sandbox.read_text(&path).await {
                        Ok(text) => {
                            observation.refresh(&key, version_of(text.as_bytes()));
                            Ok(vec![ContentBlock::Text { text: view_range(&text, &call.input) }])
                        }
                        Err(error) => {
                            observation.record(&key, Observed::Absent);
                            Err(error)
                        }
                    }
                }
                "create" => {
                    let text = arg(&call.input, "file_text");
                    // 创建新文件不受观测门禁约束（缺失的观察允许后续 write 重建）。
                    let message = sandbox.write(&path, &text).await?;
                    refresh_observation(&observation, &sandbox, &path).await;
                    Ok(vec![ContentBlock::Text { text: message }])
                }
                "str_replace" => {
                    let old = arg(&call.input, "old_str");
                    let new = arg(&call.input, "new_str");
                    let token = guard_edit(&observation, &sandbox, &path).await?;
                    let message = sandbox.edit_text(&path, &old, &new, false, token.as_deref()).await?;
                    refresh_observation(&observation, &sandbox, &path).await;
                    Ok(vec![ContentBlock::Text { text: message }])
                }
                "insert" => {
                    let line = call.input.get("insert_line").and_then(Value::as_i64).unwrap_or(-1);
                    let text = arg(&call.input, "new_str");
                    let token = guard_edit(&observation, &sandbox, &path).await?;
                    if let Some(_token) = token {
                        return Err(format!(
                            "cannot modify \"{path}\": file changed since it was read \
                             (FS_STALE_VERSION) — read the file again, then retry"
                        ));
                    }
                    let message = sandbox.insert_line(&path, line, &text).await?;
                    refresh_observation(&observation, &sandbox, &path).await;
                    Ok(vec![ContentBlock::Text { text: message }])
                }
                other => Err(format!(
                    "unknown command \"{other}\" — allowed: view, create, str_replace, insert"
                )),
            }
        })
    }
}

/// 读参（缺省空串，由下游校验给出模型可见错误）。
fn arg(input: &Value, key: &str) -> String {
    input.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

/// 按 `view_range` 截取行区间（1 基闭区间；end = -1 表示到文件末尾）。
fn view_range(text: &str, input: &Value) -> String {
    let Some(range) = input.get("view_range").and_then(Value::as_array) else {
        return text.to_string();
    };
    if range.len() != 2 {
        return text.to_string();
    }
    let start = range[0].as_i64().unwrap_or(1).max(1) as usize;
    let end_raw = range[1].as_i64().unwrap_or(-1);
    let lines: Vec<&str> = text.lines().collect();
    let end = if end_raw < 0 { lines.len() } else { (end_raw as usize).min(lines.len()) };
    if start > end {
        return String::new();
    }
    lines[start - 1..end].join("\n")
}
