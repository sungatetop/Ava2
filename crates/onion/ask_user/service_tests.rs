//! `service` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use crate::ask_user::service::answer_to_pending_message;
use crate::ask_user::types::{AskUserAnswer, AskUserAnswerItem, AskUserQuestion};
use crate::types::{ContentBlock, MessageSource, Role};

fn question(id: &str) -> AskUserQuestion {
    AskUserQuestion {
        id: id.to_string(),
        question: "continue?".to_string(),
        header: None,
        options: None,
        multi_select: None,
    }
}

#[test]
fn answer_to_pending_message_carries_call_id_and_questions() {
    let answer = AskUserAnswer {
        answers: vec![AskUserAnswerItem {
            id: "q1".to_string(),
            selected: vec!["yes".to_string()],
            custom: None,
        }],
    };
    let message = answer_to_pending_message("call-7", &[question("q1")], &answer);
    assert_eq!(message.role, Role::User);
    assert_eq!(
        message.source,
        Some(MessageSource::AnswerToPending { call_id: "call-7".to_string() })
    );
    let ContentBlock::Text { text } = &message.blocks[0] else {
        panic!("迟到答复应为单个文本块");
    };
    let payload: serde_json::Value = serde_json::from_str(text).expect("payload is json");
    assert_eq!(payload["kind"], "answer_to_pending_question");
    assert_eq!(payload["tool"], "ask_user_question");
    assert_eq!(payload["callId"], "call-7");
    assert_eq!(payload["questions"][0]["id"], "q1");
    assert_eq!(payload["answers"][0]["selected"][0], "yes");
}
