//! banner.rs 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::banner_lines;
use crate::renderer::Style;
use crossterm::style::Color;

#[test]
fn banner_follows_three_section_layout() {
    let lines = banner_lines("C:\\workspace", "Qwen3.8-27B");
    // 虚线 + 6 行大字 + 虚线 + 3 行信息区 + 1 空行。
    assert_eq!(lines.len(), 12);
    // 首尾虚线：62 个 '-'，dim。
    for idx in [0, 7] {
        assert_eq!(lines[idx].text, "-".repeat(62));
        assert!(lines[idx].style.dim, "虚线应为 dim");
    }
    // 大字 6 行，绿色：前 5 行含实心块，底行为横线块（ANSI Shadow 底边）。
    for (offset, anchor) in ["█", "█", "█", "█", "█", "═"].iter().enumerate() {
        let line = &lines[1 + offset];
        assert!(line.text.contains(anchor), "大字行应含 {anchor} offset={offset} text={:?}", line.text);
        assert_eq!(line.style.color, Some(Color::DarkGreen));
    }
}

#[test]
fn info_section_carries_cwd_and_model() {
    let lines = banner_lines("C:\\workspace", "Qwen3.8-27B");
    let texts: Vec<&str> = lines.iter().map(|l| l.text.as_str()).collect();
    // 信息区三行均为 `>> ` 前缀。
    for text in &texts[8..11] {
        assert!(text.starts_with(">> "), "信息区应带 >> 前缀：{text}");
    }
    // 两列布局：身份行与 model 行、cwd 行与恢复提示行同排。
    assert!(texts[8].starts_with(">> ONION TUI"));
    assert!(texts[8].contains(">> Model: Qwen3.8-27B"));
    assert!(texts[9].starts_with(">> cwd: C:\\workspace"));
    assert!(texts[9].contains(">> /resume <id> 恢复会话"));
    // 快捷键通栏行 + 尾部空行。
    assert!(texts[10].contains("Ctrl+M 切换模型"));
    assert_eq!(lines[11].text, "");
}

#[test]
fn left_column_padding_aligns_two_columns() {
    let lines = banner_lines("C:\\w", "m");
    // 左列内容 pad 到 38 列后右列起始于固定列位（两条信息行同列对齐）。
    let pos8 = lines[8].text.find(">> Model").unwrap();
    let pos9 = lines[9].text.find(">> /resume").unwrap();
    assert_eq!(pos8, pos9);
    assert_eq!(pos8, 38);
}

#[test]
fn banner_lines_are_styled_not_plain_default() {
    // 大字行样式为绿色（非默认 plain），防止误改回 Line::plain。
    let lines = banner_lines("", "");
    assert_ne!(lines[1].style, Style::new());
}
