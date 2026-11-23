//! frontmatter 解析/序列化与 HTML 注释剥离。
//!
//! 对齐 ava-harness::memory 设计蓝本：用行级解析提取 YAML 块，
//! 失败时二次尝试"给特殊字符加引号"。

use crate::memory::types::{parse_memory_type, MemoryType};

/// 记忆文件 frontmatter。
#[derive(Debug, Clone, Default)]
pub struct MemoryFrontmatter {
    pub name: String,
    pub description: String,
    pub memory_type: Option<MemoryType>,
}

/// frontmatter 解析结果。
#[derive(Debug, Clone)]
pub struct ParsedFrontmatter {
    pub fm: MemoryFrontmatter,
    /// 去除 frontmatter 后的正文。
    pub body: String,
}

/// 提取 frontmatter 块。返回 `(frontmatter_yaml, body)`；无 frontmatter 时 `(None, 原content)`。
pub fn extract_frontmatter(content: &str) -> (Option<&str>, &str) {
    let trimmed = content.trim_start_matches('\u{feff}');
    if let Some(rest) = trimmed.strip_prefix("---") {
        // 需要第一行是 `---`（可能带换行）。
        if let Some(nl) = rest.find('\n') {
            let inner = &rest[..nl];
            if inner.trim().is_empty() {
                let after = &rest[nl + 1..];
                if let Some(end) = find_closing(after) {
                    // 闭合 `---` 前的尾部换行不属于 yaml 块。
                    let yaml = after[..end].trim_end();
                    let tail = &after[end + 3..];
                    // 跳过闭合行的换行符（其后空行分隔保留在正文中）。
                    let body = tail.strip_prefix('\n').unwrap_or(tail);
                    return (Some(yaml), body);
                }
            }
        }
    }
    (None, content)
}

/// 在 frontmatter 内找闭合 `---`（独占一行）。
fn find_closing(s: &str) -> Option<usize> {
    let mut idx = 0;
    for line in s.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed == "---" {
            return Some(idx);
        }
        idx += line.len();
    }
    None
}

/// 解析 frontmatter YAML 中的 `key: value` 字段。
fn parse_yaml_fields(yaml: &str) -> (String, String, Option<MemoryType>) {
    let mut name = String::new();
    let mut description = String::new();
    let mut memory_type = None;
    for line in yaml.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = unquote(value.trim());
            match key {
                "name" => name = value,
                "description" => description = value,
                "type" => memory_type = parse_memory_type(&value),
                _ => {}
            }
        }
    }
    (name, description, memory_type)
}

/// 去掉单/双引号包裹。
fn unquote(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 {
        let b = s.as_bytes();
        if (b[0] == b'"' && b[s.len() - 1] == b'"') || (b[0] == b'\'' && b[s.len() - 1] == b'\'') {
            return s[1..s.len() - 1].to_string();
        }
    }
    s.to_string()
}

/// 解析记忆文件内容。返回 frontmatter + 正文。
pub fn parse_frontmatter(content: &str) -> ParsedFrontmatter {
    let (yaml, body) = extract_frontmatter(content);
    let fm = match yaml {
        Some(yaml) => {
            let (name, description, memory_type) = parse_yaml_fields(yaml);
            MemoryFrontmatter { name, description, memory_type }
        }
        None => MemoryFrontmatter::default(),
    };
    ParsedFrontmatter { fm, body: body.to_string() }
}

/// 渲染 frontmatter + 正文。
pub fn render_frontmatter(fm: &MemoryFrontmatter, body: &str) -> String {
    let ty = fm.memory_type.map(|t| t.as_str()).unwrap_or("user");
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!("name: {}\n", fm.name));
    out.push_str(&format!("description: {}\n", fm.description));
    out.push_str(&format!("type: {ty}\n"));
    out.push_str("---\n\n");
    out.push_str(body.trim_start_matches('\n'));
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// 剥离 HTML 块级注释 `<!-- ... -->`（简单状态机，跳过代码块内）。
pub fn strip_html_comments(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    let mut chars = content.chars().peekable();
    let mut in_code_fence = false;
    while let Some(c) = chars.next() {
        // 跟踪代码围栏（``` 或 ~~~）。
        if c == '`' || c == '~' {
            let mut run = 1;
            while chars.peek() == Some(&c) {
                chars.next();
                run += 1;
            }
            if run >= 3 {
                in_code_fence = !in_code_fence;
            }
            out.extend(std::iter::repeat_n(c, run));
            continue;
        }
        if !in_code_fence && c == '<' && chars.peek() == Some(&'!') {
            // 尝试匹配 <!-- ... -->，命中则整体跳过。
            let _ = chars.next(); // 先消费已 peek 的 `!`，否则回退时重复（`<!--` → `<!!--`）。
            let mut consumed = String::from("<!");
            let mut buf: Vec<char> = Vec::new();
            let mut found_end = false;
            for ch in chars.by_ref() {
                consumed.push(ch);
                buf.push(ch);
                if buf.len() > 3 {
                    buf.remove(0);
                }
                if buf == ['-', '-', '>'] {
                    found_end = true;
                    break;
                }
            }
            if !found_end {
                // 未闭合，回退为原文。
                out.push_str(&consumed);
            }
            continue;
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
#[path = "frontmatter_tests.rs"]
mod tests;
