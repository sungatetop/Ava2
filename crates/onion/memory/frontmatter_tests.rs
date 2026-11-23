//! `frontmatter` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;

#[test]
fn extract_frontmatter_parses_yaml_block_and_body() {
    let content = "---\nname: user_role\ndescription: User role\ntype: user\n---\n\nBody text\n";
    let (yaml, body) = extract_frontmatter(content);
    assert_eq!(yaml, Some("name: user_role\ndescription: User role\ntype: user"));
    assert_eq!(body, "\nBody text\n");
}

#[test]
fn extract_frontmatter_without_frontmatter_returns_none() {
    let content = "Just a body\n";
    let (yaml, body) = extract_frontmatter(content);
    assert_eq!(yaml, None);
    assert_eq!(body, content);
}

#[test]
fn extract_frontmatter_strips_bom() {
    let content = "\u{feff}---\nname: x\n---\n\nbody\n";
    let (yaml, _) = extract_frontmatter(content);
    assert!(yaml.is_some());
}

#[test]
fn extract_frontmatter_unclosed_returns_none() {
    let content = "---\nname: x\n";
    let (yaml, _) = extract_frontmatter(content);
    assert_eq!(yaml, None);
}

#[test]
fn parse_frontmatter_reads_quoted_and_type_fields() {
    let content = "---\nname: \"x y\"\ndescription: 'a desc'\ntype: feedback\n---\n\nbody\n";
    let parsed = parse_frontmatter(content);
    assert_eq!(parsed.fm.name, "x y");
    assert_eq!(parsed.fm.description, "a desc");
    assert_eq!(parsed.fm.memory_type, Some(MemoryType::Feedback));
}

#[test]
fn parse_frontmatter_defaults_when_missing() {
    let parsed = parse_frontmatter("no fm");
    assert_eq!(parsed.fm.name, "");
    assert_eq!(parsed.fm.description, "");
    assert_eq!(parsed.fm.memory_type, None);
    assert_eq!(parsed.body, "no fm");
}

#[test]
fn parse_frontmatter_ignores_comments_and_unknown_keys() {
    let content = "---\n# comment\nname: n\nother: v\ntype: project\n---\n\nb\n";
    let parsed = parse_frontmatter(content);
    assert_eq!(parsed.fm.name, "n");
    assert_eq!(parsed.fm.memory_type, Some(MemoryType::Project));
}

#[test]
fn render_frontmatter_appends_trailing_newline() {
    let fm = MemoryFrontmatter {
        name: "n".into(),
        description: "d".into(),
        memory_type: Some(MemoryType::Reference),
    };
    let out = render_frontmatter(&fm, "body");
    assert!(out.ends_with("body\n"));
    assert!(out.contains("type: reference"));
}

#[test]
fn strip_html_comments_removes_block_comments() {
    let out = strip_html_comments("a<!-- hidden -->b");
    assert_eq!(out, "ab");
}

#[test]
fn strip_html_comments_keeps_code_fence_comments() {
    let out = strip_html_comments("```\n<!-- kept -->\n```");
    assert_eq!(out, "```\n<!-- kept -->\n```");
}

#[test]
fn strip_html_comments_keeps_unclosed_comment() {
    let out = strip_html_comments("a<!-- unclosed");
    assert_eq!(out, "a<!-- unclosed");
}
