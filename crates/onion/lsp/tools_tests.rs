//! `lsp/tools.rs` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//! 覆盖参数校验 / 坐标转换 / 无 provider 结构化失败。

use std::sync::Arc;

use serde_json::json;

use super::LspTool;
use crate::lsp::seam::LspService;
use crate::tools::ToolPort;

/// 构造一条工具调用。
fn call(operation: &str, file_path: &str, line: u64, character: u64) -> crate::types::ToolCall {
    crate::types::ToolCall {
        id: "call_1".to_string(),
        name: "lsp".to_string(),
        input: json!({
            "operation": operation,
            "file_path": file_path,
            "line": line,
            "character": character,
        }),
    }
}

#[test]
fn parse_args_converts_one_based_to_zero_based() {
    let (op, path, pos) = LspTool::parse_args(&call("goToDefinition", "src/main.rs", 3, 5)).unwrap();
    assert_eq!(op.as_str(), "goToDefinition");
    assert_eq!(path, "src/main.rs");
    assert_eq!(pos.line, 2);
    assert_eq!(pos.character, 4);
}

#[test]
fn parse_args_rejects_unknown_operation() {
    let err = LspTool::parse_args(&call("jump", "a.rs", 1, 1)).unwrap_err();
    assert!(err.contains("operation must be one of"));
}

#[test]
fn parse_args_rejects_empty_file_path() {
    let err = LspTool::parse_args(&call("hover", "  ", 1, 1)).unwrap_err();
    assert_eq!(err, "file_path must be a non-empty string");
}

#[test]
fn parse_args_rejects_non_positive_coordinates() {
    let err = LspTool::parse_args(&call("hover", "a.rs", 0, 1)).unwrap_err();
    assert!(err.contains("line must be a positive integer"));
    let err = LspTool::parse_args(&call("hover", "a.rs", 1, 0)).unwrap_err();
    assert!(err.contains("character must be a positive integer"));
}

#[test]
fn execute_no_provider_returns_unavailable() {
    let service = LspService::new();
    // 语义验证：空 seam 的 query 返回 Unavailable（execute 直接透传 seam 错误）。
    let err = service
        .query(&crate::lsp::seam::LspQueryRequest {
            operation: crate::lsp::seam::LspOperation::GoToDefinition,
            file_path: "a.rs".to_string(),
            position: crate::lsp::seam::LspPosition { line: 0, character: 0 },
            workspace_root: "/ws".to_string(),
        })
        .unwrap_err();
    assert_eq!(err.code, crate::lsp::seam::LspErrorCode::Unavailable);
    // execute 路径：无 workspace root 时返回 LSP_WORKSPACE_REQUIRED 语义错误。
    let empty_root = LspTool::new(Arc::new(service), std::path::PathBuf::new());
    let result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(empty_root.execute(call("hover", "a.rs", 1, 1), tokio_util::sync::CancellationToken::new()));
    assert!(result.is_err());
    let message = result.unwrap_err();
    assert!(message.contains("requires a session workspace cwd"));
}
