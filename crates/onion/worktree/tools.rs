//! `create_worktree` 工具定义。
//!
//! 工具薄壳：校验调用方有 agent 后委托 `WorktreeService::create`，返回
//! `{path, branch, baseCommit, repositoryRoot}`。

use std::sync::Arc;

use serde_json::json;
use tokio_util::sync::CancellationToken;

use crate::tools::ToolPort;
use crate::types::{ContentBlock, ToolCall, ToolDefinition};
use crate::waterfall::BoxFuture;
use crate::worktree::{CreateWorktreeRequest, WorktreeService};

/// `create_worktree` 工具：创建 git worktree 并切换会话工作目录。
#[derive(Clone)]
pub struct CreateWorktreeTool {
    service: Arc<WorktreeService>,
}

impl CreateWorktreeTool {
    pub fn new(service: Arc<WorktreeService>) -> Self {
        Self { service }
    }
}

impl ToolPort for CreateWorktreeTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "create_worktree".to_string(),
            description: "Create a new Git branch and worktree from a local commit, branch, or tag, \
                then change this session's working directory to it. Defaults to HEAD and a generated \
                name. Uncommitted files stay in the source checkout. Existing names fail. Use \
                working_directory with cd to leave; the checkout and branch remain."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "New branch name, also used as the checkout directory name. Omit to auto-generate a unique name." },
                    "from": { "type": "string", "description": "Local commit, branch, or tag to start from. Defaults to HEAD. Does not fetch." }
                },
                "additionalProperties": false
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let service = self.service.clone();
        Box::pin(async move {
            let name = call.input.get("name").and_then(serde_json::Value::as_str).map(str::to_string);
            let from = call.input.get("from").and_then(serde_json::Value::as_str).map(str::to_string);
            let created = service
                .create(CreateWorktreeRequest { name, from })
                .await?;
            Ok(vec![ContentBlock::Text {
                text: format_create(&created),
            }])
        })
    }

    /// worktree 创建有文件系统 / git 副作用，标记 WorkspaceWrite（写面工具）；
    /// 可见性由当前 Agent 工具面决定（coding 预设包含）。
    fn modes(&self) -> Option<Vec<String>> {
        Some(vec!["work".to_string()])
    }

    fn risk(&self) -> crate::approval::ToolRisk {
        crate::approval::ToolRisk::WorkspaceWrite
    }

    fn blocking(&self) -> bool {
        false
    }
}

/// 渲染创建结果：`{path, branch, baseCommit, repositoryRoot}`（JSON 序列化）。
fn format_create(created: &crate::worktree::CreatedWorktree) -> String {
    json!({
        "path": created.path.to_string_lossy(),
        "branch": created.branch,
        "baseCommit": created.base_commit,
        "repositoryRoot": created.repository_root.to_string_lossy(),
    })
    .to_string()
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
