//! 相关性召回（onion 版：确定性关键词召回，无 LLM 子查询）。
//!
//! 对齐 ava-harness::memory 设计蓝本的**关键词回退路径**：把 query 与
//! description/name 的 token 重叠打分排序，取前 `MAX_SELECTED_MEMORIES` 条。
//! ava-harness 的 LLM 子查询依赖 `ava_providers::Provider`，onion 不依赖
//! ava-\* crate，故此处仅保留确定性召回（`memory_search` 工具补足全文检索）。

use crate::memory::types::MemoryHeader;

/// 召回最多条数。
pub const MAX_SELECTED_MEMORIES: usize = 5;

/// 关键词打分召回：按 query 与 description/name 的 token 重叠排序。
pub fn keyword_relevant_memories(
    query: &str,
    headers: &[MemoryHeader],
    already_surfaced: &[String],
) -> Vec<MemoryHeader> {
    let q_tokens: Vec<String> = tokenize(query);
    if q_tokens.is_empty() {
        return Vec::new();
    }
    let mut scored: Vec<(i32, MemoryHeader)> = headers
        .iter()
        .filter(|h| !already_surfaced.iter().any(|s| s == &h.filename))
        .map(|h| {
            let hay = format!("{} {}", h.filename, h.description);
            let hay_lower = hay.to_ascii_lowercase();
            let score = q_tokens
                .iter()
                .filter(|t| hay_lower.contains(t.as_str()))
                .count() as i32;
            (score, h.clone())
        })
        .filter(|(score, _)| *score > 0)
        .collect();
    scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    scored.into_iter().take(MAX_SELECTED_MEMORIES).map(|(_, h)| h).collect()
}

fn tokenize(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

/// 格式化为文本清单（供子查询/预注入）——重导出 [`crate::memory::scan`]。
pub use crate::memory::scan::format_memory_manifest as manifest;

#[cfg(test)]
#[path = "recall_tests.rs"]
mod tests;
