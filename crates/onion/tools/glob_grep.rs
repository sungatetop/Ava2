//! 文件发现工具：glob / grep。
//!
//! - `glob(pattern, path?)`：按 glob 找**文件**（不含目录），含隐藏/忽略文件，
//!   按修改时间升序；返回最多 `GLOB_MAX_RESULTS` 条，超限 spill 到
//!   `<root>/logs/jobs/` 并提示（超限保存完整列表到 artifact）。
//! - `grep(pattern, path?, include?)`：按正则搜文件内容，按文件分组
//!   `Line N: <preview>`；保留最多 `GREP_MAX_MATCHES` 条，每行预览截断
//!   `GREP_MAX_LINE_BYTES` 字节（标记 `(line truncated)`），超限 spill。
//!
//! 核心语义：两者都是**无条件发现工具**（`ReadOnly`），不触发
//! read-before-write 观测（发现工具走 `ctx.subprocess` 消费者而非
//! `ctx.fs`）；不产生后台任务（普通前台调用）。onion 教学实现不依赖宿主 `rg`，
//! 用纯 Rust 递归 + `regex` 内联实现，无 shell 层、无外部二进制。
//!
//! 返回路径相对搜索根展示（`/` 分隔，跨平台统一），可被 `read` 继续读取。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use regex::Regex;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::ToolPort;
use crate::approval::ToolRisk;
use crate::sandbox::Sandbox;
use crate::types::{ContentBlock, ToolCall, ToolDefinition};
use crate::waterfall::BoxFuture;
/// `glob` 单次调用内联展示的最大路径数（缺省 100）。
const GLOB_MAX_RESULTS: usize = 100;
/// `grep` 单次调用内联保留的最大匹配数（缺省 250）。
const GREP_MAX_MATCHES: usize = 250;
/// `grep` 单行预览的最大字节数（缺省 2000）。
const GREP_MAX_LINE_BYTES: usize = 2000;

/// 把 glob 模式转换为正则表达式字符串。
///
/// 对齐 ripgrep glob 语义：
/// - 无 `/` 的模式匹配任意深度的 basename（`*` 匹配整个树，前缀 `**/`）；
/// - `**` 匹配任意深度路径段；
/// - `*` 匹配单个路径段内任意字符（不含 `/`）；
/// - `?` 匹配单个字符（不含 `/`）；
/// - `[...]` 字符类（含 `[!...]` 取反）；
/// - `{a,b}` 大括号交替。
fn glob_to_regex(pattern: &str) -> Result<String, String> {
    let prefix = if pattern.contains('/') { String::new() } else { "**/".to_string() };
    let full = format!("{prefix}{pattern}");
    let mut re = String::from("^");
    let chars: Vec<char> = full.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '*' => {
                if i + 1 < chars.len() && chars[i + 1] == '*' {
                    // 吞掉连续 `**`。
                    while i < chars.len() && chars[i] == '*' {
                        i += 1;
                    }
                    // 紧邻的 `/` 一并吞掉（`**/` 表示跨任意深度路径段）。
                    if i < chars.len() && chars[i] == '/' {
                        i += 1;
                    }
                    re.push_str("(?:.*/)?");
                    continue;
                }
                // 单个 `*`：匹配路径段内任意字符（不含 `/`）。
                re.push_str("[^/]*");
            }
            '?' => re.push_str("[^/]"),
            '[' => {
                let mut j = i + 1;
                let mut cls = String::from("[");
                if j < chars.len() && (chars[j] == '!' || chars[j] == '^') {
                    cls.push('^');
                    j += 1;
                }
                let mut closed = false;
                while j < chars.len() {
                    if chars[j] == ']' && j > i + 1 {
                        closed = true;
                        break;
                    }
                    cls.push(chars[j]);
                    j += 1;
                }
                if closed {
                    cls.push(']');
                    re.push_str(&cls);
                    i = j;
                } else {
                    re.push_str("\\[");
                }
            }
            '{' => {
                if let Some(offset) = full[i + 1..].find('}') {
                    let end = i + 1 + offset;
                    let inner = &full[i + 1..end];
                    let alts: Vec<&str> = inner.split(',').collect();
                    if alts.len() > 1 {
                        let joined = alts.iter().map(|a| escape_segment(a)).collect::<Vec<_>>().join("|");
                        re.push_str(&format!("(?:{joined})"));
                        i = end;
                        continue;
                    }
                }
                re.push_str("\\{");
            }
            '\\' => {
                if i + 1 < chars.len() {
                    re.push_str(&regex::escape(&chars[i + 1].to_string()));
                    i += 2;
                    continue;
                }
                re.push_str("\\\\");
            }
            '.' | '+' | '(' | ')' | '|' | '^' | '$' | ']' | '}' => {
                re.push('\\');
                re.push(c);
            }
            _ => re.push(c),
        }
        i += 1;
    }
    re.push('$');
    Ok(re)
}

