//! 确定性记忆检索（`memory_search` 工具的检索内核，无 LLM）。
//!
//! 对齐 ava-harness::memory 设计蓝本：复用 [`crate::memory::scan`] 枚举的记忆头，
//! 读取正文做大小写不敏感的 token 重叠打分（token 出现在
//! `filename + description + 正文` 各计 1 分）；对 CJK 等无空格分词的语言，
//! 以"整个 trimmed query 作为正文 substring"辅助加分。空查询退化为按
//! 最近修改时间列最近记忆（对齐 mrsi 空查询语义）。

use std::fs;
use std::path::Path;

use crate::memory::frontmatter::parse_frontmatter;
use crate::memory::types::MemoryHeader;

/// 默认返回条数。
pub const DEFAULT_SEARCH_LIMIT: usize = 5;
/// 返回条数上限（入参 clamp 到 `1..=MAX_SEARCH_LIMIT`）。
pub const MAX_SEARCH_LIMIT: usize = 20;
/// CJK 整串 substring 命中的额外加分。
const CJK_SUBSTRING_BONUS: i64 = 2;
/// snippet 窗口：命中位置向前保留的字符数。
const SNIPPET_BEFORE_CHARS: usize = 40;
/// snippet 窗口总长（字符数）。
const SNIPPET_WINDOW_CHARS: usize = 160;

/// 单条检索命中。
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    /// 主题文件名。
    pub filename: String,
    /// 单行描述。
    pub description: String,
    /// 命中片段（仅 filename/description 命中或空查询时为正文开头截断）。
    pub snippet: String,
    /// 相关性得分（token 命中数 + CJK 整串加分；空查询列最近时为 0）。
    pub score: i64,
}

/// 确定性检索记忆，返回按 score 降序（同分按修改时间新者优先）的命中列表。
///
/// - `query`：检索词；trimmed 为空时退化为按最近修改时间列最近记忆。
/// - `limit`：返回条数，clamp 到 `1..=MAX_SEARCH_LIMIT`。
/// - `headers`：扫描层产出的记忆头（正文从 `file_path` 读取，读取失败跳过）。
pub fn search_memories(query: &str, limit: usize, headers: &[MemoryHeader]) -> Vec<SearchHit> {
    let limit = limit.clamp(1, MAX_SEARCH_LIMIT);
    let trimmed = query.trim();
    // 空查询：列最近（headers 已按 mtime 新→旧）。
    if trimmed.is_empty() {
        return headers
            .iter()
            .filter_map(|h| {
                let body = read_body(&h.file_path)?;
                Some(SearchHit {
                    filename: h.filename.clone(),
                    description: h.description.clone(),
                    snippet: head_snippet(&body),
                    score: 0,
                })
            })
            .take(limit)
            .collect();
    }
    let tokens = tokenize(trimmed);
    let mut hits: Vec<SearchHit> = headers
        .iter()
        .filter_map(|h| {
            let body = read_body(&h.file_path)?;
            score_header(h, &body, trimmed, &tokens)
        })
        .collect();
    // headers 已按 mtime 新→旧；稳定排序保持同分时的时序优先。
    hits.sort_by_key(|h| std::cmp::Reverse(h.score));
    hits.truncate(limit);
    hits
}

/// 对单条记忆打分：token 命中 `filename + description + 正文` 各计 1 分；
/// 整个 trimmed query（≥2 字符）作为正文 substring 额外加 2 分；无命中返回 `None`。
fn score_header(
    header: &MemoryHeader,
    body: &str,
    trimmed: &str,
    tokens: &[String],
) -> Option<SearchHit> {
    let hay_lower =
        format!("{} {} {}", header.filename, header.description, body).to_ascii_lowercase();
    let body_lower = body.to_ascii_lowercase();
    let mut score: i64 = 0;
    // 正文内最早命中字节位置（to_ascii_lowercase 保字节长度，可直接用于原文切片定位）。
    let mut first_hit: Option<usize> = None;
    for token in tokens {
        if hay_lower.contains(token.as_str()) {
            score += 1;
        }
        if let Some(pos) = body_lower.find(token.as_str()) {
            first_hit = Some(first_hit.map_or(pos, |p: usize| p.min(pos)));
        }
    }
    // CJK 辅助：整串出现在正文中，加分并参与 snippet 定位。
    if trimmed.chars().count() >= 2 {
        if let Some(pos) = body_lower.find(&trimmed.to_ascii_lowercase()) {
            score += CJK_SUBSTRING_BONUS;
            first_hit = Some(first_hit.map_or(pos, |p: usize| p.min(pos)));
        }
    }
    if score <= 0 {
        return None;
    }
    let snippet = match first_hit {
        Some(pos) => window_snippet(body, pos),
        None => head_snippet(body),
    };
    Some(SearchHit {
        filename: header.filename.clone(),
        description: header.description.clone(),
        snippet,
        score,
    })
}

/// query 分词：按非字母数字切分、转小写并去重（与 recall 的 tokenize 一致）。
fn tokenize(s: &str) -> Vec<String> {
    let mut tokens: Vec<String> = s
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    tokens.sort_unstable();
    tokens.dedup();
    tokens
}

/// 读取记忆正文（去除 frontmatter）；读取失败返回 `None`（跳过该文件）。
fn read_body(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    Some(parse_frontmatter(&content).body)
}

/// 正文开头截断为 snippet。
fn head_snippet(body: &str) -> String {
    let text: String = body.trim().chars().take(SNIPPET_WINDOW_CHARS).collect();
    collapse_ws(&text)
}

/// 命中位置向前约 40 字符、总长约 160 字符的窗口 snippet。
fn window_snippet(body: &str, hit_byte: usize) -> String {
    let chars: Vec<char> = body.chars().collect();
    let hit_char = body.get(..hit_byte).map(|s| s.chars().count()).unwrap_or(0);
    let start = hit_char.saturating_sub(SNIPPET_BEFORE_CHARS);
    let text: String = chars.iter().skip(start).take(SNIPPET_WINDOW_CHARS).collect();
    collapse_ws(&text)
}

/// 压缩连续空白为单个空格（snippet 渲染为单行，命中词原样保留）。
fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
