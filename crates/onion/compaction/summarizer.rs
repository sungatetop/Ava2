//! LLM 摘要器：复用内核 `LlmPort` 做一次性压缩调用。
//!
//! 语义：replay 对话前缀 + instruction 作为最后 user 消息（KV cache 前缀
//! 复用）；summary 拒图片只留 text 块；摘要帧以 `<compacted-summary>`
//! 标签包裹 + CHECKPOINT_PREAMBLE。

use futures_util::StreamExt;
use tokio_util::sync::CancellationToken;

use crate::context::RequestDraft;
use crate::llm::{LlmPort, LlmStreamItem};
use crate::types::{AgentMessage, Usage};

/// 摘要标签（开/闭标签成对出现）。
pub const SUMMARY_OPEN_TAG: &str = "<compacted-summary>";
pub const SUMMARY_CLOSE_TAG: &str = "</compacted-summary>";

/// 固定 7 节 Markdown 结构（摘要指令）。
pub const COMPACTION_INSTRUCTION: &str = "You are now acting as a compaction engine for this AI coding assistant. Condense the conversation ABOVE into a structured checkpoint that lets another model resume the work with no loss of essential context.

Output EXACTLY the Markdown structure below: keep every section, in order. Use terse bullets, not prose paragraphs. Write \"(none)\" for an empty section — never drop a section.

## Primary Request and Intent
- [the user's original and evolving goals; quote verbatim where the exact wording matters]

## Key Technical Concepts
- [technologies, frameworks, patterns, and conventions in play]

## Files and Code
- [exact path: why it matters, key changes or snippets]

## Errors and Fixes
- [error: how it was resolved, plus any related user feedback]

## Pending Jobs
- [explicitly requested work not yet completed]

## Current Work
- [precisely what was in progress at this checkpoint]

## Next Step
- [the single next action, directly in line with the most recent request, or \"(none)\"]

## Critical Context
- [decisions and their rationale, constraints, user preferences, open questions, data needed to continue]

Rules:
- Write concise English engineering prose. Preserve exact file paths, commands, error strings, identifiers, numeric values, function signatures, and syntax fragments.
- Capture user feedback and explicit instructions faithfully, especially corrections.
- Do NOT mention this summarization request or that the context was compacted.
- Output only the checkpoint text: do not call any tool or take any other action.
- If the conversation already contains a <compacted-summary> block, it is a PRIOR checkpoint. Do not copy it forward verbatim: preserve still-true facts, drop stale ones, and merge newer information into a single consolidated summary under the same structure.";

/// checkpoint 前导词。
pub const CHECKPOINT_PREAMBLE: &str = "This is an automatically generated checkpoint condensing an earlier span of the conversation to free up context. Treat the captured context as established background and build on it without restating it. Continue the task directly from the messages that follow, without acknowledging this checkpoint.";

/// 一次摘要调用的配置（model 复用主链路，避免空 model 被供应商拒绝）。
#[derive(Clone, Debug)]
pub struct SummaryConfig {
    pub model: String,
    pub max_tokens: u32,
    pub temperature: Option<f32>,
}

/// 摘要结果（safe 文本块 + 调用信封 + usage）。
#[derive(Clone, Debug, PartialEq)]
pub struct SummaryResult {
    pub summary: String,
    pub model: String,
    pub usage: Usage,
}

/// 一次摘要调用失败。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SummaryError(pub String);

impl std::fmt::Display for SummaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SummaryError {}

/// 压缩摘要调用（复用内核 `LlmPort` 端口；目标模型复用主链路）。
pub async fn summarize_with_llm(
    llm: &dyn LlmPort,
    config: SummaryConfig,
    replay: Vec<AgentMessage>,
    cancel: CancellationToken,
) -> Result<SummaryResult, SummaryError> {
    let draft = RequestDraft {
        model: config.model.clone(),
        system: None,
        messages: replay,
        tools: Vec::new(),
        max_tokens: config.max_tokens,
        temperature: config.temperature,
        starts_request_series: false,
    };
    let mut stream = llm.stream(&draft, cancel).await;
    let mut text = String::new();
    let mut usage = Usage::default();
    let mut model_out = String::new();
    while let Some(item) = stream.next().await {
        match item {
            LlmStreamItem::Delta(_) => {}
            LlmStreamItem::Done(turn) => {
                model_out = turn.model.clone();
                usage = turn.usage;
                if turn.cancelled {
                    return Err(SummaryError("cancelled".to_string()));
                }
                if let Some(error) = turn.error {
                    return Err(SummaryError(error));
                }
                let joined = turn
                    .message
                    .blocks
                    .iter()
                    .filter_map(|block| match block {
                        crate::types::ContentBlock::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                if joined.trim().is_empty() {
                    return Err(SummaryError(
                        "summarization produced no text summary content".to_string(),
                    ));
                }
                text = joined;
            }
        }
    }
    Ok(SummaryResult { summary: text, model: model_out, usage })
}

/// 摘要落地为 user 消息（preamble + open + summary + close）。
pub fn frame_summary(summary: &str) -> AgentMessage {
    AgentMessage::user(format!(
        "{CHECKPOINT_PREAMBLE}\n\n{SUMMARY_OPEN_TAG}\n{summary}\n{SUMMARY_CLOSE_TAG}"
    ))
}

/// 摘要落地为**压缩检查点**消息（`source: compact-checkpoint`）：
/// 正文同 [`frame_summary`]，额外带
/// `source: compact-checkpoint` 标记，使投影与前端能识别「这条 user 消息是压缩
/// 产生的上下文替换」而非真实用户输入。
pub fn frame_checkpoint(summary: &str, compaction_id: &str) -> AgentMessage {
    let mut message = frame_summary(summary);
    message.source = Some(crate::types::MessageSource::CompactCheckpoint {
        compaction_id: compaction_id.to_string(),
    });
    message
}

#[cfg(test)]
#[path = "summarizer_tests.rs"]
mod tests;
