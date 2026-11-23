//! `lsp` 工具纯格式化与坐标转换。
//!
//! one-based↔zero-based UTF-16 光标转换、按文件分组的 location 渲染、完整结果截断。
//! 无 I/O——UI 可在实时流与重放上调用 presenter，故只依赖工具参数。

use crate::lsp::seam::{LspHover, LspLocation};

/// 渲染 location 前的条数上限。
pub const DEFAULT_MAX_LOCATIONS: usize = 100;
/// 完整渲染结果字符上限（含截断元数据）。
pub const DEFAULT_MAX_RESULT_CHARS: usize = 16_000;

/// 渲染 locations 结果，按文件分组，把每个 zero-based location 转回 one-based
/// `path:line:character` 条目。`file:` URI 在 workspace 内 → workspace 相对路径；在外 →
/// URI 派生绝对路径；非 `file:` URI 原样保留。应用 `max_locations` 并在按条数截断时
/// 追加省略标记，再应用完整结果上限。
pub fn format_locations(
    locations: &[LspLocation],
    workspace_uri: &str,
    max_locations: usize,
    max_result_chars: usize,
) -> String {
    if locations.is_empty() {
        return bound_result("No results.", max_result_chars, "locations");
    }
    let shown = &locations[..locations.len().min(max_locations)];
    let omitted = locations.len() - shown.len();
    let mut lines: Vec<String> = Vec::new();
    for location in shown {
        let path = render_uri(&location.uri, workspace_uri);
        let line = location.range.start.line + 1;
        let character = location.range.start.character + 1;
        lines.push(format!("{path}:{line}:{character}"));
    }
    if omitted > 0 {
        let noun = if omitted == 1 { "location" } else { "locations" };
        lines.push(format!("… {omitted} more {noun} omitted (limit {max_locations})."));
    }
    bound_result(&lines.join("\n"), max_result_chars, "locations")
}

/// 渲染 hover 结果，最后应用 `max_result_chars` 并让标记保持在限内。
pub fn format_hover(hover: &Option<LspHover>, max_result_chars: usize) -> String {
    let text = match hover {
        None => "No hover information.",
        Some(hover) => hover.contents.as_str(),
    };
    bound_result(text, max_result_chars, "hover")
}

/// 约束完整渲染结果（含截断提示本身）。
fn bound_result(text: &str, max_chars: usize, label: &str) -> String {
    if text.len() <= max_chars {
        return text.to_string();
    }
    let notice = format!("\n… {label} truncated (limit {max_chars} characters).");
    if notice.len() >= max_chars {
        return notice[..max_chars].to_string();
    }
    let cut = max_chars - notice.len();
    format!("{}{}", &text[..cut], notice)
}

/// 解析 location URI（不套用宿主机路径规则）。合法 `file:` URI 在规范 workspace URI
/// 之下 → workspace 相对；否则 → URI 派生绝对路径；畸形与非 `file:` URI 原样保留。
pub fn render_uri(uri: &str, workspace_uri: &str) -> String {
    if !uri.starts_with("file:") {
        return uri.to_string();
    }
    // 直接做轻量 file: URI →
    // 路径解码与相对化（覆盖 file:/// 与 file://host/ 形态）。
    let Some(target) = decode_file_uri(uri) else {
        return uri.to_string();
    };
    let Some(workspace) = decode_file_uri(workspace_uri) else {
        return uri.to_string();
    };
    // Windows drive 判定：workspace URI 含 host 或以 `/X:` 开头视为 Windows 世界。
    let workspace_windows = is_windows_path(&workspace);
    let target_windows = workspace_windows && is_windows_path(&target);
    if workspace_windows != target_windows {
        return target;
    }
    let rel = relativize(&workspace, &target);
    if rel.is_empty() {
        return ".".to_string();
    }
    if is_escape(&rel) {
        return target;
    }
    rel
}

/// 是否形如 Windows 盘符路径（`C:\...` 或 `/c:/...` drivePath）。
fn is_windows_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    // `X:\...` 或 `X:/...`
    if bytes.len() >= 3 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        return true;
    }
    // `/c:/...`（URL drivePath 形态）
    if bytes.len() >= 4 && bytes[0] == b'/' && bytes[2] == b':' && bytes[1].is_ascii_alphabetic() {
        return true;
    }
    false
}

/// 把 `file:` URI 解码为路径字符串；畸形返回 None。
fn decode_file_uri(uri: &str) -> Option<String> {
    // 形态：file:///path 或 file://host/path 或 file:/path。
    let rest = uri.strip_prefix("file:")?;
    let rest = rest.strip_prefix("//").unwrap_or(rest);
    // 丢弃 authority（host），仅保留路径段。
    let path_part = match rest.find('/') {
        Some(i) => &rest[i..],
        None => "/",
    };
    Some(percent_decode(path_part))
}

/// 简单 percent-decode（覆盖 %20 等）；畸形转义原样保留。
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = hex_val(bytes[i + 1]);
            let lo = hex_val(bytes[i + 2]);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push((hi << 4) | lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// 计算 `target` 相对 `base` 的路径；用 `/` 归一化比较。
fn relativize(base: &str, target: &str) -> String {
    let base_parts: Vec<&str> = base.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
    let target_parts: Vec<&str> = target.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
    let mut common = 0;
    while common < base_parts.len()
        && common < target_parts.len()
        && base_parts[common] == target_parts[common]
    {
        common += 1;
    }
    let mut out: Vec<String> = Vec::new();
    for _ in common..base_parts.len() {
        out.push("..".to_string());
    }
    for part in &target_parts[common..] {
        out.push((*part).to_string());
    }
    out.join("/")
}

/// 是否越出 base（`..` 开头或绝对路径）。
fn is_escape(rel: &str) -> bool {
    rel == ".."
        || rel.starts_with("../")
        || rel.starts_with("..\\")
        || rel.starts_with('/')
        || rel.starts_with('\\')
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
