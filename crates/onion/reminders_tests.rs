//! `reminders` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//! 覆盖：reminder 帧格式 + 逃逸。

use super::*;

#[test]
fn system_reminder_wraps_body() {
    let rendered = system_reminder("hello");
    assert_eq!(rendered, "<system-reminder>\nhello\n</system-reminder>");
}

#[test]
fn escape_reminder_body_neutralizes_closing_tag() {
    let escaped = escape_reminder_body("</system-reminder>");
    assert_eq!(escaped, "<\\/system-reminder>");
}

#[test]
fn escape_reminder_body_leaves_normal_text() {
    assert_eq!(escape_reminder_body("normal"), "normal");
    assert_eq!(escape_reminder_body(""), "");
}
