//! 记忆工具：`SaveMemory` / `UpdateMemory` / `RecallMemory` / `SearchMemories` /
//! `ListMemories` / `ReadMemory` / `DeleteMemory`。
//!
//! 对齐 ava-harness::memory 设计蓝本：写入统一走
//! [`MemoryStore::save`] 的结构化路径（name/type/description/body → frontmatter +
//! `MEMORY.md` 索引自动维护），删除走 [`MemoryStore::delete`]（文件与索引行一并
//! 移除），agent 不手写 frontmatter、不手工维护索引。所有工具都做路径边界校验
//! （只读/写记忆目录内）。工具实现 onion 的 [`ToolPort`]（不依赖 ava-\*）。

use std::sync::Arc;

use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use crate::memory::recall::keyword_relevant_memories;
use crate::memory::scan::scan_memory_files;
use crate::memory::search::{search_memories, DEFAULT_SEARCH_LIMIT};
use crate::memory::store::MemoryStore;
use crate::memory::types::parse_memory_type;
use crate::tools::ToolPort;
use crate::types::{ContentBlock, ToolCall, ToolDefinition};
use crate::waterfall::BoxFuture;

/// 文本结果归一：模型面文本 + 结构化 JSON 文本（UI/日志面可解析）。
fn result_text(value: Value) -> Vec<ContentBlock> {
    vec![ContentBlock::Text { text: value.to_string() }]
}

/// `SaveMemory` —— 保存一条跨会话记忆（只读风险：写入经 [`MemoryStore::save`]
/// 结构化路径，路径边界锁定记忆目录内，不触碰工作区文件）。
pub struct SaveMemory {
    store: MemoryStore,
}

impl SaveMemory {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }
}

impl ToolPort for SaveMemory {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "SaveMemory".into(),
            description: "Save a cross-session memory. Type is one of: user, feedback, project, reference. \
                 Provide a concise one-line description used for later relevance matching, and the \
                 memory body. Only save non-obvious facts not derivable from the project state."
                .into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Memory name/title" },
                    "type": { "type": "string", "enum": ["user", "feedback", "project", "reference"] },
                    "description": { "type": "string", "description": "One-line description (relevance keywords)" },
                    "body": { "type": "string", "description": "Memory content" }
                },
                "required": ["name", "type", "description", "body"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let arguments = call.input;
        let store = self.store.clone();
        Box::pin(async move {
            if cancel.is_cancelled() {
                return Ok(result_text(json!({ "cancelled": true })));
            }
            let name = arguments.get("name").and_then(Value::as_str).unwrap_or("").to_string();
            let ty = arguments.get("type").and_then(Value::as_str).unwrap_or("");
            let description = arguments.get("description").and_then(Value::as_str).unwrap_or("").to_string();
            let body = arguments.get("body").and_then(Value::as_str).unwrap_or("").to_string();
            if name.trim().is_empty() {
                return Err("save_memory: 'name' is required".to_string());
            }
            let memory_type = parse_memory_type(ty).ok_or_else(|| {
                format!("save_memory: invalid type '{ty}' (expected user|feedback|project|reference)")
            })?;
            let path = store
                .save(&name, memory_type, &description, &body)
                .map_err(|e| format!("save_memory: {e}"))?;
            Ok(result_text(json!({ "ok": true, "path": path.to_string_lossy() })))
        })
    }
}

/// `RecallMemory` —— 按关键词召回相关记忆（只读）。
pub struct RecallMemory {
    store: MemoryStore,
}

impl RecallMemory {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }
}

impl ToolPort for RecallMemory {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "RecallMemory".into(),
            description: "Recall memories relevant to a query. Returns matching memory filenames and \
                 descriptions so you can then read the specific files. Use when the user references \
                 prior context or you need remembered facts."
                .into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "What to recall about" }
                },
                "required": ["query"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let arguments = call.input;
        let store = self.store.clone();
        Box::pin(async move {
            if cancel.is_cancelled() {
                return Ok(result_text(json!({ "cancelled": true })));
            }
            let query = arguments.get("query").and_then(Value::as_str).unwrap_or("").to_string();
            let headers = scan_memory_files(store.root())
                .map_err(|e| format!("RecallMemory: {e}"))?;
            let hits = keyword_relevant_memories(&query, &headers, &[]);
            let items: Vec<Value> = hits
                .iter()
                .map(|h| {
                    json!({
                        "filename": h.filename,
                        "type": h.memory_type.map(|t| t.as_str()).unwrap_or("unknown"),
                        "description": h.description,
                    })
                })
                .collect();
            Ok(result_text(json!({ "query": query, "memories": items })))
        })
    }
}

/// `SearchMemories`（`memory_search`）—— 确定性全文检索记忆（无 LLM，搜正文；
/// 只读）。与 LLM 召回的 [`RecallMemory`] 互补：本工具搜记忆正文，
/// recall 仅按 filename/description 由子查询语义选择。
pub struct SearchMemories {
    store: MemoryStore,
}

