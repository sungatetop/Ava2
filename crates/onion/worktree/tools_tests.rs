//! `tools` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::sync::Arc;

use serde_json::json;

use crate::tools::ToolPort;
use crate::types::ToolCall;
use crate::worktree::tools::CreateWorktreeTool;
use crate::worktree::seam::WorktreeService;

#[test]
fn definition_matches_reference_schema() {
    let svc = Arc::new(WorktreeService::new());
    let tool = CreateWorktreeTool::new(svc);
    let def = tool.definition();
    assert_eq!(def.name, "create_worktree");
    // description 覆盖 name/from/HEAD/自动命名/未提交保留/已存在失败/离开方式。
    let desc = def.description.to_lowercase();
    assert!(desc.contains("worktree"));
    assert!(desc.contains("branch"));
    assert!(desc.contains("working directory"));
    // 参数：name + from（均可选）。
    let props = def.parameters["properties"].as_object().expect("properties object");
    assert!(props.contains_key("name"));
    assert!(props.contains_key("from"));
    assert!(def.parameters["additionalProperties"] == json!(false));
}

#[test]
fn modes_and_risk_match_work_tree_tool() {
    let svc = Arc::new(WorktreeService::new());
    let tool = CreateWorktreeTool::new(svc);
    let modes = tool.modes().expect("modes");
    assert_eq!(modes, vec!["work".to_string()]);
    assert_eq!(tool.risk(), crate::approval::ToolRisk::WorkspaceWrite);
    assert!(!tool.blocking());
}

#[tokio::test]
async fn execute_requires_owning_session() {
    let svc = Arc::new(WorktreeService::new());
    let tool = CreateWorktreeTool::new(svc);
    let call = ToolCall {
        id: "call-1".into(),
        name: "create_worktree".into(),
        input: json!({}),
    };
    let result = tool.execute(call, tokio_util::sync::CancellationToken::new()).await;
    let err = result.expect_err("no owning session should fail");
    assert!(err.contains("requires an owning agent session"));
}
