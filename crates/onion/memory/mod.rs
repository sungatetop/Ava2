//! `memory` — 跨会话持久记忆规范模块（对齐 ava-harness::memory 设计蓝本，
//! onion 自包含实现，不依赖任何 ava-\* crate）。
//!
//! 自动记忆（memdir / auto-memory）机制：纯文件存储、
//! 无数据库、无向量库。记忆就是 Markdown + 目录：
//!
//! ```text
//! <ONION_HOME 或 ~/.onion>/projects/<normalized-cwd>/memory/
//! ├── MEMORY.md                    # 索引（每次会话加载，有界）
//! ├── user_role.md                 # 用户记忆
//! ├── feedback_testing.md          # 反馈记忆
//! ├── project_release_freeze.md    # 项目记忆
//! └── reference_linear_ingest.md   # 参考记忆
//! ```
//!
//! 核心概念：
//! - **四型封闭分类**：`user / feedback / project / reference`。
//! - **只存"项目状态不可推导"的信息**：代码、架构、git 历史不存记忆。
//! - **索引有界**：`MEMORY.md` 有 200 行 / 25KB 双上限，避免污染上下文。
//! - **保存两步**：① 写主题文件；② 在 `MEMORY.md` 加一行指针。
//! - **召回**：扫描 frontmatter → 确定性关键词打分（onion 无 LLM 子查询，
//!   由 `memory_search` 补足全文检索）。
//!
//! ## 职责边界（规范层，非运行层）
//!
//! 本模块只定义记忆的**规范**：格式（frontmatter/索引）、类型（四型）、
//! 读写（`MemoryStore`）、扫描（`scan`）、检索（`search`）、
//! 召回（`recall`）、工具（7 件，实现 [`crate::tools::ToolPort`]）。
//!
//! **不包含** fork-agent 运行时：会话结束后的记忆沉淀由编排层完成
//! （onion 无 fork-agent 运行时，仅提供工具面）。

pub mod frontmatter;
pub mod index;
pub mod paths;
pub mod recall;
pub mod scan;
pub mod search;
pub mod store;
pub mod tool;
pub mod types;

pub use frontmatter::{parse_frontmatter, render_frontmatter, strip_html_comments, MemoryFrontmatter};
pub use index::{
    format_index_line, read_entrypoint, remove_index_line, truncate_entrypoint, upsert_index_line,
    ENTRYPOINT_NAME, MAX_ENTRYPOINT_BYTES, MAX_ENTRYPOINT_LINES, MAX_INDEX_LINE_CHARS,
};
pub use paths::{validate_memory_path, MemoryPaths};
pub use recall::keyword_relevant_memories;
pub use scan::{
    diff_files, format_memory_manifest, scan_memory_files, snapshot_files, MAX_SCAN_RESULTS,
    SCAN_HEAD_LINES,
};
pub use search::{search_memories, SearchHit, DEFAULT_SEARCH_LIMIT, MAX_SEARCH_LIMIT};
pub use store::{MemoryFile, MemoryStore};
pub use tool::{
    agent_memory_tools, memory_tools, session_memory_tools, DeleteMemory, ListMemories, ReadMemory,
    RecallMemory, SaveMemory, SearchMemories, UpdateMemory,
};
pub use types::{parse_memory_type, MemoryHeader, MemoryType};

/// 错误类型。
#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid memory path: {0}")]
    InvalidPath(String),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

/// crate 级 `Result` 别名。
pub type Result<T> = std::result::Result<T, MemoryError>;

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
