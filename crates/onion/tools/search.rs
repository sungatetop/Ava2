//! tool_search：deferred 工具检索 + 解锁（两段式调用的第一段）。
//!
//! 语义参照本 Agent 环境的 ToolSearch → DeferExecuteTool：deferred 工具平时只下发
//! stub（空 parameters + 提示后缀），模型先 search 拿到完整 schema 全文（进入对话
//! 历史），再按全文直接构造正式调用；命中即解锁（会话级，一次长期有效）。
//! 搜索范围 = 当前模式下全部 deferred 工具（与模式准入边界一致）。

use std::sync::Arc;

use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::{ToolPort, ToolRegistry, ToolState};
use crate::types::{ContentBlock, ToolCall, ToolDefinition};
use crate::waterfall::BoxFuture;

/// tool_search：按关键词检索 deferred 工具，返回完整 schema 并解锁命中项。
pub struct ToolSearchTool {
    /// 注册表快照（构造于 tool_search 注册之前，不含自身；检索范围实时按当前模式求值）。
    registry: ToolRegistry,
    state: Arc<ToolState>,
}

impl ToolSearchTool {
    pub fn new(registry: ToolRegistry, state: Arc<ToolState>) -> Self {
        Self { registry, state }
    }
}

impl ToolPort for ToolSearchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "tool_search".to_string(),
            description: "Search deferred tools by keyword and load their full schemas. Call this \
                before invoking a deferred tool. Input: {\"query\": string} (empty matches all)."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "whitespace-separated keywords, case-insensitive; empty = all deferred tools" }
                },
                "required": ["query"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let registry = self.registry.clone();
        let state = self.state.clone();
        Box::pin(async move {
            let query = call.input.get("query").and_then(Value::as_str).unwrap_or("");
            let matched = match_tools(&registry.deferred_definitions(), query);
            if matched.is_empty() {
                return Ok(vec![ContentBlock::Text {
                    text: format!("no deferred tools match query \"{query}\""),
                }]);
            }
            for tool in &matched {
                state.unlock(&tool.name);
            }
            let body = matched.iter().map(format_tool).collect::<Vec<_>>().join("\n\n");
            Ok(vec![ContentBlock::Text { text: body }])
        })
    }
}

/// 关键词匹配：query 按空白分词，任一词命中 name/description 子串即匹配
/// （大小写不敏感）；空 query 返回全部。
fn match_tools(candidates: &[ToolDefinition], query: &str) -> Vec<ToolDefinition> {
    let terms: Vec<String> = query.split_whitespace().map(str::to_ascii_lowercase).collect();
    if terms.is_empty() {
        return candidates.to_vec();
    }
    candidates
        .iter()
        .filter(|tool| {
            let name = tool.name.to_ascii_lowercase();
            let description = tool.description.to_ascii_lowercase();
            terms.iter().any(|term| name.contains(term) || description.contains(term))
        })
        .cloned()
        .collect()
}

/// 单个匹配结果的展示格式：name / description / parameters 全文（JSON 序列化）。
fn format_tool(tool: &ToolDefinition) -> String {
    format!(
        "name: {}\ndescription: {}\nparameters: {}",
        tool.name, tool.description, tool.parameters
    )
}
