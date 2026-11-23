//! `working_directory` 工具：读 / 改会话工作目录。
//!
//! 核心语义：
//! - 输入 `cd?: string`：省略读当前目录，提供则改目录（相对路径用当前目录解析）；
//! - 返回值 `{cwd}`：当前绝对工作目录；
//! - 权限根不变：`cd` 目标须解析到权限根内（「权限根不变」约束），
//!   越出权限根的绝对路径 / `..` 逃逸拒绝；
//! - 变更经 `SessionCwd::set` 提交（唯一写入口：写 durable 事件 + 更新投影）；
//! - 只读（不改文件系统，仅变更会话 cwd 状态）、非 deferred；
//! - 无 owning session（非 agent 调用方）拒绝。

use std::sync::Arc;

use serde_json::json;
use tokio_util::sync::CancellationToken;

use crate::session_cwd::SessionCwd;
use crate::tools::ToolPort;
use crate::types::{ContentBlock, ToolCall, ToolDefinition};
use crate::waterfall::BoxFuture;

/// `working_directory` 工具：薄壳，全部状态与事件写入下沉到 `SessionCwd`。
#[derive(Clone)]
pub struct WorkingDirectoryTool {
    cwd: Arc<SessionCwd>,
}

impl WorkingDirectoryTool {
    /// 以独立 `SessionCwd` 构造（单会话 / 测试场景）。
    pub fn new(root: std::path::PathBuf) -> Self {
        Self::with_cwd(Arc::new(SessionCwd::new(root)))
    }

    /// 以共享的会话目录句柄构造（生产装配路径）。
    pub fn with_cwd(cwd: Arc<SessionCwd>) -> Self {
        Self { cwd }
    }

    /// 延迟接线会话（宿主装配完成后调用）；重放恢复 cwd。
    pub fn wire_session(&self, session: Arc<crate::session::SessionLog>) {
        self.cwd.wire_session(session);
    }
}

impl ToolPort for WorkingDirectoryTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "working_directory".to_string(),
            description: "Read the current working directory, or change it with cd. Relative paths use \
                the current directory. Existing shells and running processes keep their own directories."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "cd": { "type": "string", "description": "Existing directory to enter. Omit to read the current directory." }
                }
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let cwd = self.cwd.clone();
        Box::pin(async move {
            // 非 agent 调用方（未接线会话）拒绝：目录变更须有 durable 归属。
            if !cwd.is_wired() {
                return Err("working_directory requires an owning agent session".to_string());
            }
            let cwd_param = call.input.get("cd").and_then(serde_json::Value::as_str);
            let resolved = match cwd_param {
                None => cwd.ensure(),
                Some(cd) => {
                    if cd.trim().is_empty() {
                        return Err("cd must be a non-empty path".to_string());
                    }
                    cwd.set(cwd.resolve(cd)?)?
                }
            };
            Ok(vec![ContentBlock::Text {
                text: json!({ "cwd": resolved.to_string_lossy() }).to_string(),
            }])
        })
    }
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
