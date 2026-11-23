//! `tools` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use serde_json::json;
use tokio_util::sync::CancellationToken;

use crate::ask_user::service::AskUserService;
use crate::ask_user::tools::AskUserQuestionTool;
use crate::ask_user::types::AskUserAnswer;
use crate::tools::ToolPort;
use crate::types::{ContentBlock, ToolCall};
use std::sync::Arc;
use tokio::sync::mpsc;

/// 从工具结果块中提取纯文本（接受 `&[ContentBlock]`）。
fn text(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

fn tool() -> (AskUserQuestionTool, mpsc::UnboundedReceiver<crate::ask_user::service::AskUserRequest>) {
    let service = Arc::new(AskUserService::new());
    let (tx, rx) = mpsc::unbounded_channel();
    service.set_request_channel(tx);
    (AskUserQuestionTool::new(service), rx)
}

#[tokio::test]
async fn missing_questions_rejected() {
    let (tool, _rx) = tool();
    let call = ToolCall { id: "c1".into(), name: "ask_user_question".into(), input: json!({}) };
    let result = tool.execute(call, CancellationToken::new()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn empty_questions_rejected() {
    let (tool, _rx) = tool();
    let call = ToolCall { id: "c1".into(), name: "ask_user_question".into(), input: json!({ "questions": [] }) };
    let result = tool.execute(call, CancellationToken::new()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn question_without_id_rejected() {
    let (tool, _rx) = tool();
    let call = ToolCall {
        id: "c1".into(),
        name: "ask_user_question".into(),
        input: json!({ "questions": [{ "question": "pick one" }] }),
    };
    let result = tool.execute(call, CancellationToken::new()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn question_without_text_rejected() {
    let (tool, _rx) = tool();
    let call = ToolCall {
        id: "c1".into(),
        name: "ask_user_question".into(),
        input: json!({ "questions": [{ "id": "q1" }] }),
    };
    let result = tool.execute(call, CancellationToken::new()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn blocking_ask_returns_answer() {
    let (tool, mut rx) = tool();
    let call = ToolCall {
        id: "c1".into(),
        name: "ask_user_question".into(),
        input: json!({
            "questions": [{
                "id": "q1",
                "question": "continue?",
                "options": [{ "label": "yes" }, { "label": "no" }]
            }]
        }),
    };
    // 并发执行工具（挂起等待答复），宿主侧回填。
    let handle = tokio::spawn(async move { tool.execute(call, CancellationToken::new()).await });
    let request = rx.recv().await.expect("request sent");
    assert_eq!(request.questions.len(), 1);
    assert_eq!(request.questions[0].id, "q1");
    assert_eq!(request.questions[0].options.as_ref().unwrap().len(), 2);
    request
        .reply
        .send(AskUserAnswer {
            answers: vec![crate::ask_user::types::AskUserAnswerItem {
                id: "q1".into(),
                selected: vec!["yes".into()],
                custom: None,
            }],
        })
        .expect("reply sent");
    let result = handle.await.expect("tool joined");
    let blocks = result.unwrap();
    let rendered = text(&blocks);
    assert!(rendered.contains("\"id\":\"q1\""));
    assert!(rendered.contains("\"selected\":[\"yes\"]"));
}

#[tokio::test]
async fn channel_closed_fails_closed() {
    let (tool, rx) = tool();
    drop(rx);
    let call = ToolCall {
        id: "c1".into(),
        name: "ask_user_question".into(),
        input: json!({ "questions": [{ "id": "q1", "question": "continue?" }] }),
    };
    let result = tool.execute(call, CancellationToken::new()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn timed_timeout_returns_pending() {
    let (tool, mut rx) = tool();
    let call = ToolCall {
        id: "c1".into(),
        name: "ask_user_question".into(),
        input: json!({
            "questions": [{ "id": "q1", "question": "continue?" }],
            "timeout": 1,
        }),
    };
    let handle = tokio::spawn(async move { tool.execute(call, CancellationToken::new()).await });
    let request = rx.recv().await.expect("request sent");
    assert_eq!(request.timeout, Some(1));
    assert_eq!(request.call_id, "c1");
    // 不答复，等待超时 → 返回 pending 结果。
    let result = handle.await.expect("tool joined");
    let blocks = result.unwrap();
    let rendered = text(&blocks);
    assert!(rendered.contains("\"pending\":true"));
    assert!(rendered.contains("\"callId\":\"c1\""));
}

#[tokio::test]
async fn timed_answered_before_timeout() {
    let (tool, mut rx) = tool();
    let call = ToolCall {
        id: "c2".into(),
        name: "ask_user_question".into(),
        input: json!({
            "questions": [{ "id": "q1", "question": "continue?", "options": [{ "label": "yes" }] }],
            "timeout": 30,
        }),
    };
    let handle = tokio::spawn(async move { tool.execute(call, CancellationToken::new()).await });
    let request = rx.recv().await.expect("request sent");
    request
        .reply
        .send(AskUserAnswer {
            answers: vec![crate::ask_user::types::AskUserAnswerItem {
                id: "q1".into(),
                selected: vec!["yes".into()],
                custom: None,
            }],
        })
        .expect("reply sent");
    let result = handle.await.expect("tool joined");
    let blocks = result.unwrap();
    let rendered = text(&blocks);
    assert!(rendered.contains("\"selected\":[\"yes\"]"));
    assert!(!rendered.contains("pending"));
}
