//! LlmPort trait + OpenAI `/chat/completions` 兼容 SSE 真流式适配器。
//! 参照 chat/llm.rs 形态；错误进流契约：传输失败 / 非 2xx / 取消 / 流中断
//! 均编码为 `Done(AssistantTurn::error)`，不返回 Err。

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use futures_util::{Stream, StreamExt, stream};
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use crate::context::RequestDraft;
use crate::model_target::{ModelTarget, ModelTargetRef};
use crate::types::{
    AgentMessage, ContentBlock, StopReason, StreamDelta, ToolDefinition, Usage,
};

/// LLM 端口：内核唯一的模型请求缝（demo 内自实现）。
pub trait LlmPort: Send + Sync {
    fn stream(&self, draft: &RequestDraft, cancel: CancellationToken) -> crate::waterfall::BoxFuture<LlmStream>;
}

pub type LlmStream = stream::BoxStream<'static, LlmStreamItem>;

/// 落定的一轮 assistant 输出（成功或错误）。
#[derive(Clone, Debug)]
pub struct AssistantTurn {
    pub message: AgentMessage,
    pub stop_reason: StopReason,
    pub error: Option<String>,
    /// 结构化失败分类（error 为 Some 时通常伴随；供重试决策消费）。
    pub failure: Option<LlmFailure>,
    pub cancelled: bool,
    pub usage: Usage,
    pub model: String,
}

impl AssistantTurn {
    pub fn error(model: impl Into<String>, message: impl Into<String>, cancelled: bool) -> Self {
        Self {
            message: AgentMessage::assistant(Vec::new()),
            stop_reason: StopReason::Cancelled,
            error: Some(message.into()),
            failure: None,
            cancelled,
            usage: Usage::default(),
            model: model.into(),
        }
    }

    /// 附加结构化失败（builder；分类由适配器产生，内核/监听者消费）。
    pub fn with_failure(mut self, failure: LlmFailure) -> Self {
        self.failure = Some(failure);
        self
    }
}

/// LLM 失败分类（adapter 层失败归类 + 传输层状态码映射）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LlmFailureCode {
    /// 429 限流（Retry-After 透传）。
    RateLimit,
    /// 5xx 服务端错误。
    Server,
    /// 请求超时。
    Timeout,
    /// 连接/传输层失败（含流中断）。
    Transport,
    /// 落定后无任何内容块（空响应）。
    EmptyResponse,
    /// 上下文溢出（compaction 恢复前提）。
    ContextWindowExceeded,
    /// 400/413 请求非法。
    InvalidRequest,
    /// 401/403 鉴权失败。
    Auth,
    /// 402 配额不足。
    Quota,
    /// 其余 HTTP 状态兜底（错误码形如 `HTTP_{status}`）。
    Http(u16),
}

/// 结构化 LLM 失败：分类 code + 人类可读消息 + Retry-After 透传。
#[derive(Clone, Debug)]
pub struct LlmFailure {
    pub code: LlmFailureCode,
    pub message: String,
    pub retry_after_ms: Option<u64>,
}

/// 供应商上下文溢出错误标记（宽松匹配；教学示例不做逐供应商精确映射）。
/// [classify_status] 命中时归为 [LlmFailureCode::ContextWindowExceeded]，
/// compaction listener 据分类判定溢出恢复（取代逐点字符串匹配）。
const OVERFLOW_MARKERS: &[&str] = &[
    "context length",
    "context_length_exceeded",
    "maximum context length",
    "too many tokens",
    "token limit",
    "context window",
    "prompt is too long",
    "input length exceeds",
    "request too large",
];

/// 错误信息是否为上下文溢出（宽松词表匹配）。
pub fn is_context_overflow_detail(detail: &str) -> bool {
    let lower = detail.to_lowercase();
    OVERFLOW_MARKERS.iter().any(|marker| lower.contains(marker))
}

/// 非成功状态 → 失败分类：
/// 溢出词表优先（溢出可能以 4xx/5xx 多种状态呈现），再按状态码映射。
pub fn classify_status(status: u16, body: &str) -> LlmFailureCode {
    if is_context_overflow_detail(body) {
        return LlmFailureCode::ContextWindowExceeded;
    }
    match status {
        429 => LlmFailureCode::RateLimit,
        401 | 403 => LlmFailureCode::Auth,
        402 => LlmFailureCode::Quota,
        400 | 413 => LlmFailureCode::InvalidRequest,
        status if (500..600).contains(&status) => LlmFailureCode::Server,
        other => LlmFailureCode::Http(other),
    }
}