/// 转义 glob 段中的正则元字符（用于大括号交替内部）。
fn escape_segment(seg: &str) -> String {
    let mut out = String::new();
    for c in seg.chars() {
        match c {
            '*' => out.push_str("[^/]*"),
            '?' => out.push_str("[^/]"),
            '.' | '+' | '(' | ')' | '|' | '^' | '$' | '[' | ']' | '{' | '}' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

/// 绝对路径 → 相对 root 的 `/` 分隔展示路径（跨平台统一）。
fn rel_display(root: &Path, abs: &Path) -> String {
    abs.strip_prefix(root)
        .unwrap_or(abs)
        .to_string_lossy()
        .replace('\\', "/")
}

/// glob：按路径模式找文件。
pub struct GlobTool {
    pub sandbox: Arc<Sandbox>,
    /// 输出 spill 根目录（`<root>/logs/jobs/`）。
    pub spill_dir: Option<PathBuf>,
}

impl GlobTool {
    /// 构造带默认 spill 目录的 glob 工具（`<root>/logs/jobs/`）。
    pub fn new(sandbox: Arc<Sandbox>) -> Self {
        let spill_dir = Some(crate::paths::logs_dir(&crate::paths::data_home()).join("jobs"));
        Self { sandbox, spill_dir }
    }
}

impl ToolPort for GlobTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "glob".to_string(),
            description: "Find files (not directories) whose paths match a glob pattern, including \
                hidden and ignored files. A pattern with no \"/\" matches basenames at any depth, \
                so \"*\" matches every file in the tree. Results come back in modification-time \
                order, up to 100; a larger result is truncated and the complete list is saved to a \
                file whose path is reported. Input: {\"pattern\": string, \"path\"?: string}."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "glob pattern to match file paths (e.g. \"**/*.rs\", \"*.md\")" },
                    "path": { "type": "string", "description": "optional directory to search under, relative to the sandbox root (default: the whole root)" }
                },
                "required": ["pattern"]
            }),
        }
    }

    fn risk(&self) -> ToolRisk {
        ToolRisk::ReadOnly
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let sandbox = self.sandbox.clone();
        let spill_dir = self.spill_dir.clone();
        Box::pin(async move {
            let pattern = call.input.get("pattern").and_then(Value::as_str).unwrap_or("");
            if pattern.is_empty() {
                return Err("pattern is required".to_string());
            }
            let re = Regex::new(&glob_to_regex(pattern)?)
                .map_err(|e| format!("SEARCH_INVALID_PATTERN: {e}"))?;

            // path 可选：搜索根（相对会话活动目录或任意绝对路径，读类不做 root
            // 校验——检索工具的 path 语义）；缺省整个活动目录。
            let search_root = match call.input.get("path").and_then(Value::as_str) {
                Some(dir) if !dir.is_empty() => {
                    let abs = sandbox.resolve_read(dir)?;
                    if tokio::fs::metadata(&abs).await.map(|m| m.is_dir()).unwrap_or(false) {
                        abs
                    } else {
                        return Err(format!("search path not a directory: {dir}"));
                    }
                }
                _ => sandbox.current().as_ref().clone(),
            };

            let files = sandbox.walk_under(&search_root).await?;
            let mut matched: Vec<String> = Vec::new();
            for abs in files {
                // 仅匹配搜索根之下的文件。
                let rel = match abs.strip_prefix(&search_root) {
                    Ok(rel) => rel.to_path_buf(),
                    Err(_) => continue,
                };
                let display = rel.to_string_lossy().replace('\\', "/");
                if re.is_match(&display) {
                    matched.push(display);
                }
            }
            if matched.is_empty() {
                return Ok(vec![ContentBlock::Text { text: "No files found".to_string() }]);
            }
            render_glob(matched, spill_dir.as_deref())
        })
    }
}

