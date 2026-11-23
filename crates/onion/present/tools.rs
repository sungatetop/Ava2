//! `present` 工具：声明已有文件为最终交付物。
//!
//! 语义：
//! - 输入 `files: [{path, description}]`，1 至 `max_files` 个（缺省 8，推荐 1-4）；
//! - 相对路径用沙箱 working directory 解析（onion 沙箱 root = 会话 cwd）；
//! - 每个文件须为**已存在的 regular file**：不存在 → `file not found`，
//!   非 regular file（目录/符号链接）→ `not a regular file`；
//! - 成功后 append 一条 `deliverables/presented` durable 事件（session-owned），
//!   返回 `{turn, files}`；只读（不复制文件内容）；
//! - 无 owning session（非 agent 调用方）拒绝。

use std::sync::{Arc, OnceLock};

use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use crate::sandbox::Sandbox;
use crate::session::SessionLog;
use crate::tools::ToolPort;
use crate::types::{ContentBlock, ToolCall, ToolDefinition};
use crate::waterfall::BoxFuture;

/// `present` 工具：沙箱 + 单次调用文件上限 + 会话接线槽。
#[derive(Clone)]
pub struct PresentTool {
    /// 文件访问沙箱（root 限定 + 防逃逸）。
    sandbox: Arc<Sandbox>,
    /// 单次调用的文件数上限（缺省 8）。
    pub max_files: usize,
    /// 会话接线槽（Runtime 装配后 wire）。
    session: Arc<OnceLock<Arc<SessionLog>>>,
}

impl PresentTool {
    pub fn new(sandbox: Arc<Sandbox>, max_files: usize) -> Self {
        Self {
            sandbox,
            max_files,
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
            .ok_or_else(|| "present requires an owning agent session".to_string())
    }
}

/// 校验单个文件：非空路径 + 沙箱内存在 + regular file。
async fn validate_file(sandbox: &Sandbox, path: &str) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("present requires a non-empty file path".to_string());
    }
    let metadata = sandbox
        .stat(path)
        .await
        .map_err(|_| format!("Cannot present {path}: file not found. Check the path, create the file if needed, and retry."))?;
    if !metadata.is_file() {
        return Err(format!("Cannot present {path}: not a regular file"));
    }
    Ok(())
}

impl ToolPort for PresentTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "present".to_string(),
            description: "Declare existing files as final deliverables for the user. Use it when the \
                user needs a separate file, especially Office documents, spreadsheets, and slide decks; \
                prefer your final response when that suffices. The user opens the current files; their \
                contents are not copied."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "files": {
                        "type": "array",
                        "description": "Usually the 1-2 most important deliverables; at most 4 per call.",
                        "items": {
                            "type": "object",
                            "additionalProperties": false,
                            "properties": {
                                "path": { "type": "string", "description": "Path of an existing regular file. Relative paths use the Session working directory." },
                                "description": { "type": "string", "description": "Brief description for the user." }
                            },
                            "required": ["path"]
                        }
                    }
                },
                "required": ["files"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let sandbox = self.sandbox.clone();
        let max_files = self.max_files;
        let session = self.session();
        Box::pin(async move {
            let session = session?;
            let files = call
                .input
                .get("files")
                .and_then(Value::as_array)
                .ok_or_else(|| "present: missing 'files' array".to_string())?;
            if files.is_empty() || files.len() > max_files {
                return Err(format!("present accepts 1 to {max_files} files"));
            }
            let mut presented = Vec::with_capacity(files.len());
            for raw in files {
                let path = raw
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "present: each file requires a 'path' string".to_string())?;
                validate_file(&sandbox, path).await?;
                let description = raw
                    .get("description")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                presented.push(json!({
                    "path": path,
                    "description": description,
                }));
            }
            // 当前回合号（durable 事件归属；onion 无 turn 边界投影，取已记录回合数）。
            let turn = session
                .snapshot()
                .iter()
                .filter(|event| matches!(event, crate::session::SessionEvent::TurnStart { .. }))
                .count() as u64;
            session.append(crate::session::SessionEvent::Presented {
                turn,
                call_id: call.id.clone(),
                files: presented.clone(),
            });
            Ok(vec![ContentBlock::Text {
                text: json!({ "turn": turn, "files": presented }).to_string(),
            }])
        })
    }
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
