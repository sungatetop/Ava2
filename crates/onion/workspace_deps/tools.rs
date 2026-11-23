//! `load_workspace_dependencies` 工具定义。
//!
//! 工具薄壳：委托 `WorkspaceDepsService::resolve`，返回
//! `{python, node?, pnpm?, pythonPackages, nodePackages?, pythonDistributions}`。

use std::sync::Arc;

use serde_json::json;
use tokio_util::sync::CancellationToken;

use crate::tools::ToolPort;
use crate::types::{ContentBlock, ToolCall, ToolDefinition};
use crate::waterfall::BoxFuture;
use crate::workspace_deps::WorkspaceDepsService;

/// `load_workspace_dependencies` 工具：查询捆绑运行时的绝对路径与版本。
#[derive(Clone)]
pub struct LoadWorkspaceDependenciesTool {
    service: Arc<WorkspaceDepsService>,
}

impl LoadWorkspaceDependenciesTool {
    pub fn new(service: Arc<WorkspaceDepsService>) -> Self {
        Self { service }
    }
}

impl ToolPort for LoadWorkspaceDependenciesTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "load_workspace_dependencies".to_string(),
            description: "Get absolute paths to bundled Python and library directories, plus \
                bundled Python distribution versions. Node.js and pnpm paths are included when \
                the payload provides them. Python includes numpy, pandas, python-docx, \
                python-pptx, openpyxl, Pillow, lxml, and XlsxWriter. Use these libraries for \
                Office files unless the user or workspace instructions select another \
                environment. When Node.js and pnpm paths are returned, run pnpm with that Node \
                executable and pnpm script path. This does not change PATH or package-manager \
                settings."
                .to_string(),
            parameters: json!({ "type": "object", "properties": {} }),
        }
    }

    fn execute(&self, _call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let service = self.service.clone();
        Box::pin(async move {
            let deps = service.resolve().map_err(|e| e.to_string())?;
            Ok(vec![ContentBlock::Text {
                text: format_deps(&deps),
            }])
        })
    }

    /// 只读查询（不改变 PATH / 包管理器设置）。
    fn modes(&self) -> Option<Vec<String>> {
        None
    }

    fn risk(&self) -> crate::approval::ToolRisk {
        crate::approval::ToolRisk::ReadOnly
    }

    fn blocking(&self) -> bool {
        false
    }
}

/// 渲染查询结果：`{python, node?, pnpm?, pythonPackages, nodePackages?,
/// pythonDistributions}`（两空格缩进 JSON）。
fn format_deps(deps: &crate::workspace_deps::WorkspaceDependencies) -> String {
    let mut map = serde_json::Map::new();
    map.insert("python".into(), json!(deps.python.to_string_lossy()));
    if let Some(node) = &deps.node {
        map.insert("node".into(), json!(node.to_string_lossy()));
    }
    if let Some(pnpm) = &deps.pnpm {
        map.insert("pnpm".into(), json!(pnpm.to_string_lossy()));
    }
    map.insert("pythonPackages".into(), json!(deps.python_packages.to_string_lossy()));
    if let Some(node_packages) = &deps.node_packages {
        map.insert("nodePackages".into(), json!(node_packages.to_string_lossy()));
    }
    map.insert(
        "pythonDistributions".into(),
        json!(deps.python_distributions),
    );
    serde_json::Value::Object(map).to_string()
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
