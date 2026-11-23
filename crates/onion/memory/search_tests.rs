//! `search` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;
use crate::memory::types::MemoryType;

fn header(filename: &str, description: &str, body: &str) -> MemoryHeader {
    let path = std::env::temp_dir().join(filename);
    fs::create_dir_all(std::env::temp_dir()).unwrap();
    fs::write(&path, body).unwrap();
    MemoryHeader {
        filename: filename.to_string(),
        file_path: path,
        mtime_ms: 0,
        description: description.to_string(),
        memory_type: Some(MemoryType::Project),
    }
}

#[test]
fn search_memories_ranks_by_token_hits() {
    let h1 = header("search_1.md", "User prefers Rust", "user prefers rust tooling\n");
    let h2 = header("search_2.md", "Release process", "release checklist\n");
    let hits = search_memories("rust", 5, &[h1, h2]);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].filename, "search_1.md");
    assert!(hits[0].score >= 1);
}

#[test]
fn search_memories_empty_query_lists_recent() {
    let h1 = header("recent_a.md", "A", "body a\n");
    let h2 = header("recent_b.md", "B", "body b\n");
    let hits = search_memories("", 5, &[h1, h2]);
    assert_eq!(hits.len(), 2);
    assert!(hits.iter().all(|h| h.score == 0));
}

#[test]
fn search_memories_limit_clamps() {
    let h1 = header("limit_a.md", "A", "a\n");
    let h2 = header("limit_b.md", "B", "b\n");
    let hits = search_memories("a", 0, &[h1, h2]);
    assert_eq!(hits.len(), 1);
}

#[test]
fn search_memories_cjk_substring_bonus() {
    let h1 = header("cjk_1.md", "中文记忆", "包含关键词的记忆\n");
    let h2 = header("cjk_2.md", "Other", "unrelated\n");
    let hits = search_memories("关键词", 5, &[h1, h2]);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].filename, "cjk_1.md");
    assert!(hits[0].score >= 2);
}