/// 渲染 glob 结果：截断到上限 + 超限 spill。
fn render_glob(matched: Vec<String>, spill_dir: Option<&Path>) -> Result<Vec<ContentBlock>, String> {
    if matched.len() <= GLOB_MAX_RESULTS {
        return Ok(vec![ContentBlock::Text { text: matched.join("\n") }]);
    }
    let shown = &matched[..GLOB_MAX_RESULTS];
    let omitted = matched.len() - GLOB_MAX_RESULTS;
    let mut text = shown.join("\n");
    // 超限 spill：完整列表落盘并提示路径。
    if let Some(dir) = spill_dir {
        if let Some(path) = spill_text(dir, "glob", &matched.join("\n")) {
            text.push_str(&format!(
                "\n\n[{omitted} more paths not shown; complete list saved to {}]",
                path.display()
            ));
            return Ok(vec![ContentBlock::Text { text }]);
        }
    }
    text.push_str(&format!("\n\n[{omitted} more paths not shown]"));
    Ok(vec![ContentBlock::Text { text }])
}

/// grep：按正则搜文件内容。
pub struct GrepTool {
    pub sandbox: Arc<Sandbox>,
    /// 输出 spill 根目录（`<root>/logs/jobs/`）。
    pub spill_dir: Option<PathBuf>,
}

impl GrepTool {
    /// 构造带默认 spill 目录的 grep 工具（`<root>/logs/jobs/`）。
    pub fn new(sandbox: Arc<Sandbox>) -> Self {
        let spill_dir = Some(crate::paths::logs_dir(&crate::paths::data_home()).join("jobs"));
        Self { sandbox, spill_dir }
    }
}

impl ToolPort for GrepTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "grep".to_string(),
            description: "Search file contents with a regular expression. Matches are grouped by \
                file as \"Line N: <preview>\", up to 250 matches; a larger result is truncated and \
                the complete list is saved to a file whose path is reported. Use read on a matched \
                file when you need surrounding context. Input: {\"pattern\": string, \"path\"?: \
                string, \"include\"?: string}."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "regular expression to match against file contents" },
                    "path": { "type": "string", "description": "optional file or directory to search (relative to sandbox root; default: the whole root)" },
                    "include": { "type": "string", "description": "optional single positive glob filter (e.g. \"*.rs\"); comma lists or negated values are rejected" }
                },
                "required": ["pattern"]
            }),
        }
    }

    fn risk(&self) -> ToolRisk {
        ToolRisk::ReadOnly
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let sandbox = self.sandbox.clone();
        let spill_dir = self.spill_dir.clone();
        Box::pin(async move {
            let pattern = call.input.get("pattern").and_then(Value::as_str).unwrap_or("");
            if pattern.is_empty() {
                return Err("pattern is required".to_string());
            }
            let re = Regex::new(pattern).map_err(|e| format!("SEARCH_INVALID_PATTERN: {e}"))?;

            // include：单个正向 glob 过滤；逗号列表或取反值拒绝。
            let include_re = match call.input.get("include").and_then(Value::as_str) {
                Some(include) if !include.is_empty() => {
                    if include.contains(',') || include.starts_with('!') {
                        return Err("include must be a single positive glob (no comma list or negation)".to_string());
                    }
                    Some(
                        Regex::new(&glob_to_regex(include)?)
                            .map_err(|e| format!("SEARCH_INVALID_PATTERN: {e}"))?,
                    )
                }
                _ => None,
            };

            // path 可选：文件或目录目标（读类不做 root 校验）；
            // 缺省整个 root。
            let target: Option<PathBuf> = match call.input.get("path").and_then(Value::as_str) {
                Some(p) if !p.is_empty() => {
                    let abs = sandbox.resolve_read(p)?;
                    if tokio::fs::metadata(&abs).await.is_ok() {
                        Some(abs)
                    } else {
                        return Err(format!("search target not found: {p}"));
                    }
                }
                _ => None,
            };

            // 收集候选文件：target 为目录 → 其下全部文件；target 为文件 → 单个；
            // 缺省 → root 下全部文件。
            let mut files: Vec<PathBuf> = Vec::new();
            match &target {
                Some(abs) if tokio::fs::metadata(abs).await.map(|m| m.is_dir()).unwrap_or(false) => {
                    files = sandbox.walk_under(abs).await?;
                }
                Some(abs) => files.push(abs.clone()),
                None => files = sandbox.walk().await?,
            }

            let mut matches: Vec<String> = Vec::new();
            let base = sandbox.current();
            for abs in files {
                // include 过滤（相对活动目录的展示路径）。
                let rel = abs
                    .strip_prefix(base.as_ref())
                    .unwrap_or(&abs)
                    .to_string_lossy()
                    .replace('\\', "/");
                if let Some(include_re) = &include_re {
                    if !include_re.is_match(&rel) {
                        continue;
                    }
                }
                // 读内容并逐行匹配（发现工具不登记 read-before-write 观测）。
                let bytes = match tokio::fs::read(&abs).await {
                    Ok(b) => b,
                    Err(_) => continue,
                };
                let text = String::from_utf8_lossy(&bytes);
                let mut file_matches: Vec<String> = Vec::new();
                for (idx, line) in text.lines().enumerate() {
                    if re.is_match(line) {
                        file_matches.push(format!("Line {}: {}", idx + 1, preview(line)));
                    }
                }
                if !file_matches.is_empty() {
                    let mut block = rel;
                    for m in file_matches {
                        block.push('\n');
                        block.push_str(&m);
                    }
                    matches.push(block);
                }
            }
            if matches.is_empty() {
                return Ok(vec![ContentBlock::Text { text: "No matches found".to_string() }]);
            }
            render_grep(matches, spill_dir.as_deref())
        })
    }
}

