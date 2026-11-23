//! terminal.rs 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::display_text;

#[test]
fn display_text_keeps_newline_and_tab() {
    assert_eq!(display_text("a\nb\tc"), "a\nb\tc");
}

#[test]
fn display_text_escapes_c0() {
    assert_eq!(display_text("a\u{1}b"), "a\\x01b");
    assert_eq!(display_text("a\u{7f}b"), "a\\x7fb");
}

#[test]
fn display_text_escapes_c1() {
    // U+009B（CSI）转可见转义。
    assert_eq!(display_text("a\u{9b}b"), "a\\x9bb");
}

#[test]
fn display_text_keeps_ansi_escape_plain_text() {
    // 字面 "\x1b" 文本（反斜杠 + x + 1b）不动。
    assert_eq!(display_text("\\x1b[31m"), "\\x1b[31m");
}
