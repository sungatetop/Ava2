//! `apply_patch` 工具：一次提交多文件补丁。
//!
//! 语义（三阶段）：
//! 1. **语法**：解析失败即整体拒绝，不触碰任何文件；
//! 2. **预检**：逐操作解析路径 + 观测门禁 + 读取原文并试算 hunk，任一失败即整体
//!    拒绝（`preflight` 阶段），现网不动；
//! 3. **提交**：按补丁顺序执行，遇首个失败即停止且**不回滚**（已完成的
//!    操作保留，模型须重读受影响文件后重试）。
//!
//! 路径一律相对沙箱 root（`Sandbox::resolve` 遏制逃逸）；写面风险 `WorkspaceWrite`。

use std::sync::Arc;

use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::patch::{apply_hunks, parse_patch, ApplyHunksResult, PatchOperation};
use super::ToolPort;
use crate::approval::ToolRisk;
use crate::fs_observation::{key_of, version_of, FsObservation, Observed};
use crate::sandbox::Sandbox;
use crate::types::{ContentBlock, ToolCall, ToolDefinition};
use crate::waterfall::BoxFuture;

/// 多文件补丁工具。
pub struct ApplyPatchTool {
    pub sandbox: Arc<Sandbox>,
    pub observation: Arc<FsObservation>,
}

impl ToolPort for ApplyPatchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "apply_patch".to_string(),
            description: "Create, update, delete, or move UTF-8 text files with one patch; prefer it \
                over multiple edit calls when a change spans several files or separated regions. \
                The whole patch is checked first (syntax, hunk matching, missing files) and a \
                rejected patch changes nothing; once applying starts, operations run in order, stop \
                at the first failure, and are not rolled back."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "Patch text: start with *** Begin Patch and end with *** End Patch. \
                            Use *** Add File: path with + lines; *** Update File: path, optionally followed by \
                            *** Move to: path, with @@ or @@ context and space-prefixed context, + added, or \
                            - removed lines; *** End of File anchors the last hunk to the file end. \
                            Use *** Delete File: path to delete. Paths are relative to the sandbox root."
                    }
                },
                "required": ["input"]
            }),
        }
    }

    fn modes(&self) -> Option<Vec<String>> {
        Some(vec!["work".to_string()])
    }

    fn risk(&self) -> ToolRisk {
        ToolRisk::WorkspaceWrite
    }

    fn execute(
        &self,
        call: ToolCall,
        _cancel: CancellationToken,
    ) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let sandbox = self.sandbox.clone();
        let observation = self.observation.clone();
        Box::pin(async move {
            let input = call.input.get("input").and_then(serde_json::Value::as_str).unwrap_or("");
            if input.trim().is_empty() {
                return Err("input must be a non-empty patch string".to_string());
            }
            // 阶段一：语法（失败即整体拒绝，不触碰文件）。
            let operations = parse_patch(input).map_err(|error| error.to_string())?;
            if operations.is_empty() {
                return Ok(vec![ContentBlock::Text { text: "empty patch: nothing to do".to_string() }]);
            }
            // 阶段二：预检（路径解析 + 观测门禁 + hunk 试算），任一失败整体拒绝。
            let mut planned: Vec<Planned> = Vec::with_capacity(operations.len());
            for operation in &operations {
                planned.push(preflight(&sandbox, &observation, operation).await?);
            }
            // 阶段三：顺序提交（首个失败即停，不回滚）。
            let mut applied: Vec<String> = Vec::new();
            for item in planned {
                match commit(&sandbox, &observation, item).await {
                    Ok(summary) => applied.push(summary),
                    Err(error) => {
                        let done = if applied.is_empty() {
                            "no operation was applied".to_string()
                        } else {
                            format!("applied before the failure (not rolled back): {}", applied.join("; "))
                        };
                        return Err(format!("{error}\n{done}"));
                    }
                }
            }
            Ok(vec![ContentBlock::Text { text: applied.join("\n") }])
        })
    }
}

/// 预检通过的一个操作（已解析路径、已试算新文本）。
enum Planned {
    Add { path: String, content: String },
    Delete { path: String },
    Update { path: String, after: String },
    Move { path: String, destination: String, after: String },
}

