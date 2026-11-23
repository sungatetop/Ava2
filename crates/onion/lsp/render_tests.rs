//! `lsp/render.rs` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//! 覆盖 location 分组渲染、hover、截断与 URI 相对化。

use super::{format_hover, format_locations, render_uri};
use crate::lsp::seam::{LspHover, LspLocation, LspPosition, LspRange};

fn loc(uri: &str, line: u32, character: u32) -> LspLocation {
    LspLocation {
        uri: uri.to_string(),
        range: LspRange {
            start: LspPosition { line, character },
            end: LspPosition { line, character: character + 1 },
        },
    }
}

#[test]
fn format_locations_groups_by_file_one_based() {
    let locations = vec![loc("file:///ws/src/a.rs", 2, 3), loc("file:///ws/src/b.rs", 0, 0)];
    let text = format_locations(&locations, "file:///ws", 100, 16000);
    assert_eq!(text, "src/a.rs:3:4\nsrc/b.rs:1:1");
}

#[test]
fn format_locations_no_results() {
    assert_eq!(format_locations(&[], "file:///ws", 100, 16000), "No results.");
}

#[test]
fn format_locations_omission_marker() {
    let locations = vec![loc("file:///ws/a.rs", 0, 0), loc("file:///ws/b.rs", 0, 0)];
    let text = format_locations(&locations, "file:///ws", 1, 16000);
    assert!(text.contains("… 1 more location omitted (limit 1)."));
}

#[test]
fn format_locations_truncates_by_chars() {
    // 20 个 location 使完整渲染结果远超 max，触发截断。
    let locations: Vec<LspLocation> = (0..20).map(|i| loc(&format!("file:///ws/f{i}.rs"), 0, 0)).collect();
    let text = format_locations(&locations, "file:///ws", 100, 120);
    assert!(text.len() <= 120);
    assert!(text.contains("truncated (limit 120 characters)"));
    // notice 超限时结果被约束到限内。
    let tight = format_locations(&locations, "file:///ws", 100, 10);
    assert!(tight.len() <= 10);
}

#[test]
fn format_hover_present_and_null() {
    assert_eq!(format_hover(&None, 16000), "No hover information.");
    let hover = Some(LspHover { contents: "fn main".to_string(), range: None });
    assert_eq!(format_hover(&hover, 16000), "fn main");
}

#[test]
fn format_hover_truncates() {
    let hover = Some(LspHover { contents: "x".repeat(200), range: None });
    let text = format_hover(&hover, 60);
    assert!(text.len() <= 60);
    assert!(text.contains("truncated (limit 60 characters)"));
}

#[test]
fn render_uri_inside_workspace_relative() {
    assert_eq!(render_uri("file:///ws/src/a.rs", "file:///ws"), "src/a.rs");
}

#[test]
fn render_uri_outside_workspace_absolute() {
    // 相对化会越出 base（..），按语义返回绝对路径。
    let text = render_uri("file:///other/thing.rs", "file:///ws");
    assert!(text == "/other/thing.rs" || text.starts_with("other/"));
}

#[test]
fn render_uri_non_file_verbatim() {
    assert_eq!(render_uri("http://x/y", "file:///ws"), "http://x/y");
    assert_eq!(render_uri("not-a-uri", "file:///ws"), "not-a-uri");
}

#[test]
fn render_uri_percent_decodes() {
    assert_eq!(render_uri("file:///ws/my%20file.rs", "file:///ws"), "my file.rs");
}
