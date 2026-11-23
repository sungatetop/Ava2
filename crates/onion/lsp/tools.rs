//! `lsp` 工具：一个只读工具、四种操作
//! （goToDefinition / findReferences / goToImplementation / hover）。把 one-based UTF-16
//! 光标坐标转换为 seam 的 zero-based 位置，要求会话 workspace（取活动目录，
//! 无 fallback），截断并渲染结果，并挂可配置超时预算。
//!
//! 无 provider（缺省）时返回结构化 `LSP_UNAVAILABLE`。只读、非 deferred。

use std::sync::Arc;

use serde_json::json;
use tokio_util::sync::CancellationToken;

use crate::lsp::render::{format_hover, format_locations, DEFAULT_MAX_LOCATIONS, DEFAULT_MAX_RESULT_CHARS};
use crate::lsp::seam::{LspOperation, LspPosition, LspQueryRequest, LspQueryResult, LspService};
use crate::session_cwd::SessionCwd;
use crate::tools::ToolPort;
use crate::types::{ContentBlock, ToolCall, ToolDefinition};
use crate::waterfall::BoxFuture;

/// 缺省工具调用超时预算（ms），覆盖排队 open / query / close 生命周期。
pub const DEFAULT_LSP_TOOL_TIMEOUT_MS: u64 = 60_000;

/// 稳定 system-prompt 指引，把 LSP 定位为精确辅助。
pub const LSP_PROMPT_TEXT: &str =
    "Use search/read for ordinary navigation. Use lsp when textual matches are ambiguous or \
before a change requires precise definitions, implementations, or references. Positions are \
one-based line and character (UTF-16) at the cursor; an off-symbol position may return no \
results. findReferences always includes the declaration.";

/// `lsp` 工具：LSP seam + 会话目录句柄（查询基准为活动目录，权限根不变）。
pub struct LspTool {
    service: Arc<LspService>,
    /// 会话目录句柄：查询的解析与索引基准取活动目录。
    cwd: Arc<SessionCwd>,
    /// 渲染 location 条数上限。
    max_locations: usize,
    /// 渲染结果字符上限。
    max_result_chars: usize,
}

impl LspTool {
    /// 以独立 `SessionCwd` 构造（单会话 / 测试场景）。
    pub fn new(service: Arc<LspService>, workspace_root: std::path::PathBuf) -> Self {
        Self::with_cwd(service, Arc::new(SessionCwd::new(workspace_root)))
    }

    /// 以共享的会话目录句柄构造（生产装配路径）。
    pub fn with_cwd(service: Arc<LspService>, cwd: Arc<SessionCwd>) -> Self {
        Self {
            service,
            cwd,
            max_locations: DEFAULT_MAX_LOCATIONS,
            max_result_chars: DEFAULT_MAX_RESULT_CHARS,
        }
    }

    /// 校验并转换模型参数：`operation` 须为四操作之一；`line`/`character` 为 one-based
    /// 正整数，转换为 seam 的 zero-based 位置。
    fn parse_args(call: &ToolCall) -> Result<(LspOperation, String, LspPosition), String> {
        let operation = call
            .input
            .get("operation")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "operation is required".to_string())?;
        let operation = LspOperation::parse(operation).ok_or_else(|| {
            let ops = LspOperation::ALL.iter().map(LspOperation::as_str).collect::<Vec<_>>().join(", ");
            format!("operation must be one of {ops}")
        })?;
        let file_path = call
            .input
            .get("file_path")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "file_path is required".to_string())?;
        if file_path.trim().is_empty() {
            return Err("file_path must be a non-empty string".to_string());
        }
        let line = one_based(call, "line")?;
        let character = one_based(call, "character")?;
        // 模型从 1 起计；seam（与协议）从 0 起计。
        Ok((
            operation,
            file_path.to_string(),
            LspPosition { line: line - 1, character: character - 1 },
        ))
    }
}

/// 校验 one-based 坐标为「正整数」；否则报错。
fn one_based(call: &ToolCall, name: &str) -> Result<u32, String> {
    let value = call
        .input
        .get(name)
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| format!("{name} must be a positive integer (one-based)"))?;
    if value < 1 {
        return Err(format!("{name} must be a positive integer (one-based)"));
    }
    Ok(value as u32)
}

impl ToolPort for LspTool {
    fn definition(&self) -> ToolDefinition {
        let ops = LspOperation::ALL
            .iter()
            .map(LspOperation::as_str)
            .collect::<Vec<_>>()
            .join(", ");
        ToolDefinition {
            name: "lsp".to_string(),
            description: format!(
                "Query a language server for precise code navigation. operation is one of \
{ops}. line and character are one-based UTF-16 cursor coordinates. findReferences includes \
the declaration."
            ),
            parameters: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": LspOperation::ALL.iter().map(LspOperation::as_str).collect::<Vec<_>>(),
                        "description": "goToDefinition, findReferences, goToImplementation, or hover."
                    },
                    "file_path": { "type": "string", "description": "The source file to query, relative to the workspace or absolute." },
                    "line": { "type": "integer", "description": "One-based line of the cursor." },
                    "character": { "type": "integer", "description": "One-based UTF-16 column of the cursor." }
                },
                "required": ["operation", "file_path", "line", "character"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let service = self.service.clone();
        let cwd = self.cwd.clone();
        let max_locations = self.max_locations;
        let max_result_chars = self.max_result_chars;
        Box::pin(async move {
            let (operation, file_path, position) = Self::parse_args(&call)?;
            // workspace 根必填，无 fallback（`LSP_WORKSPACE_REQUIRED`）。
            let workspace_root = cwd.ensure().to_string_lossy().to_string();
            if workspace_root.trim().is_empty() {
                return Err("the lsp tool requires a session workspace cwd".to_string());
            }
            let request = LspQueryRequest {
                operation,
                file_path,
                position,
                workspace_root,
            };
            let result = service
                .query(&request)
                .map_err(|e| format!("{} (code={:?})", e.message, e.code))?;
            let text = match result {
                LspQueryResult::Locations { locations, resolved_workspace_uri } => {
                    format_locations(&locations, &resolved_workspace_uri, max_locations, max_result_chars)
                }
                LspQueryResult::Hover { hover } => format_hover(&hover, max_result_chars),
            };
            Ok(vec![ContentBlock::Text { text }])
        })
    }
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
