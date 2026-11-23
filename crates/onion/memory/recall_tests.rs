//! `recall` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;
use crate::memory::types::MemoryType;

fn header(filename: &str, description: &str) -> MemoryHeader {
    MemoryHeader {
        filename: filename.to_string(),
        file_path: std::path::PathBuf::from(filename),
        mtime_ms: 0,
        description: description.to_string(),
        memory_type: Some(MemoryType::Project),
    }
}

#[test]
fn keyword_relevant_memories_scores_by_token_overlap() {
    let headers = vec![
        header("rust_pref.md", "User prefers Rust"),
        header("release.md", "Release process"),
    ];
    let hits = keyword_relevant_memories("rust tooling", &headers, &[]);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].filename, "rust_pref.md");
}

#[test]
fn keyword_relevant_memories_excludes_already_surfaced() {
    let headers = vec![header("rust_pref.md", "User prefers Rust")];
    let surfaced = vec!["rust_pref.md".to_string()];
    let hits = keyword_relevant_memories("rust", &headers, &surfaced);
    assert!(hits.is_empty());
}

#[test]
fn keyword_relevant_memories_empty_query_returns_empty() {
    let headers = vec![header("rust_pref.md", "User prefers Rust")];
    let hits = keyword_relevant_memories("", &headers, &[]);
    assert!(hits.is_empty());
}

#[test]
fn keyword_relevant_memories_caps_at_max() {
    let headers: Vec<MemoryHeader> = (0..10)
        .map(|i| header(&format!("mem_{i}.md"), "common keyword"))
        .collect();
    let hits = keyword_relevant_memories("common keyword", &headers, &[]);
    assert_eq!(hits.len(), MAX_SELECTED_MEMORIES);
}

#[test]
fn keyword_relevant_memories_ranked_by_score() {
    let headers = vec![
        header("one_hit.md", "A B"),
        header("two_hit.md", "A B C"),
    ];
    let hits = keyword_relevant_memories("A B C", &headers, &[]);
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].filename, "two_hit.md");
}
