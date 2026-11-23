//! `deliverables` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use serde_json::json;

use super::*;

/// 构造一条 `Presented` 事件（files 直接给 JSON 条目）。
fn presented(turn: u64, call_id: &str, files: Vec<serde_json::Value>) -> SessionEvent {
    SessionEvent::Presented {
        turn,
        call_id: call_id.to_string(),
        files,
    }
}

#[test]
fn project_keeps_first_order_and_last_declaration() {
    let events = vec![
        presented(1, "c1", vec![json!({ "path": "report.md", "description": "报告" })]),
        presented(2, "c2", vec![json!({ "path": "deck.pptx" })]),
        presented(3, "c3", vec![json!({ "path": "report.md", "description": "最终版" })]),
    ];
    assert_eq!(
        project(&events),
        vec![
            DeliverableInfo {
                turn: 3,
                call_id: "c3".to_string(),
                path: "report.md".to_string(),
                description: Some("最终版".to_string()),
            },
            DeliverableInfo {
                turn: 2,
                call_id: "c2".to_string(),
                path: "deck.pptx".to_string(),
                description: None,
            },
        ]
    );
}

#[test]
fn project_skips_malformed_entries() {
    let events = vec![
        presented(
            1,
            "c1",
            vec![json!({ "description": "缺 path" }), json!({ "path": "   " })],
        ),
        SessionEvent::TodoWrite { todos: Vec::new() },
        presented(2, "c2", vec![json!({ "path": "ok.md", "description": "" })]),
    ];
    assert_eq!(
        project(&events),
        vec![DeliverableInfo {
            turn: 2,
            call_id: "c2".to_string(),
            path: "ok.md".to_string(),
            description: None,
        }]
    );
}
