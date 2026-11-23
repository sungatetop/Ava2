//! 统一 reminder 通道：所有 model-visible reminder 走这里。
//!
//! framing 由 producer 拥有：Session 层不加 wrapper，producer 自己
//! 用 `system_reminder` 包裹 body；`escape_reminder_body` 防注入逃逸。

/// 统一 reminder 帧：所有 model-visible reminder 走这里。
pub fn system_reminder(body: &str) -> String {
    format!("<system-reminder>\n{body}\n</system-reminder>")
}

/// 防注入逃逸：正文中的 `</system-reminder>` 转义。
pub fn escape_reminder_body(body: &str) -> String {
    body.replace("</system-reminder>", "<\\/system-reminder>")
}

#[cfg(test)]
#[path = "reminders_tests.rs"]
mod tests;
