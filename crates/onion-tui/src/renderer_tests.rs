//! renderer.rs 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::{FrameDiff, diff_rows, format_line_text, Line, Style};

#[test]
fn color_true_emits_sgr_prefix() {
    let line = Line::styled("hello", Style::new().dim());
    assert_eq!(format_line_text(&line, true), "\x1b[0m\x1b[2mhello");
}

#[test]
fn color_false_strips_all_sgr() {
    let line = Line::styled(
        "hello",
        Style::new().dim().reverse().color(crossterm::style::Color::Red),
    );
    assert_eq!(format_line_text(&line, false), "hello");
}

#[test]
fn format_line_sanitizes_embedded_ansi() {
    // 模型输出携带 ANSI 序列 → displayText 转可见转义（只有 TUI 产生 ANSI 序列）。
    let line = Line::plain("a\u{1b}[31mb");
    assert_eq!(format_line_text(&line, true), "a\\x1b[31mb");
}

#[test]
fn plain_line_with_color_true_has_no_sgr() {
    let line = Line::plain("hello");
    assert_eq!(format_line_text(&line, true), "hello");
}

#[test]
fn diff_of_identical_frames_rewrites_nothing() {
    let prev = vec!["a".to_string(), "b".to_string()];
    assert_eq!(diff_rows(&prev, &prev), super::FrameDiff { rewrite_from: 2, clear_tail_from: None });
}

#[test]
fn diff_of_streaming_append_rewrites_tail_only() {
    // 流式追加：前缀不变 → 只从新增行起重写。
    let prev = vec!["header".to_string(), "line1".to_string()];
    let next = vec!["header".to_string(), "line1".to_string(), "line2".to_string()];
    assert_eq!(diff_rows(&prev, &next), super::FrameDiff { rewrite_from: 2, clear_tail_from: None });
}

#[test]
fn diff_of_delta_growth_rewrites_last_row() {
    // 流式 delta：最后一行内容增长 → 从该行起重写（打字机推进）。
    let prev = ["head", "hello"].map(String::from).to_vec();
    let next = ["head", "hello wor"].map(String::from).to_vec();
    assert_eq!(diff_rows(&prev, &next), FrameDiff { rewrite_from: 1, clear_tail_from: None });
}

#[test]
fn diff_of_shrinking_frame_clears_tail() {
    // overlay 关闭：行数变少 → 清到屏尾。
    let prev = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    let next = vec!["a".to_string()];
    assert_eq!(diff_rows(&prev, &next), super::FrameDiff { rewrite_from: 1, clear_tail_from: Some(1) });
}

#[test]
fn diff_of_disjoint_frames_rewrites_all() {
    // 窗口滑动：无公共前缀 → 全窗重写（无清屏，不闪）。
    let prev = vec!["x".to_string()];
    let next = vec!["y".to_string(), "z".to_string()];
    assert_eq!(diff_rows(&prev, &next), super::FrameDiff { rewrite_from: 0, clear_tail_from: None });
}
