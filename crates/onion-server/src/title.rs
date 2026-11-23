//! 会话自动标题：首轮完成后以一次轻量模型调用生成短标题并写入会话日志。
//!
//! 触发幂等：日志中已有 `SessionTitle` 直接跳过（重命名后不再覆盖）；无用户
//! 消息（仅系统注入）或无可用模型时跳过。任何失败静默——标题属体验增强，
//! 不得打断对话主流程。

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use onion::context::RequestDraft;
use onion::llm::LlmPort;
use onion::session::SessionEvent;
use onion::types::{AgentMessage, ContentBlock, StreamDelta};
use tokio_util::sync::CancellationToken;

use crate::session::SessionHandle;

/// 标题生成超时（秒）：超时放弃，保留首条消息截断标题。
const TIMEOUT_SECS: u64 = 45;
/// 标题生成的 token 预算：思考型模型会先消耗思考 token，预算过小会导致
/// content 为空（`Done` 无 error 但正文空），故留足余量。
const MAX_TOKENS: u32 = 512;
/// 标题字符上限（超长截断）。
const MAX_CHARS: usize = 24;
/// 首条用户消息取样上限（字符），避免把超长粘贴内容整段送进标题请求。
const SAMPLE_CHARS: usize = 500;

/// 系统提示：只产出标题本身，禁止解释、引号与标点装饰。
const SYSTEM_PROMPT: &str = "你是会话标题生成器。根据用户的首条消息，输出一个能概括对话主题的简短标题。\
要求：不超过 12 个汉字或 24 个字符；不要引号、句号、前缀与解释；只输出标题本身。";

/// 首轮完成后异步生成标题（不阻塞轮次闸门）。
pub(crate) fn spawn_auto_title(handle: Arc<SessionHandle>) {
    tokio::spawn(async move {
        generate(&handle).await;
    });
}

async fn generate(handle: &Arc<SessionHandle>) {
    let snapshot = handle.runtime.session().snapshot();
    // 已有显式标题（自动或手工）→ 不再覆盖。
    if snapshot.iter().any(|event| {
        matches!(event, SessionEvent::SessionTitle { title } if !title.trim().is_empty())
    }) {
        return;
    }
    let Some(first) = first_user_text(&snapshot) else {
        return;
    };
    let Some(model) = handle.model_target.current().map(|target| target.model) else {
        return;
    };
    let draft = RequestDraft {
        model,
        system: Some(SYSTEM_PROMPT.to_string()),
        messages: vec![AgentMessage::user(sample(&first))],
        tools: Vec::new(),
        max_tokens: MAX_TOKENS,
        temperature: Some(0.2),
        starts_request_series: true,
    };
    let mut stream = handle.llm.stream(&draft, CancellationToken::new()).await;
    let mut text = String::new();
    let mut thinking_chars = 0usize;
    let collected = tokio::time::timeout(Duration::from_secs(TIMEOUT_SECS), async {
        while let Some(item) = stream.next().await {
            match item {
                onion::llm::LlmStreamItem::Delta(StreamDelta::Text { text: chunk }) => {
                    text.push_str(&chunk)
                }
                onion::llm::LlmStreamItem::Delta(StreamDelta::Thinking { thinking }) => {
                    thinking_chars += thinking.chars().count()
                }
                onion::llm::LlmStreamItem::Done(turn) => {
                    // 流内错误（非 2xx / 传输失败）不产标题。
                    if turn.error.is_some() {
                        return None;
                    }
                    return Some(text);
                }
                _ => {}
            }
        }
        Some(text)
    })
    .await
    .ok()
    .flatten();
    let Some(raw) = collected else {
        eprintln!("[onion-server] 自动标题失败：模型流错误或超时");
        return;
    };
    let Some(title) = normalize(&raw) else {
        eprintln!(
            "[onion-server] 自动标题失败：模型输出为空（thinking {thinking_chars} 字符）"
        );
        return;
    };
    eprintln!("[onion-server] 会话标题已生成：{title}");
    handle
        .runtime
        .session()
        .append(SessionEvent::SessionTitle { title });
}

/// 首条真实用户消息文本（跳过系统注入与历史宿主包裹）。
fn first_user_text(events: &[SessionEvent]) -> Option<String> {
    events.iter().find_map(|event| {
        let SessionEvent::UserMessage { message, .. } = event else {
            return None;
        };
        if message.source.is_some() {
            return None;
        }
        let text = message
            .blocks
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<String>();
        let trimmed = text.trim();
        let injected = trimmed.starts_with("<system-reminder>")
            || trimmed.starts_with("<environment")
            || trimmed.starts_with("Current runtime context.");
        (!trimmed.is_empty() && !injected).then(|| trimmed.to_string())
    })
}

fn sample(text: &str) -> String {
    if text.chars().count() > SAMPLE_CHARS {
        text.chars().take(SAMPLE_CHARS).collect()
    } else {
        text.to_string()
    }
}

/// 清洗模型输出：取首行、去引号/书名号包裹与结尾标点、限长。
fn normalize(raw: &str) -> Option<String> {
    let line = raw.lines().map(str::trim).find(|line| !line.is_empty())?;
    let stripped = line
        .trim_matches(|c: char| {
            matches!(c, '"' | '\'' | '“' | '”' | '‘' | '’' | '《' | '》' | '「' | '」' | '`' | '#')
        })
        .trim_end_matches(|c: char| matches!(c, '。' | '.' | '，' | ',' | '：' | ':'));
    let mut title: String = stripped.chars().take(MAX_CHARS).collect();
    if stripped.chars().count() > MAX_CHARS {
        title.push('…');
    }
    let title = title.trim().to_string();
    (!title.is_empty()).then_some(title)
}
