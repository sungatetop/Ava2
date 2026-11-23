//! `office` 工具单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//!
//! 只覆盖不依赖 soffice 的参数校验与门禁路径；真实转换由 `office::mod_tests`
//! 的冒烟覆盖（无 soffice 环境自动跳过）。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::office::{ConvertTool, RecalculateTool, RenderTool, Shared};
use super::ToolPort;
use crate::attachment::AttachmentStore;
use crate::office::Engine;
use crate::sandbox::Sandbox;
use crate::types::{ContentBlock, ToolCall};

fn temp_root(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("onion_office_tool_{name}_{}", std::process::id()))
}

fn shared(root: &Path, engine: Option<Engine>) -> Arc<Shared> {
    Arc::new(Shared {
        sandbox: Arc::new(Sandbox::new(root.to_path_buf())),
        engine,
        attachments: AttachmentStore::new(root.join("attachments")),
    })
}

fn call(name: &str, input: Value) -> ToolCall {
    ToolCall { id: "call-1".to_string(), name: name.to_string(), input }
}

async fn execute(tool: &dyn ToolPort, input: Value) -> Result<Vec<ContentBlock>, String> {
    let name = tool.definition().name;
    tool.execute(call(&name, input), CancellationToken::new()).await
}

#[test]
fn definitions_declare_required_arguments() {
    let root = temp_root("definitions");
    let shared = shared(&root, None);
    let tools: Vec<Box<dyn ToolPort>> = vec![
        Box::new(ConvertTool { shared: shared.clone() }),
        Box::new(RecalculateTool { shared: shared.clone() }),
        Box::new(RenderTool { shared }),
    ];
    let names: Vec<String> = tools.iter().map(|tool| tool.definition().name).collect();
    assert_eq!(names, ["office_convert", "office_recalculate", "office_render"]);
    assert_eq!(tools[0].definition().parameters["required"], json!(["input", "format"]));
    assert_eq!(tools[1].definition().parameters["required"], json!(["input", "format"]));
    assert_eq!(tools[2].definition().parameters["required"], json!(["input"]));
}

#[tokio::test]
async fn convert_rejects_unknown_extension() {
    let root = temp_root("unknown_ext");
    let tool = ConvertTool { shared: shared(&root, None) };
    let error = execute(&tool, json!({ "input": "notes.md", "format": "pdf" }));
    assert!(error.await.expect_err("must fail").starts_with("unsupported-format"));
}

#[tokio::test]
async fn convert_requires_format_argument() {
    let root = temp_root("missing_format");
    let tool = ConvertTool { shared: shared(&root, None) };
    let error = execute(&tool, json!({ "input": "notes.txt" }));
    assert!(error.await.expect_err("must fail").starts_with("invalid-request"));
}

#[tokio::test]
async fn convert_rejects_target_outside_family() {
    let root = temp_root("target_outside");
    let tool = ConvertTool { shared: shared(&root, None) };
    let error = execute(&tool, json!({ "input": "notes.txt", "format": "pptx" }));
    assert!(error.await.expect_err("must fail").starts_with("unsupported-format"));
}

#[tokio::test]
async fn recalculate_rejects_non_matching_format() {
    let root = temp_root("recalc_mismatch");
    let tool = RecalculateTool { shared: shared(&root, None) };
    let error = execute(&tool, json!({ "input": "book.xlsx", "format": "ods" }));
    assert!(error.await.expect_err("must fail").starts_with("unsupported-format"));
}

#[tokio::test]
async fn reports_unavailable_without_engine() {
    let root = temp_root("unavailable");
    std::fs::create_dir_all(&root).expect("create sandbox root");
    std::fs::write(root.join("book.xlsx"), b"not a real workbook").expect("write fixture");
    let tool = ConvertTool { shared: shared(&root, None) };
    let error = execute(&tool, json!({ "input": "book.xlsx", "format": "pdf" }))
        .await
        .expect_err("must fail");
    let _ = std::fs::remove_dir_all(&root);
    assert!(error.starts_with("unavailable"), "{error}");
}

#[tokio::test]
async fn render_reports_read_failure_for_missing_input() {
    let root = temp_root("render_missing");
    let tool = RenderTool { shared: shared(&root, None) };
    let error = execute(&tool, json!({ "input": "missing.pdf" }));
    assert!(error.await.expect_err("must fail").starts_with("failed"));
}