/// Retry-After 响应头解析：delta-seconds → 毫秒。HTTP-date 形态教学版不解析
/// （主流网关均发秒数）；0 / 非法值返回 None（无有效透传）。
pub fn parse_retry_after_ms(value: &str) -> Option<u64> {
    let seconds: u64 = value.trim().parse().ok()?;
    (seconds > 0).then_some(seconds.saturating_mul(1000))
}

pub enum LlmStreamItem {
    Delta(StreamDelta),
    Done(AssistantTurn),
}

/// OpenAI 兼容 SSE 流式适配器。
///
/// 运行期读 `ModelTargetRef`（缺省构造为静态 base_url/api_key 兼容既有调用方）：
/// 当持有 `target` 时，每次 `stream` 读取当前 assembled 目标，替换 base_url /
/// api_key / model / effort——运行期切换立即生效于下一请求。
#[derive(Clone)]
pub struct OpenAiSseLlm {
    pub base_url: String,
    pub api_key: String,
    pub client: reqwest::Client,
    /// 运行期模型目标引用（None = 静态 base_url/api_key 模式）。
    pub target: Option<Arc<ModelTargetRef>>,
}

impl OpenAiSseLlm {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            api_key: api_key.into(),
            client: reqwest::Client::new(),
            target: None,
        }
    }

    /// 绑定运行期模型目标引用（/model 切换经它生效）。
    pub fn with_target(mut self, target: Arc<ModelTargetRef>) -> Self {
        self.target = Some(target);
        self
    }

    /// 解析本次请求的目标：优先 `target.assembled()`（当前 step 快照），
    /// 回退 `target.current()`，再回退静态字段。返回 (target, resolved_model)。
    fn resolve(&self, draft_model: &str) -> (Option<ModelTarget>, String) {
        if let Some(target) = &self.target {
            if let Some(assembled) = target.assembled() {
                let model = if assembled.model.is_empty() { draft_model.to_string() } else { assembled.model.clone() };
                return (Some(assembled), model);
            }
            if let Some(current) = target.current() {
                let model = if current.model.is_empty() { draft_model.to_string() } else { current.model.clone() };
                return (Some(current), model);
            }
        }
        (None, draft_model.to_string())
    }
}

impl LlmPort for OpenAiSseLlm {
    fn stream(&self, draft: &RequestDraft, cancel: CancellationToken) -> crate::waterfall::BoxFuture<LlmStream> {
        let (target, model) = self.resolve(&draft.model);
        let draft = draft.clone();
        let client = self.client.clone();
        let (base_url, api_key) = match &target {
            Some(t) => (t.base_url.clone(), t.api_key.clone()),
            None => (self.base_url.clone(), self.api_key.clone()),
        };
        let effort = target.as_ref().and_then(|t| t.effort.clone());
        let url = format!("{base_url}/chat/completions");
        Box::pin(async move {
            let response = client
                .post(&url)
                .bearer_auth(&api_key)
                .json(&build_payload(&draft, &model, effort.as_deref()))
                .send()
                .await;
            match response {
                Ok(response) if response.status().is_success() => Box::pin(stream::unfold(
                    StreamState {
                        body: Some(Box::pin(response.bytes_stream())),
                        buffer: String::new(),
                        pending: VecDeque::new(),
                        accum: SseAccum::default(),
                        cancel,
                        model: model.clone(),
                        finished: false,
                    },
                    |state: StreamState| {
                        Box::pin(pump(state))
                            as Pin<Box<dyn Future<Output = Option<(LlmStreamItem, StreamState)>> + Send>>
                    },
                )) as LlmStream,
                Ok(response) => {
                    let status = response.status();
                    // Retry-After 透传须在消费 body 前读取（text() 会消耗响应）。
                    let retry_after_ms = response
                        .headers()
                        .get("retry-after")
                        .and_then(|value| value.to_str().ok())
                        .and_then(parse_retry_after_ms);
                    let body = response.text().await.unwrap_or_default();
                    let failure = LlmFailure {
                        code: classify_status(status.as_u16(), &body),
                        message: format!("HTTP {status}: {body}"),
                        retry_after_ms,
                    };
                    done_stream(model, failure, /*cancelled*/ false)
                }
                Err(error) => {
                    let failure = LlmFailure {
                        code: classify_transport(&error),
                        message: format!("HTTP 请求失败: {error}"),
                        retry_after_ms: None,
                    };
                    done_stream(model, failure, cancel.is_cancelled())
                }
            }
        })
    }
}