impl SearchMemories {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }
}

impl ToolPort for SearchMemories {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "memory_search".into(),
            description: "Search the persistent memory store for prior facts, preferences, lessons, and \
                 project notes. Rank combines text relevance and recency, and the full memory \
                 body is searched. Search BEFORE repeating work a prior session may have already \
                 learned. Pass an empty query to list the most recent memories."
                .into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Keywords to search for; pass an empty string to list the most recent memories" },
                    "limit": { "type": "integer", "description": "Max results (default 5, max 20)" }
                },
                "required": ["query"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let arguments = call.input;
        let store = self.store.clone();
        Box::pin(async move {
            if cancel.is_cancelled() {
                return Ok(result_text(json!({ "cancelled": true })));
            }
            let query = arguments.get("query").and_then(Value::as_str).unwrap_or("").to_string();
            let limit = arguments
                .get("limit")
                .and_then(Value::as_u64)
                .map(|v| v as usize)
                .unwrap_or(DEFAULT_SEARCH_LIMIT);
            let headers = scan_memory_files(store.root())
                .map_err(|e| format!("memory_search: {e}"))?;
            let hits = search_memories(&query, limit, &headers);
            if hits.is_empty() {
                return Ok(vec![ContentBlock::Text {
                    text: "No matching memories.".into(),
                }]);
            }
            let text = hits
                .iter()
                .map(|h| format!("- [{}] {} — {}", h.filename, h.description, h.snippet))
                .collect::<Vec<_>>()
                .join("\n");
            Ok(vec![ContentBlock::Text { text }])
        })
    }
}

/// `ListMemories` —— 列出记忆目录与索引（只读）。
pub struct ListMemories {
    store: MemoryStore,
}

impl ListMemories {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }
}

impl ToolPort for ListMemories {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "ListMemories".into(),
            description: "List all memory topic files with their type and one-line description, plus the \
                 MEMORY.md index."
                .into(),
            parameters: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    fn execute(&self, _call: ToolCall, cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let store = self.store.clone();
        Box::pin(async move {
            if cancel.is_cancelled() {
                return Ok(result_text(json!({ "cancelled": true })));
            }
            let entrypoint = store.entrypoint().map_err(|e| format!("ListMemories: {e}"))?;
            let headers = store.list().map_err(|e| format!("ListMemories: {e}"))?;
            let items: Vec<Value> = headers
                .iter()
                .map(|h| {
                    json!({
                        "filename": h.filename,
                        "type": h.memory_type.map(|t| t.as_str()).unwrap_or("unknown"),
                        "description": h.description,
                    })
                })
                .collect();
            Ok(result_text(json!({ "index": entrypoint.trim(), "memories": items })))
        })
    }
}

/// `ReadMemory` —— 读取一条记忆的完整内容（只读）。
pub struct ReadMemory {
    store: MemoryStore,
}

impl ReadMemory {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }
}

impl ToolPort for ReadMemory {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "ReadMemory".into(),
            description: "Read one memory topic file. Returns its name, type, description and body."
                .into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "filename": { "type": "string", "description": "Memory filename, e.g. user-role.md" }
                },
                "required": ["filename"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let arguments = call.input;
        let store = self.store.clone();
        Box::pin(async move {
            if cancel.is_cancelled() {
                return Ok(result_text(json!({ "cancelled": true })));
            }
            let filename = arguments.get("filename").and_then(Value::as_str).unwrap_or("").trim();
            if filename.is_empty() {
                return Err("ReadMemory: 'filename' is required".to_string());
            }
            let file = store
                .load(filename)
                .map_err(|e| format!("ReadMemory: {e}"))?
                .ok_or_else(|| format!("ReadMemory: memory not found: {filename}"))?;
            Ok(result_text(json!({
                "filename": file.filename,
                "name": file.name,
                "type": file.memory_type.map(|t| t.as_str()).unwrap_or("unknown"),
                "description": file.description,
                "body": file.body,
            })))
        })
    }
}

/// `UpdateMemory` —— 按 filename 部分更新一条记忆（只读风险，同 SaveMemory：
/// 落点锁定记忆目录内，frontmatter/索引由代码维护）。
pub struct UpdateMemory {
    store: MemoryStore,
}

impl UpdateMemory {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }
}

