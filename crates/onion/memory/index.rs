//! `MEMORY.md` 索引读写与截断。
//!
//! 对齐 ava-harness::memory 设计蓝本：索引是链接列表，非记忆本体；
//! 有 200 行 / 25KB 双上限；保存记忆是"写主题文件 + 更新索引"两步。

use std::fs;
use std::path::Path;

use crate::memory::{MemoryError, Result};

/// 索引文件名。
pub const ENTRYPOINT_NAME: &str = "MEMORY.md";
/// 索引行上限。
pub const MAX_ENTRYPOINT_LINES: usize = 200;
/// 索引字节上限。
pub const MAX_ENTRYPOINT_BYTES: usize = 25_000;
/// 单行索引字符上限。
pub const MAX_INDEX_LINE_CHARS: usize = 150;

/// 生成一行索引：`- [Title](file.md) — hook`。
pub fn format_index_line(title: &str, filename: &str, hook: &str) -> String {
    let title = truncate_chars(title, MAX_INDEX_LINE_CHARS);
    let hook = truncate_chars(hook, MAX_INDEX_LINE_CHARS);
    format!("- [{title}]({filename}) — {hook}")
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let clipped: String = s.chars().take(max).collect();
    format!("{clipped}…")
}

/// 截断索引内容：先按行、再按字节，并在末尾追加 WARNING。
pub fn truncate_entrypoint(content: &str) -> String {
    let mut warning = String::new();
    let mut out = content.to_string();
    // 按行截断（预留 2 行给末尾 warning，保证含 warning 的总行数不超上限）。
    let lines: Vec<&str> = out.lines().collect();
    if lines.len() > MAX_ENTRYPOINT_LINES {
        out = lines[..MAX_ENTRYPOINT_LINES - 2].join("\n");
        warning.push_str(&format!(
            "\n\n<warning>Memory index truncated: exceeds {MAX_ENTRYPOINT_LINES} lines.</warning>\n"
        ));
    }
    // 按字节截断（在最后一个换行处）。
    if out.len() > MAX_ENTRYPOINT_BYTES {
        let bytes = out.as_bytes();
        let mut cut = MAX_ENTRYPOINT_BYTES;
        while cut > 0 && bytes[cut - 1] != b'\n' {
            cut -= 1;
        }
        out = out[..cut].to_string();
        if !warning.contains("lines") {
            warning.push_str(&format!(
                "\n\n<warning>Memory index truncated: exceeds {MAX_ENTRYPOINT_BYTES} bytes.</warning>\n"
            ));
        }
    }
    out.push_str(&warning);
    out
}

/// 读取索引内容（已截断）。
pub fn read_entrypoint(root: &Path) -> Result<String> {
    let path = root.join(ENTRYPOINT_NAME);
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(e) => return Err(MemoryError::Io(e)),
    };
    Ok(truncate_entrypoint(&content))
}

/// 解析索引行 → `(title, filename, hook)`；非索引行返回 `None`。
fn parse_index_line(line: &str) -> Option<(String, String, String)> {
    let line = line.trim();
    let rest = line.strip_prefix("- [")?;
    let (title, after_title) = rest.split_once("](")?;
    let (filename, after_filename) = after_title.split_once(')')?;
    let hook = after_filename.trim_start_matches(" — ").trim();
    Some((title.to_string(), filename.to_string(), hook.to_string()))
}

/// 更新（新增或替换）索引中的一行。
///
/// 若 `filename` 已存在则替换该行，否则追加到末尾。
pub fn upsert_index_line(root: &Path, filename: &str, title: &str, hook: &str) -> Result<()> {
    let path = root.join(ENTRYPOINT_NAME);
    let existing = read_entrypoint(root)?;
    let new_line = format_index_line(title, filename, hook);
    let mut replaced = false;
    let mut lines: Vec<String> = Vec::new();
    for line in existing.lines() {
        if let Some((_, f, _)) = parse_index_line(line) {
            if f == filename {
                lines.push(new_line.clone());
                replaced = true;
                continue;
            }
        }
        lines.push(line.to_string());
    }
    if !replaced {
        lines.push(new_line);
    }
    fs::create_dir_all(root)?;
    let content = truncate_entrypoint(&lines.join("\n"));
    fs::write(path, content + "\n")?;
    Ok(())
}

/// 从索引中移除某记忆文件对应的行。
pub fn remove_index_line(root: &Path, filename: &str) -> Result<()> {
    let path = root.join(ENTRYPOINT_NAME);
    let existing = read_entrypoint(root)?;
    let mut lines: Vec<String> = Vec::new();
    for line in existing.lines() {
        if let Some((_, f, _)) = parse_index_line(line) {
            if f == filename {
                continue;
            }
        }
        lines.push(line.to_string());
    }
    fs::create_dir_all(root)?;
    let content = truncate_entrypoint(&lines.join("\n"));
    fs::write(path, content + "\n")?;
    Ok(())
}

#[cfg(test)]
#[path = "index_tests.rs"]
mod tests;
