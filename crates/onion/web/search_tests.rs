//! `search` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use serde_json::json;

use crate::tools::ToolPort;
use crate::types::ToolCall;
use crate::web::search::{Source, WebSearchTool, format_search_output, map_anthropic_response, merge_search_results, parse_search_args};
use tokio_util::sync::CancellationToken;

fn tool() -> WebSearchTool {
    WebSearchTool::new(String::new(), String::new(), 8, 4)
}

#[test]
fn parse_rejects_empty() {
    let call = ToolCall { id: "c1".into(), name: "web_search".into(), input: json!({ "queries": [] }) };
    assert!(parse_search_args(&call, 4).is_err());
}

#[test]
fn parse_rejects_missing() {
    let call = ToolCall { id: "c1".into(), name: "web_search".into(), input: json!({}) };
    assert!(parse_search_args(&call, 4).is_err());
}

#[test]
fn parse_rejects_blank() {
    let call = ToolCall { id: "c1".into(), name: "web_search".into(), input: json!({ "queries": ["  "] }) };
    assert!(parse_search_args(&call, 4).is_err());
}

#[test]
fn parse_dedupes_and_trims() {
    let call = ToolCall {
        id: "c1".into(),
        name: "web_search".into(),
        input: json!({ "queries": ["a", "b", " a "] }),
    };
    let q = parse_search_args(&call, 4).unwrap();
    assert_eq!(q, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn parse_rejects_over_max() {
    let call = ToolCall {
        id: "c1".into(),
        name: "web_search".into(),
        input: json!({ "queries": ["a", "b", "c", "d", "e"] }),
    };
    assert!(parse_search_args(&call, 4).is_err());
}

#[test]
fn maps_anthropic_response() {
    let payload = json!({
        "content": [
            {
                "type": "text",
                "text": "summary",
                "citations": [{ "url": "https://a.com", "cited_text": "snippet-a" }]
            },
            {
                "type": "web_search_tool_result",
                "content": [
                    { "type": "web_search_result", "url": "https://a.com", "title": "A", "page_age": "2 days ago" },
                    { "type": "web_search_result", "url": "https://b.com", "title": "B" }
                ]
            }
        ]
    });
    let outcome = map_anthropic_response(&payload, 8).unwrap();
    assert_eq!(outcome.sources.len(), 2);
    // snippet 从 citation 补齐。
    assert_eq!(outcome.sources[0].snippet.as_deref(), Some("snippet-a"));
    assert_eq!(outcome.sources[0].published_at.as_deref(), Some("2 days ago"));
}

#[test]
fn maps_no_result_blocks_as_error() {
    let payload = json!({ "content": [{ "type": "text", "text": "nothing" }] });
    assert!(map_anthropic_response(&payload, 8).is_err());
}

#[test]
fn merge_dedupes_round_robin() {
    let queries = vec!["q1".to_string(), "q2".to_string()];
    let results = vec![
        crate::web::search::SearchOutcome {
            content: Some("c1".into()),
            sources: vec![
                Source { url: "https://a.com".into(), title: None, snippet: None, published_at: None },
                Source { url: "https://c.com".into(), title: None, snippet: None, published_at: None },
            ],
            truncated: false,
        },
        crate::web::search::SearchOutcome {
            content: Some("c2".into()),
            sources: vec![
                Source { url: "https://a.com".into(), title: None, snippet: None, published_at: None },
                Source { url: "https://b.com".into(), title: None, snippet: None, published_at: None },
            ],
            truncated: false,
        },
    ];
    let merged = merge_search_results(&queries, &results, 8);
    let urls: Vec<&str> = merged.sources.iter().map(|s| s.url.as_str()).collect();
    // round-robin：a, b(rank0) → c, b(rank1 去重) → 最终 a, c, b。
    assert_eq!(urls, vec!["https://a.com", "https://c.com", "https://b.com"]);
    // 多 query content 带标题。
    let content = merged.content.unwrap();
    assert!(content.contains("### q1"));
    assert!(content.contains("### q2"));
}

#[test]
fn formats_sources_with_label() {
    let outcome = crate::web::search::SearchOutcome {
        content: None,
        sources: vec![Source {
            url: "https://example.com/path".into(),
            title: Some("Example".into()),
            snippet: Some("a snippet".into()),
            published_at: None,
        }],
        truncated: false,
    };
    let text = format_search_output(&outcome);
    assert!(text.contains("External web content follows."));
    assert!(text.contains("- [Example](https://example.com/path) — a snippet"));
    assert!(text.contains("Cite the relevant URLs above as markdown links in your answer."));
}

#[test]
fn no_sources_prints_no_results() {
    let outcome = crate::web::search::SearchOutcome { content: None, sources: vec![], truncated: false };
    let text = format_search_output(&outcome);
    assert!(text.contains("No results found."));
}

#[tokio::test]
async fn unconfigured_provider_fails_structurally() {
    let tool = tool();
    let call = ToolCall { id: "c1".into(), name: "web_search".into(), input: json!({ "queries": ["a"] }) };
    let result = tool.execute(call, CancellationToken::new()).await;
    let err = result.unwrap_err();
    assert!(err.contains("not configured"));
}
