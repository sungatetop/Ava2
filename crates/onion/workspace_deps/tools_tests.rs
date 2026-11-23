//! `workspace_deps` 工具测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::sync::Arc;

use serde_json::json;
use tokio_util::sync::CancellationToken;

use crate::tools::ToolPort;
use crate::types::ToolCall;
use crate::workspace_deps::{LoadWorkspaceDependenciesTool, WorkspaceDepsService};

#[test]
fn definition_matches_reference_schema() {
    let tool = LoadWorkspaceDependenciesTool::new(Arc::new(WorkspaceDepsService::new(
        std::path::PathBuf::new(),
    )));
    let def = tool.definition();
    assert_eq!(def.name, "load_workspace_dependencies");
    // 零参数（`parameters: {}`）。
    assert_eq!(def.parameters, json!({ "type": "object", "properties": {} }));
    // description 点名捆绑的 Office 库。
    assert!(def.description.contains("numpy, pandas, python-docx"));
    assert!(def.description.contains("XlsxWriter"));
    assert!(def.description.contains("does not change PATH"));
}

#[test]
fn risk_and_modes_match_read_only_dependency_query() {
    let tool = LoadWorkspaceDependenciesTool::new(Arc::new(WorkspaceDepsService::new(
        std::path::PathBuf::new(),
    )));
    // 只读查询：ReadOnly 风险、（modes=None）、非 blocking。
    assert_eq!(tool.risk(), crate::approval::ToolRisk::ReadOnly);
    assert_eq!(tool.modes(), None);
    assert!(!tool.blocking());
}

#[tokio::test]
async fn execute_fails_closed_without_payload() {
    // source 为空（未配置）→ 执行期结构化失败（无法读取 runtime.json）。
    let tool = LoadWorkspaceDependenciesTool::new(Arc::new(WorkspaceDepsService::new(
        std::path::PathBuf::new(),
    )));
    let call = ToolCall {
        id: "call_1".into(),
        name: "load_workspace_dependencies".into(),
        input: json!({}),
    };
    let result = tool.execute(call, CancellationToken::new()).await;
    assert!(result.is_err(), "未配置 source 应结构化失败");
}
