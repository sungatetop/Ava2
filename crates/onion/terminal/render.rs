//! `terminal_*` 工具纯格式化与 UTF-8 安全截断。
//!
//! 无 I/O——UI 可在实时流与重放上调用 presenter，故只依赖工具参数。

use crate::terminal::seam::{TerminalSessionView, TerminalStatus, WaitReason};

/// 渲染结果字符上限。
pub const DEFAULT_MAX_RESULT_CHARS: usize = 32_768;

/// UTF-8 安全截断到 `max_chars` 字符，超限追加省略标记。不跨 UTF-8 码元边界。
pub fn truncate_for_render(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let cut: String = text.chars().take(max_chars).collect();
    format!("{cut}\n[output truncated]")
}

/// 渲染 spawn 产物（MOTD）。
pub fn format_open(id: &str, name: &str, type_: &str, motd: &str) -> String {
    let mut out = format!("started terminal session {id} ({name}) [type: {type_}]");
    if !motd.trim().is_empty() {
        out.push('\n');
        out.push_str(motd.trim_end());
    }
    out
}

/// 渲染 send 结果（前台）：viewport + wait/session 标记。
pub fn format_send(
    viewport: &str,
    wait_reason: WaitReason,
    status: &TerminalStatus,
    max_chars: usize,
) -> String {
    let viewport = truncate_for_render(viewport, max_chars);
    let wait = format!("[wait: {}]", wait_reason.as_str());
    let session = match status {
        TerminalStatus::Running => "[session: running]".to_string(),
        TerminalStatus::Exited { exit_code, signal } => {
            let sig = signal.map(|s| format!(" signal={}", s.as_str())).unwrap_or_default();
            format!("[session: exited code={exit_code}{sig}]")
        }
    };
    format!("{viewport}\n{wait}\n{session}")
}

/// 渲染 read 结果：文本 + 分页标记。
pub fn format_read(lines: &[String], offset: usize, total: usize, max_chars: usize) -> String {
    let text = truncate_for_render(&lines.join("\n"), max_chars);
    let begin = offset + 1;
    let end = offset + lines.len();
    format!("{text}\n[lines: {begin}-{end} of {total}]")
}

/// 渲染 signal 结果。
pub fn format_signal(sig: &str, pgid: &str) -> String {
    format!("delivered {sig} to foreground process group {pgid}")
}

/// 渲染 close 结果。
pub fn format_close(id: &str) -> String {
    format!("closed terminal session {id}")
}

/// 渲染 list 结果：每会话一行或空提示。
pub fn format_list(views: &[TerminalSessionView], max_chars: usize) -> String {
    if views.is_empty() {
        return "(no terminal sessions)".to_string();
    }
    let mut lines = Vec::new();
    for view in views {
        let status = match &view.status {
            TerminalStatus::Running => "running".to_string(),
            TerminalStatus::Exited { exit_code, .. } => format!("exited (code={exit_code})"),
        };
        lines.push(format!("{} ({}) [type: {}] [lines: {}] — {}", view.id.0, view.name, view.type_, view.lines, status));
    }
    truncate_for_render(&lines.join("\n"), max_chars)
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
