//! `summarizer` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;
use crate::llm::{AssistantTurn, LlmStream};
use crate::types::StopReason;

/// 确定性 fake 流（单 Done 帧）。
fn done_stream(turn: AssistantTurn) -> LlmStream {
    Box::pin(futures_util::stream::iter(vec![LlmStreamItem::Done(turn)]))
}

struct FakeLlm {
    turn: AssistantTurn,
}

impl LlmPort for FakeLlm {
    fn stream(
        &self,
        _draft: &RequestDraft,
        _cancel: tokio_util::sync::CancellationToken,
    ) -> crate::waterfall::BoxFuture<LlmStream> {
        let turn = self.turn.clone();
        Box::pin(async move { done_stream(turn) })
    }
}

fn ok_turn(summary: &str) -> AssistantTurn {
    AssistantTurn {
        message: AgentMessage::assistant(vec![crate::types::ContentBlock::Text {
            text: summary.to_string(),
        }]),
        stop_reason: StopReason::EndTurn,
        error: None,
        failure: None,
        cancelled: false,
        usage: Usage::default(),
        model: "fake-model".to_string(),
    }
}

#[tokio::test]
async fn success_summary_collected() {
    let llm = FakeLlm { turn: ok_turn("## Current Work\n- compaction demo") };
    let result = super::summarize_with_llm(
        &llm,
        super::SummaryConfig { model: "fake-model".to_string(), max_tokens: 256, temperature: None },
        vec![AgentMessage::user("replay")],
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .expect("must succeed");
    assert_eq!(result.summary, "## Current Work\n- compaction demo");
    assert_eq!(result.model, "fake-model");
}

#[tokio::test]
async fn empty_summary_is_error() {
    let llm = FakeLlm { turn: ok_turn("   ") };
    let error = super::summarize_with_llm(
        &llm,
        super::SummaryConfig { model: "fake-model".to_string(), max_tokens: 256, temperature: None },
        vec![],
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .expect_err("empty summary must fail");
    assert_eq!(error.0, "summarization produced no text summary content");
}

#[tokio::test]
async fn provider_error_propagates() {
    let mut turn = ok_turn("ignored");
    turn.error = Some("HTTP 500".to_string());
    turn.cancelled = false;
    let llm = FakeLlm { turn };
    let error = super::summarize_with_llm(
        &llm,
        super::SummaryConfig { model: "fake-model".to_string(), max_tokens: 256, temperature: None },
        vec![],
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .expect_err("error turn must fail");
    assert_eq!(error.0, "HTTP 500");
}

#[tokio::test]
async fn cancelled_summary_is_error() {
    let mut turn = ok_turn("ignored");
    turn.cancelled = true;
    let llm = FakeLlm { turn };
    let error = super::summarize_with_llm(
        &llm,
        super::SummaryConfig { model: "fake-model".to_string(), max_tokens: 256, temperature: None },
        vec![],
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .expect_err("cancelled must fail");
    assert_eq!(error.0, "cancelled");
}

#[test]
fn frame_summary_wraps_with_tags() {
    let framed = super::frame_summary("body");
    let text = framed.text_content();
    assert!(text.contains(super::CHECKPOINT_PREAMBLE));
    assert!(text.contains(super::SUMMARY_OPEN_TAG));
    assert!(text.contains("body"));
    assert!(text.contains(super::SUMMARY_CLOSE_TAG));
}

#[test]
fn instruction_contains_all_sections() {
    for section in [
        "## Primary Request and Intent",
        "## Key Technical Concepts",
        "## Files and Code",
        "## Errors and Fixes",
        "## Pending Jobs",
        "## Current Work",
        "## Next Step",
        "## Critical Context",
    ] {
        assert!(super::COMPACTION_INSTRUCTION.contains(section));
    }
}