/// 预检单操作：解析路径、过观测门禁、读取原文并试算 hunk。
async fn preflight(
    sandbox: &Sandbox,
    observation: &FsObservation,
    operation: &PatchOperation,
) -> Result<Planned, String> {
    match operation {
        PatchOperation::Add { path, content, .. } => {
            // 新建不受 read-before-write 约束（add 即创建）；但目标已存在
            // 时拒绝，避免静默覆盖既有文件。
            if sandbox.read_bytes(path).await.is_ok() {
                return Err(format!(
                    "cannot add \"{path}\": file already exists — use *** Update File instead"
                ));
            }
            Ok(Planned::Add { path: path.clone(), content: content.clone() })
        }
        PatchOperation::Delete { path, .. } => {
            let key = key_of(std::path::Path::new(path));
            let data = sandbox
                .read_bytes(path)
                .await
                .map_err(|error| format!("cannot delete \"{path}\": {error}"))?;
            observation.guard(&key, Some(&version_of(&data)))?;
            Ok(Planned::Delete { path: path.clone() })
        }
        PatchOperation::Update { path, hunks, move_to, .. } => {
            let key = key_of(std::path::Path::new(path));
            let data = sandbox
                .read_bytes(path)
                .await
                .map_err(|error| format!("cannot update \"{path}\": {error}"))?;
            observation.guard(&key, Some(&version_of(&data)))?;
            let before = String::from_utf8(data)
                .map_err(|_| format!("cannot update \"{path}\": file is not valid UTF-8"))?;
            let after = match apply_hunks(&before, hunks) {
                ApplyHunksResult::Ok { after } => after,
                ApplyHunksResult::Failed { failure } => {
                    return Err(format!(
                        "cannot update \"{path}\": hunk at patch line {} did not match (stage {}, \
                         searched from line {}); re-read the file and give the hunk more surrounding \
                         context",
                        failure.hunk_line_number,
                        failure.stage.as_str(),
                        failure.search_start + 1
                    ));
                }
            };
            match move_to {
                None => Ok(Planned::Update { path: path.clone(), after }),
                Some(destination) => {
                    if sandbox.read_bytes(destination).await.is_ok() {
                        return Err(format!(
                            "cannot move \"{path}\" to \"{destination}\": destination already exists"
                        ));
                    }
                    Ok(Planned::Move {
                        path: path.clone(),
                        destination: destination.clone(),
                        after,
                    })
                }
            }
        }
    }
}

/// 提交单操作并刷新观测。
async fn commit(
    sandbox: &Sandbox,
    observation: &FsObservation,
    planned: Planned,
) -> Result<String, String> {
    match planned {
        Planned::Add { path, content } => {
            let message = sandbox.write(&path, &content).await?;
            refresh(observation, sandbox, &path).await;
            Ok(format!("added {path} ({message})"))
        }
        Planned::Delete { path } => {
            let resolved = sandbox.resolve(&path)?;
            tokio::fs::remove_file(&resolved)
                .await
                .map_err(|error| format!("delete {} failed: {error}", resolved.display()))?;
            observation.record(&key_of(std::path::Path::new(&path)), Observed::Absent);
            Ok(format!("deleted {path}"))
        }
        Planned::Update { path, after } => {
            sandbox.write(&path, &after).await?;
            refresh(observation, sandbox, &path).await;
            Ok(format!("updated {path}"))
        }
        Planned::Move { path, destination, after } => {
            sandbox.write(&destination, &after).await?;
            let resolved = sandbox.resolve(&path)?;
            tokio::fs::remove_file(&resolved)
                .await
                .map_err(|error| format!("move {} failed: {error}", resolved.display()))?;
            observation.record(&key_of(std::path::Path::new(&path)), Observed::Absent);
            refresh(observation, sandbox, &destination).await;
            Ok(format!("moved {path} -> {destination}"))
        }
    }
}

/// 改动成功后刷新观测（本会话已持有新事实）。
async fn refresh(observation: &FsObservation, sandbox: &Sandbox, relative: &str) {
    let key = key_of(std::path::Path::new(relative));
    match sandbox.read_bytes(relative).await {
        Ok(data) => observation.refresh(&key, version_of(&data)),
        Err(_) => observation.record(&key, Observed::Absent),
    }
}

#[cfg(test)]
#[path = "patch_tool_tests.rs"]
mod tests;
