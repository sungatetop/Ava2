//! `fetch` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use crate::web::fetch::{FetchOutcome, format_fetch_output, html_to_text, parse_headers};

fn outcome(url: &str, status: u16, content: &str, truncated: bool) -> FetchOutcome {
    FetchOutcome {
        url: url.to_string(),
        status_code: status,
        kind: "text",
        content: content.to_string(),
        truncated,
    }
}

#[test]
fn parse_headers_empty_when_absent() {
    let headers = parse_headers(&serde_json::json!({"url": "https://example.com"})).unwrap();
    assert!(headers.is_empty());
}

#[test]
fn parse_headers_accepts_object_of_strings() {
    let headers = parse_headers(&serde_json::json!({
        "url": "https://example.com",
        "headers": { "authorization": "Bearer tok", "x-custom": "v" }
    }))
    .unwrap();
    assert_eq!(headers.len(), 2);
    assert_eq!(headers.get("authorization").unwrap(), "Bearer tok");
    assert_eq!(headers.get("x-custom").unwrap(), "v");
}

#[test]
fn parse_headers_rejects_non_object() {
    let error = parse_headers(&serde_json::json!({"headers": "x"})).unwrap_err();
    assert!(error.contains("headers must be an object"));
}

#[test]
fn parse_headers_rejects_non_string_value() {
    let error = parse_headers(&serde_json::json!({"headers": {"x": 1}})).unwrap_err();
    assert!(error.contains("header `x` must be a string"));
}

#[test]
fn parse_headers_rejects_invalid_header_name() {
    let error = parse_headers(&serde_json::json!({"headers": {"bad name\u{0}": "v"}})).unwrap_err();
    assert!(error.contains("invalid header name"));
}

#[test]
fn formats_header_and_notice() {
    let text = format_fetch_output(&outcome("https://example.com", 200, "hello world", false), 200_000);
    assert!(text.starts_with("Fetched https://example.com (HTTP 200)"));
    assert!(text.contains("External web content follows."));
    assert!(text.contains("hello world"));
}

#[test]
fn appends_truncation_footer_when_cut() {
    let body = "a".repeat(100);
    let text = format_fetch_output(&outcome("https://example.com", 200, &body, true), 200_000);
    assert!(text.contains("(Content truncated."));
}

#[test]
fn caps_output_to_max_chars() {
    let body = "x".repeat(1000);
    let text = format_fetch_output(&outcome("https://example.com", 200, &body, false), 200);
    assert!(text.len() <= 200);
    assert!(text.ends_with("(Content truncated. Fetch a more specific URL or section for the full text.)"));
}

#[test]
fn strips_html_tags_and_scripts() {
    let html = "<html><head><style>body{}</style></head><body><h1>Title</h1><script>evil()</script><p>Body &amp; more</p></body></html>";
    let text = html_to_text(html);
    assert!(text.contains("Title"));
    assert!(text.contains("Body & more"));
    assert!(!text.contains("<"));
    assert!(!text.contains("evil"));
}
