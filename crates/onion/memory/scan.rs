//! 记忆文件扫描。
//!
//! 对齐 ava-harness::memory 设计蓝本：递归读目录，过滤 `.md` 且非
//! `MEMORY.md`，读前 `SCAN_HEAD_LINES` 行 frontmatter，按 mtime 新→旧排序，上限 200。

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::UNIX_EPOCH;

use crate::memory::frontmatter::parse_frontmatter;
use crate::memory::index::ENTRYPOINT_NAME;
use crate::memory::types::MemoryHeader;
use crate::memory::Result;

/// 扫描结果上限。
pub const MAX_SCAN_RESULTS: usize = 200;
/// 扫描读取的 frontmatter 行数。
pub const SCAN_HEAD_LINES: usize = 30;

/// 扫描记忆目录，返回记忆头列表（按 mtime 新→旧，上限 `MAX_SCAN_RESULTS`）。
pub fn scan_memory_files(root: &Path) -> Result<Vec<MemoryHeader>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut headers = Vec::new();
    collect(root, &mut headers)?;
    headers.sort_by_key(|h| std::cmp::Reverse(h.mtime_ms));
    headers.truncate(MAX_SCAN_RESULTS);
    Ok(headers)
}

fn collect(dir: &Path, out: &mut Vec<MemoryHeader>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out)?;
            continue;
        }
        let filename = match path.file_name().and_then(|s| s.to_str()) {
            Some(f) => f.to_string(),
            None => continue,
        };
        if !filename.ends_with(".md") || filename == ENTRYPOINT_NAME {
            continue;
        }
        let mtime = mtime_ms(&path);
        let header = read_header(&path, &filename, mtime);
        out.push(header);
    }
    Ok(())
}

fn mtime_ms(path: &Path) -> u64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 读前 `SCAN_HEAD_LINES` 行解析 frontmatter。
fn read_header(path: &Path, filename: &str, mtime_ms: u64) -> MemoryHeader {
    let head = read_head(path);
    let parsed = parse_frontmatter(&head);
    MemoryHeader {
        filename: filename.to_string(),
        file_path: path.to_path_buf(),
        mtime_ms,
        description: parsed.fm.description,
        memory_type: parsed.fm.memory_type,
    }
}

fn read_head(path: &Path) -> String {
    let f = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return String::new(),
    };
    let reader = BufReader::new(f);
    let mut out = String::new();
    for (i, line) in reader.lines().map_while(std::result::Result::ok).enumerate() {
        if i >= SCAN_HEAD_LINES {
            break;
        }
        out.push_str(&line);
        out.push('\n');
    }
    out
}

/// 格式化为文本清单（供子查询/预注入）。
pub fn format_memory_manifest(headers: &[MemoryHeader]) -> String {
    if headers.is_empty() {
        return "(no memories)".to_string();
    }
    headers
        .iter()
        .map(MemoryHeader::manifest_line)
        .collect::<Vec<_>>()
        .join("\n")
}

/// 记忆目录快照（仅主题文件，不含 `MEMORY.md`/隐藏文件）：文件名 → 内容哈希。
///
/// 供编排层在 fork-agent 运行前后各取一次，用 [`diff_files`] 统计落盘差异。
pub fn snapshot_files(root: &Path) -> Result<std::collections::BTreeMap<String, u64>> {
    use std::hash::{Hash, Hasher};
    let mut out = std::collections::BTreeMap::new();
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e.into()),
    };
    for entry in entries.flatten() {
        let filename = entry.file_name().to_string_lossy().to_string();
        if !filename.ends_with(".md") || filename.starts_with('.') || filename == ENTRYPOINT_NAME {
            continue;
        }
        let content = match fs::read_to_string(entry.path()) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        content.hash(&mut hasher);
        out.insert(filename, hasher.finish());
    }
    Ok(out)
}

/// 对比快照 → (新增或变更的文件数, 删除的文件数)。
pub fn diff_files(
    before: &std::collections::BTreeMap<String, u64>,
    after: &std::collections::BTreeMap<String, u64>,
) -> (usize, usize) {
    let changed = after
        .iter()
        .filter(|(name, hash)| before.get(*name).is_none_or(|b| b != *hash))
        .count();
    let deleted =
        before.len().saturating_sub(before.iter().filter(|(name, _)| after.contains_key(*name)).count());
    (changed, deleted)
}

#[cfg(test)]
#[path = "scan_tests.rs"]
mod tests;