impl ToolPort for UpdateMemory {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "UpdateMemory".into(),
            description: "Partially update one memory topic file by filename: only the fields you provide \
                 are changed. `body` replaces the whole content — ReadMemory first if you need to \
                 merge instead of overwrite. Renaming is not supported (delete + save instead)."
                .into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "filename": { "type": "string", "description": "Memory filename, e.g. user-role.md" },
                    "type": { "type": "string", "enum": ["user", "feedback", "project", "reference"],
                              "description": "New memory type (optional)" },
                    "description": { "type": "string", "description": "New one-line description (optional)" },
                    "body": { "type": "string", "description": "New full memory content (optional, replaces whole body)" }
                },
                "required": ["filename"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let arguments = call.input;
        let store = self.store.clone();
        Box::pin(async move {
            if cancel.is_cancelled() {
                return Ok(result_text(json!({ "cancelled": true })));
            }
            let filename = arguments.get("filename").and_then(Value::as_str).unwrap_or("").trim();
            if filename.is_empty() {
                return Err("UpdateMemory: 'filename' is required".to_string());
            }
            let memory_type = match arguments.get("type").and_then(Value::as_str) {
                None => None,
                Some(ty) => Some(parse_memory_type(ty).ok_or_else(|| {
                    format!("UpdateMemory: invalid type '{ty}' (expected user|feedback|project|reference)")
                })?),
            };
            let description = arguments
                .get("description")
                .and_then(Value::as_str)
                .map(str::to_string);
            let body = arguments.get("body").and_then(Value::as_str).map(str::to_string);
            if memory_type.is_none() && description.is_none() && body.is_none() {
                return Err("UpdateMemory: provide at least one of 'type' | 'description' | 'body'".to_string());
            }
            let path = store
                .update(
                    filename,
                    memory_type,
                    description.as_deref(),
                    body.as_deref(),
                )
                .map_err(|e| format!("UpdateMemory: {e}"))?;
            Ok(result_text(json!({ "ok": true, "path": path.to_string_lossy() })))
        })
    }
}

/// `DeleteMemory` —— 删除一条记忆（文件 + 索引行，写风险，需审批）。
pub struct DeleteMemory {
    store: MemoryStore,
}

impl DeleteMemory {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }
}

impl ToolPort for DeleteMemory {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "DeleteMemory".into(),
            description: "Delete one memory topic file; its MEMORY.md index line is removed automatically. \
                 MEMORY.md itself cannot be deleted."
                .into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "filename": { "type": "string", "description": "Memory filename to delete" }
                },
                "required": ["filename"]
            }),
        }
    }

    fn risk(&self) -> crate::approval::ToolRisk {
        crate::approval::ToolRisk::WorkspaceWrite
    }

    fn execute(&self, call: ToolCall, cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let arguments = call.input;
        let store = self.store.clone();
        Box::pin(async move {
            if cancel.is_cancelled() {
                return Ok(result_text(json!({ "cancelled": true })));
            }
            let filename = arguments.get("filename").and_then(Value::as_str).unwrap_or("").trim();
            if filename.is_empty() {
                return Err("DeleteMemory: 'filename' is required".to_string());
            }
            if filename.eq_ignore_ascii_case(crate::memory::index::ENTRYPOINT_NAME) {
                return Err("DeleteMemory: MEMORY.md cannot be deleted".to_string());
            }
            store
                .delete(filename)
                .map_err(|e| format!("DeleteMemory: {e}"))?;
            Ok(result_text(json!({ "ok": true, "deleted": filename })))
        })
    }
}

/// 便捷：返回注册进引擎的记忆工具列表（主会话用）。
pub fn memory_tools(store: MemoryStore) -> Vec<Arc<dyn ToolPort>> {
    vec![
        Arc::new(SaveMemory::new(store.clone())),
        Arc::new(UpdateMemory::new(store.clone())),
        Arc::new(RecallMemory::new(store.clone())),
        Arc::new(ReadMemory::new(store.clone())),
        Arc::new(SearchMemories::new(store)),
    ]
}

/// 便捷：主会话工具面（7 件唯一）。`memory_tools` 与 `agent_memory_tools` 有
/// Read/Save/Update 三件重名，直接叠加注册会让同名 schema 重复下发（注册表按
/// Vec 保序、不去重），故装配侧统一用本集合。
pub fn session_memory_tools(store: MemoryStore) -> Vec<Arc<dyn ToolPort>> {
    vec![
        Arc::new(SaveMemory::new(store.clone())),
        Arc::new(UpdateMemory::new(store.clone())),
        Arc::new(RecallMemory::new(store.clone())),
        Arc::new(ReadMemory::new(store.clone())),
        Arc::new(SearchMemories::new(store.clone())),
        Arc::new(ListMemories::new(store.clone())),
        Arc::new(DeleteMemory::new(store)),
    ]
}

/// 便捷：记忆 fork-agent 的工具集合（List/Read/Save/Update/Delete；RecallMemory
/// 仅供主会话按需召回，agent 直接 list/read 即可）。
pub fn agent_memory_tools(store: MemoryStore) -> Vec<Arc<dyn ToolPort>> {
    vec![
        Arc::new(ListMemories::new(store.clone())),
        Arc::new(ReadMemory::new(store.clone())),
        Arc::new(SaveMemory::new(store.clone())),
        Arc::new(UpdateMemory::new(store.clone())),
        Arc::new(DeleteMemory::new(store)),
    ]
}

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tests;