/// `RequestDraft` → OpenAI 线协议 payload（请求侧唯一转换点）。
/// `model` 为解析后的目标模型；`effort` 可选 `reasoning_effort` 透传。
fn build_payload(draft: &RequestDraft, model: &str, effort: Option<&str>) -> Value {
    let mut messages: Vec<Value> = Vec::new();
    if let Some(system) = &draft.system {
        messages.push(json!({ "role": "system", "content": system }));
    }
    for message in &draft.messages {
        if let Some(entry) = wire_message(message) {
            messages.push(entry);
        }
    }
    let mut payload = json!({
        "model": model,
        "stream": true,
        "stream_options": { "include_usage": true },
        "max_tokens": draft.max_tokens,
        "messages": messages,
    });
    if let Some(temperature) = draft.temperature {
        payload["temperature"] = json!(temperature);
    }
    if let Some(effort) = effort {
        payload["reasoning_effort"] = json!(effort);
    }
    if !draft.tools.is_empty() {
        payload["tools"] = Value::Array(draft.tools.iter().map(wire_tool).collect());
    }
    payload
}

fn wire_tool(tool: &ToolDefinition) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": tool.name,
            "description": tool.description,
            "parameters": tool.parameters,
        }
    })
}

/// 单条消息 → OpenAI 线协议（assistant 思考块不回传；tool 结果含图片时 content 用 parts 数组）。
fn wire_message(message: &AgentMessage) -> Option<Value> {
    match message.role {
        crate::types::Role::User => {
            if message.blocks.iter().any(|b| matches!(b, ContentBlock::Image { .. })) {
                let parts: Vec<Value> = message
                    .blocks
                    .iter()
                    .filter_map(|block| match block {
                        ContentBlock::Text { text } => Some(json!({ "type": "text", "text": text })),
                        ContentBlock::Image { image_url } => {
                            Some(json!({ "type": "image_url", "image_url": { "url": image_url } }))
                        }
                        _ => None,
                    })
                    .collect();
                return Some(json!({ "role": "user", "content": parts }));
            }
            let text: String = message
                .blocks
                .iter()
                .filter_map(|block| match block {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect();
            (!text.is_empty()).then(|| json!({ "role": "user", "content": text }))
        }
        crate::types::Role::Assistant => {
            let mut text = String::new();
            let mut tool_calls: Vec<Value> = Vec::new();
            for block in &message.blocks {
                match block {
                    ContentBlock::Text { text: part } => text.push_str(part),
                    ContentBlock::ToolUse { id, name, input } => tool_calls.push(json!({
                        "id": id,
                        "type": "function",
                        "function": { "name": name, "arguments": input.to_string() },
                    })),
                    _ => {}
                }
            }
            if text.is_empty() && tool_calls.is_empty() {
                return None;
            }
            let mut entry = json!({
                "role": "assistant",
                "content": if text.is_empty() { Value::Null } else { Value::String(text) },
            });
            if !tool_calls.is_empty() {
                entry["tool_calls"] = Value::Array(tool_calls);
            }
            Some(entry)
        }
        crate::types::Role::Tool => {
            for block in &message.blocks {
                if let ContentBlock::ToolResult { tool_call_id, content, .. } = block {
                    // 结果含图片时 content 用 parts 数组（text + image_url），
                    // 图片块是多模态输入的正式载体，不得丢弃。
                    let has_image = content
                        .iter()
                        .any(|part| matches!(part, ContentBlock::Image { .. }));
                    if !has_image {
                        let text: String = content
                            .iter()
                            .filter_map(|part| match part {
                                ContentBlock::Text { text } => Some(text.as_str()),
                                _ => None,
                            })
                            .collect();
                        return Some(json!({
                            "role": "tool",
                            "tool_call_id": tool_call_id,
                            "content": text
                        }));
                    }
                    let parts: Vec<Value> = content
                        .iter()
                        .filter_map(|part| match part {
                            ContentBlock::Text { text } => {
                                Some(json!({ "type": "text", "text": text }))
                            }
                            ContentBlock::Image { image_url } => Some(
                                json!({ "type": "image_url", "image_url": { "url": image_url } }),
                            ),
                            _ => None,
                        })
                        .collect();
                    return Some(json!({ "role": "tool", "tool_call_id": tool_call_id, "content": parts }));
                }
            }
            None
        }
    }
}

#[derive(Default)]
struct SseAccum {
    text: String,
    thinking: String,
    calls: Vec<(usize, String, String, String)>,
    finish_reason: Option<String>,
    usage: Option<Value>,
}

struct StreamState {
    body: Option<Pin<Box<dyn Stream<Item = reqwest::Result<bytes::Bytes>> + Send>>>,
    buffer: String,
    pending: VecDeque<LlmStreamItem>,
    accum: SseAccum,
    cancel: CancellationToken,
    model: String,
    finished: bool,
}

/// 传输层错误分类：超时 → Timeout，其余 → Transport（含连接失败与流中断）。
fn classify_transport(error: &reqwest::Error) -> LlmFailureCode {
    if error.is_timeout() {
        LlmFailureCode::Timeout
    } else {
        LlmFailureCode::Transport
    }
}

/// 错误落定进流：错误轮 + 结构化失败（取消路径不经此函数）。
fn done_stream(model: String, failure: LlmFailure, cancelled: bool) -> LlmStream {
    Box::pin(stream::iter(vec![LlmStreamItem::Done(
        AssistantTurn::error(model, failure.message.clone(), cancelled).with_failure(failure),
    )]))
}

/// 中断落定（部分文本保真）：取消/传输失败的错误进流收尾保留已累积的
/// 部分增量——已流出的文本/思考随落定消息可见，不因中断丢弃。
fn partial_error(state: &StreamState, message: String, cancelled: bool) -> AssistantTurn {
    let mut settled = AssistantTurn::error(state.model.clone(), message, cancelled);
    settled.message = AgentMessage::assistant(final_blocks(&state.accum));
    settled.usage = usage_from_json(state.accum.usage.as_ref());
    settled
}

async fn pump(mut state: StreamState) -> Option<(LlmStreamItem, StreamState)> {
    loop {
        if let Some(item) = state.pending.pop_front() {
            return Some((item, state));
        }
        while let Some(position) = state.buffer.find("\n\n") {
            let block: String = state.buffer.drain(..position + 2).collect();
            feed_block(&block, &mut state.accum, &mut state.pending);
        }
        if let Some(item) = state.pending.pop_front() {
            return Some((item, state));
        }
        if state.cancel.is_cancelled() {
            // 取消落定只发一次：`finished` 后再被轮询必须结束流，否则
            // 上层 `while let Some(item)` 会无限收到 Done，整轮 Live 循环
            // 空转并长期持有内核锁（表现为服务整体无响应）。
            if state.finished {
                return None;
            }
            state.finished = true;
            let settled = partial_error(&state, "cancelled during stream".to_string(), true);
            return Some((LlmStreamItem::Done(settled), state));
        }
        let Some(mut body) = state.body.take() else {
            if state.finished {
                return None;
            }
            if !state.buffer.trim().is_empty() {
                let block = std::mem::take(&mut state.buffer);
                feed_block(&block, &mut state.accum, &mut state.pending);
            }
            if let Some(item) = state.pending.pop_front() {
                return Some((item, state));
            }
            state.finished = true;
            return Some((settle(&state), state));
        };
        // 取消可即时打断阻塞读：不等下一条 SSE 分片（上游停摆时 interrupt
        // 仍能立刻生效）；取消分支丢弃 body 连带关闭 HTTP 连接。
        let cancel = state.cancel.clone();
        let chunk = tokio::select! {
            biased;
            () = cancel.cancelled() => {
                state.finished = true;
                drop(body);
                let settled = partial_error(&state, "cancelled during stream".to_string(), true);
                return Some((LlmStreamItem::Done(settled), state));
            }
            chunk = body.next() => chunk,
        };
        match chunk {
            Some(Ok(bytes)) => {
                state.body = Some(body);
                state.buffer.push_str(&String::from_utf8_lossy(&bytes).replace("\r\n", "\n"));
            }
            Some(Err(error)) => {
                state.finished = true;
                let message = format!("流式传输失败: {error}");
                let settled = partial_error(&state, message.clone(), /*cancelled*/ false)
                    .with_failure(LlmFailure {
                        code: classify_transport(&error),
                        message,
                        retry_after_ms: None,
                    });
                return Some((LlmStreamItem::Done(settled), state));
            }
            None => {}
        }
    }
}

fn settle(state: &StreamState) -> LlmStreamItem {
    let blocks = final_blocks(&state.accum);
    // 退化完成（空响应）：落定后无任何内容块（无思考/
    // 文本/工具调用）→ 编码为可重试失败，不再静默产出空 assistant 轮。
    if blocks.is_empty() {
        let failure = LlmFailure {
            code: LlmFailureCode::EmptyResponse,
            message: "供应商返回空响应（无任何内容块）".to_string(),
            retry_after_ms: None,
        };
        return LlmStreamItem::Done(
            AssistantTurn::error(state.model.clone(), failure.message.clone(), false)
                .with_failure(failure),
        );
    }
    let stop_reason = match state.accum.finish_reason.as_deref() {
        Some(reason) => StopReason::from_provider(reason),
        None if state.accum.calls.is_empty() => StopReason::EndTurn,
        None => StopReason::ToolUse,
    };
    LlmStreamItem::Done(AssistantTurn {
        message: AgentMessage::assistant(blocks),
        stop_reason,
        error: None,
        failure: None,
        cancelled: false,
        usage: usage_from_json(state.accum.usage.as_ref()),
        model: state.model.clone(),
    })
}

fn feed_block(block: &str, accum: &mut SseAccum, items: &mut VecDeque<LlmStreamItem>) {
    for line in block.lines() {
        let Some(data) = line.strip_prefix("data:") else { continue };
        let data = data.trim();
        if data.is_empty() || data == "[DONE]" {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(data) else { continue };
        if let Some(usage) = value.get("usage") {
            if !usage.is_null() {
                accum.usage = Some(usage.clone());
            }
        }
        let Some(choices) = value.get("choices").and_then(Value::as_array) else { continue };
        for choice in choices {
            if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
                accum.finish_reason = Some(reason.to_string());
            }
            let Some(delta) = choice.get("delta") else { continue };
            if let Some(text) = delta.get("content").and_then(Value::as_str) {
                if !text.is_empty() {
                    accum.text.push_str(text);
                    items.push_back(LlmStreamItem::Delta(StreamDelta::Text { text: text.to_string() }));
                }
            }
            // 思考内容字段名兼容：`reasoning_content`（DeepSeek/OpenAI 官方系）
            // 与 `reasoning`（vLLM/Qwen3 系）二选一，同时存在时优先前者。
            let thinking = delta
                .get("reasoning_content")
                .or_else(|| delta.get("reasoning"))
                .and_then(Value::as_str);
            if let Some(thinking) = thinking {
                if !thinking.is_empty() {
                    accum.thinking.push_str(thinking);
                    items.push_back(LlmStreamItem::Delta(StreamDelta::Thinking { thinking: thinking.to_string() }));
                }
            }
            let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) else { continue };
            for call in tool_calls {
                let index = call.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                if let Some(id) = call.get("id").and_then(Value::as_str) {
                    let name = call
                        .pointer("/function/name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    items.push_back(LlmStreamItem::Delta(StreamDelta::ToolCallStart {
                        index,
                        id: id.to_string(),
                        name: name.clone(),
                    }));
                    match accum.calls.iter_mut().find(|(i, ..)| *i == index) {
                        Some(entry) => {
                            entry.1 = id.to_string();
                            entry.2 = name;
                        }
                        None => accum.calls.push((index, id.to_string(), name, String::new())),
                    }
                }
                if let Some(arguments) = call.pointer("/function/arguments").and_then(Value::as_str) {
                    if !arguments.is_empty() {
                        items.push_back(LlmStreamItem::Delta(StreamDelta::ToolCallInput {
                            index,
                            partial_json: arguments.to_string(),
                        }));
                        match accum.calls.iter_mut().find(|(i, ..)| *i == index) {
                            Some(entry) => entry.3.push_str(arguments),
                            None => accum.calls.push((index, String::new(), String::new(), arguments.to_string())),
                        }
                    }
                }
            }
        }
    }
}

fn final_blocks(accum: &SseAccum) -> Vec<ContentBlock> {
    let mut blocks = Vec::new();
    if !accum.thinking.is_empty() {
        blocks.push(ContentBlock::Thinking { thinking: accum.thinking.clone() });
    }
    if !accum.text.is_empty() {
        blocks.push(ContentBlock::Text { text: accum.text.clone() });
    }
    let mut calls = accum.calls.clone();
    calls.sort_by_key(|(index, ..)| *index);
    for (_, id, name, arguments) in calls {
        let input = serde_json::from_str::<Value>(&arguments).unwrap_or(Value::Null);
        blocks.push(ContentBlock::ToolUse { id, name, input });
    }
    blocks
}

fn usage_from_json(value: Option<&Value>) -> Usage {
    let Some(value) = value else { return Usage::default() };
    Usage {
        input_tokens: value.get("prompt_tokens").and_then(Value::as_u64).unwrap_or(0),
        output_tokens: value.get("completion_tokens").and_then(Value::as_u64).unwrap_or(0),
        cache_read_tokens: value
            .pointer("/prompt_tokens_details/cached_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        cache_write_tokens: 0,
    }
}

#[cfg(test)]
#[path = "llm_tests.rs"]
mod tests;
