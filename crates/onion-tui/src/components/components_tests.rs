//! components.rs 单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use crate::components::{render_assistant_block, render_header, render_tool_card, render_user_message};
use crate::renderer::Line;

#[test]
fn header_is_three_lines() {
    let lines = render_header("s-1", "Qwen3.8-27B");
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0].text, "onion session");
    assert_eq!(lines[1].text, "  id: s-1");
    assert_eq!(lines[2].text, "  model: Qwen3.8-27B");
}

#[test]
fn user_message_single_line() {
    let lines = render_user_message("hello world");
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].text, "▸ user: hello world");
}

#[test]
fn assistant_block_with_usage() {
    let usage = onion::types::Usage {
        input_tokens: 100,
        output_tokens: 50,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
    };
    let lines = render_assistant_block("answer text", "thinking...", Some(&usage));
    // thinking + text + tokens = 3 行。
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0].text, "thinking...");
    assert_eq!(lines[1].text, "answer text");
    assert_eq!(lines[2].text, "  tokens: in 100 out 50");
}

#[test]
fn tool_card_error_flagged() {
    let lines = render_tool_card("read", "path=foo.txt", Some("output"), true);
    // 展开态：标题行 + 缩进输出 + error 标记。
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0].text, "▸ read(path=foo.txt)");
    assert_eq!(lines[1].text, "   output");
    assert_eq!(lines[2].text, "error");
}

#[test]
fn tool_card_collapsed_shows_done_stats() {
    // 折叠态：标题行 + `└ Done (ctrl+o to expand)` 统计行。
    let lines = render_tool_card("read", "path=foo.txt", None, false);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].text, "▸ read(path=foo.txt)");
    assert_eq!(lines[1].text, "└ Done (ctrl+o to expand)");
}

#[test]
fn lines_have_no_ansi_from_components() {
    // displayText 净化边界：组件层只产文本，不产 ANSI（样式经 Line.style 单独携带）。
    let lines: Vec<Line> = render_header("s", "m")
        .into_iter()
        .chain(render_user_message("x"))
        .collect();
    for line in &lines {
        assert!(!line.text.contains('\x1b'), "组件文本不应含 ANSI: {:?}", line.text);
    }
}