/// 单行预览：截断到 `GREP_MAX_LINE_BYTES` 字节，保留 UTF-8 边界并标记。
fn preview(line: &str) -> String {
    if line.len() <= GREP_MAX_LINE_BYTES {
        return line.to_string();
    }
    let mut end = GREP_MAX_LINE_BYTES;
    while end > 0 && !line.is_char_boundary(end) {
        end -= 1;
    }
    format!("{} (line truncated)", &line[..end])
}

/// 渲染 grep 结果：截断到上限 + 超限 spill。
fn render_grep(matches: Vec<String>, spill_dir: Option<&Path>) -> Result<Vec<ContentBlock>, String> {
    // 展平后按匹配计数截断——以「文件块」计数近似扁平匹配上限；
    // 逐行计数会跨文件拆分展示，块级截断更贴合「按文件分组」的呈现。
    if matches.len() <= GREP_MAX_MATCHES {
        return Ok(vec![ContentBlock::Text { text: matches.join("\n") }]);
    }
    let shown = &matches[..GREP_MAX_MATCHES];
    let omitted = matches.len() - GREP_MAX_MATCHES;
    let mut text = shown.join("\n");
    if let Some(dir) = spill_dir {
        if let Some(path) = spill_text(dir, "grep", &matches.join("\n")) {
            text.push_str(&format!(
                "\n\n[{omitted} more matches not shown; complete list saved to {}]",
                path.display()
            ));
            return Ok(vec![ContentBlock::Text { text }]);
        }
    }
    text.push_str(&format!("\n\n[{omitted} more matches not shown]"));
    Ok(vec![ContentBlock::Text { text }])
}

/// 把完整结果 spill 到 `<dir>/<kind>-<hash>.log`，返回文件路径。
fn spill_text(dir: &Path, kind: &str, body: &str) -> Option<PathBuf> {
    std::fs::create_dir_all(dir).ok()?;
    let name = crate::paths::encode_session_id(&format!("{kind}-{}", simple_hash(body)));
    let path = dir.join(format!("{name}.log"));
    std::fs::write(&path, body).ok()?;
    Some(path)
}

/// 简单内容哈希（spill 文件名用；非加密用途）。
fn simple_hash(s: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}
