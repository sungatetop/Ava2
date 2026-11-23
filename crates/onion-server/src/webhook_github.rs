//! GitHub 入站 webhook（GitHub webhook 处理的教学子集）。
//!
//! 形态：`POST /webhooks/github`，GitHub 用 `X-Hub-Signature-256`（HMAC-SHA256，
//! 共享密钥）签名的原始 body 推事件；服务端验签通过后按「settled 规则」决定是否
//! 拉起一轮 agent 工作。
//!
//! 能力边界（诚实降级）：
//! - webhook 运行时直接内嵌为 onion-server 的一个路由，事件源只有 GitHub；
//! - settled 规则只覆盖 `issues` / `pull_request` 的 `opened` 动作（其余事件一律
//!   202 接收但 IGNORE，不创建会话）；不做 PR 增量提交（`synchronize`）触发；
//! - 不推送 GitHub 侧状态（不做 check run / comment 回写）。
//!
//! 安全约束：密钥只经环境变量 `ONION_GITHUB_WEBHOOK_SECRET` 注入（空 = 端点关闭，
//! 返回 503）；验签用常量时间比较；body 上限 1 MiB；同一 `X-GitHub-Delivery`
//! 在窗口内只处理一次（GitHub 会重投）。

use std::collections::VecDeque;
use std::sync::Mutex;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use onion::types::AgentMessage;
use serde_json::{json, Value};

use crate::router::RouterState;
use crate::session::spawn_turn;

/// 请求体上限（GitHub 事件体远小于此；超限由 body limit 层直接 413）。
pub(crate) const MAX_BODY_BYTES: usize = 1024 * 1024;

/// 重投去重窗口（保留最近 N 个 delivery id）。
const DELIVERY_WINDOW: usize = 512;

/// 提示词上限（GitHub 正文可能很长；截断避免爆上下文）。
const MAX_PROMPT_CHARS: usize = 8_000;

/// 已处理的 delivery id（进程内窗口；重投是常态，幂等由它保证）。
static SEEN_DELIVERIES: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());

/// `POST /webhooks/github`。
pub(crate) async fn handle(
    State(state): State<RouterState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Some(secret) = secret_from_env() else {
        return reply(
            StatusCode::SERVICE_UNAVAILABLE,
            json!({ "error": "github webhook disabled: ONION_GITHUB_WEBHOOK_SECRET is not set" }),
        );
    };
    if !verify_signature(
        &secret,
        &body,
        header(&headers, "x-hub-signature-256").as_deref(),
    ) {
        return reply(StatusCode::UNAUTHORIZED, json!({ "error": "invalid signature" }));
    }
    let Some(delivery) = header(&headers, "x-github-delivery") else {
        return reply(StatusCode::BAD_REQUEST, json!({ "error": "missing X-GitHub-Delivery" }));
    };
    let Some(event) = header(&headers, "x-github-event") else {
        return reply(StatusCode::BAD_REQUEST, json!({ "error": "missing X-GitHub-Event" }));
    };
    let payload: Value = match serde_json::from_slice(&body) {
        Ok(payload) => payload,
        Err(error) => {
            return reply(
                StatusCode::BAD_REQUEST,
                json!({ "error": format!("invalid JSON payload: {error}") }),
            );
        }
    };
    if event == "ping" {
        return reply(StatusCode::OK, json!({ "accepted": false, "pong": true }));
    }
    let Some(prompt) = settled_prompt(&event, &payload) else {
        return reply(
            StatusCode::ACCEPTED,
            json!({ "accepted": false, "reason": format!("event `{event}` is not dispatched") }),
        );
    };
    if !mark_delivery(&delivery) {
        return reply(
            StatusCode::OK,
            json!({ "accepted": false, "reason": "duplicate delivery" }),
        );
    }
    // 会话按非交互装配（webhook 无审批通道，与 A2A/ACP 一致）。
    let cwd = cwd_from_env();
    let session = match state.registry.create(None, None, false, cwd).await {
        Ok(session) => session,
        Err(error) => {
            return reply(
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({ "error": format!("create session failed: {error}") }),
            );
        }
    };
    session.handle.gate.enqueue();
    if let Err(error) = session
        .handle
        .runtime
        .inbox()
        .followup(AgentMessage::user(prompt))
    {
        return reply(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({ "error": format!("enqueue prompt failed: {error}") }),
        );
    }
    spawn_turn(&session.handle);
    reply(
        StatusCode::ACCEPTED,
        json!({ "accepted": true, "sessionId": session.id, "delivery": delivery }),
    )
}

/// 读取密钥；空/未设置视为端点关闭（fail-closed）。
fn secret_from_env() -> Option<String> {
    std::env::var("ONION_GITHUB_WEBHOOK_SECRET")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// 会话工作目录（`ONION_GITHUB_WEBHOOK_CWD`；缺省用服务兜底工作区）。
fn cwd_from_env() -> Option<String> {
    std::env::var("ONION_GITHUB_WEBHOOK_CWD")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// 取请求头（小写名；缺失或非 UTF-8 返回 None）。
fn header(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}

/// JSON 响应。
fn reply(status: StatusCode, body: Value) -> Response {
    (status, Json(body)).into_response()
}

/// 重投去重：首次见到返回 true；窗口内已见返回 false。
fn mark_delivery(delivery: &str) -> bool {
    let mut seen = SEEN_DELIVERIES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if seen.iter().any(|known| known == delivery) {
        return false;
    }
    seen.push_back(delivery.to_string());
    while seen.len() > DELIVERY_WINDOW {
        seen.pop_front();
    }
    true
}

/// settled 规则：仅 `issues` / `pull_request` 的 `opened` 动作为可派发事件。
fn settled_prompt(event: &str, payload: &Value) -> Option<String> {
    let action = payload.get("action").and_then(Value::as_str).unwrap_or_default();
    if action != "opened" {
        return None;
    }
    let (kind, subject) = match event {
        "issues" => ("issue", payload.get("issue")),
        "pull_request" => ("pull request", payload.get("pull_request")),
        _ => return None,
    };
    let subject = subject?;
    let title = subject.get("title").and_then(Value::as_str).unwrap_or_default();
    let body = subject.get("body").and_then(Value::as_str).unwrap_or_default();
    let number = subject
        .get("number")
        .and_then(Value::as_u64)
        .map(|number| number.to_string())
        .unwrap_or_else(|| "?".to_string());
    let url = subject.get("html_url").and_then(Value::as_str).unwrap_or_default();
    let repo = payload
        .get("repository")
        .and_then(|repository| repository.get("full_name"))
        .and_then(Value::as_str)
        .unwrap_or("unknown repository");
    Some(truncate_prompt(format!(
        "A GitHub {kind} was opened in {repo} (#{number}): {title}\n\n{body}\n\n{url}"
    )))
}

/// 提示词截断（按字符，不切 UTF-8）。
fn truncate_prompt(prompt: String) -> String {
    if prompt.chars().count() <= MAX_PROMPT_CHARS {
        return prompt;
    }
    let mut truncated: String = prompt.chars().take(MAX_PROMPT_CHARS).collect();
    truncated.push_str("\n… [truncated]");
    truncated
}

/// 验签：`sha256=<hex>`，HMAC-SHA256 常量时间比较。
fn verify_signature(secret: &str, body: &[u8], header: Option<&str>) -> bool {
    let Some(header) = header else {
        return false;
    };
    let Some(hex) = header.trim().strip_prefix("sha256=") else {
        return false;
    };
    let Some(expected) = decode_hex(hex) else {
        return false;
    };
    let actual = hmac_sha256(secret.as_bytes(), body);
    // 长度不等直接失败；等长时逐字节累积 XOR，耗时与首个差异位置无关。
    let mut diff = (actual.len() ^ expected.len()) as u8;
    for (a, b) in actual.iter().zip(expected.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

/// 十六进制解码（偶数长度；非法字符返回 None）。
fn decode_hex(hex: &str) -> Option<Vec<u8>> {
    if !hex.len().is_multiple_of(2) {
        return None;
    }
    let bytes = hex.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.chunks(2) {
        let high = (pair[0] as char).to_digit(16)?;
        let low = (pair[1] as char).to_digit(16)?;
        out.push(((high << 4) | low) as u8);
    }
    Some(out)
}

/// HMAC-SHA256（RFC 2104；密钥超块长先哈希）。
fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    const BLOCK: usize = 64;
    let mut key_block = [0u8; BLOCK];
    if key.len() > BLOCK {
        let digest = Sha256::digest(key);
        key_block[..digest.len()].copy_from_slice(&digest);
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }
    let mut inner_pad = [0x36u8; BLOCK];
    let mut outer_pad = [0x5cu8; BLOCK];
    for index in 0..BLOCK {
        inner_pad[index] ^= key_block[index];
        outer_pad[index] ^= key_block[index];
    }
    let mut inner = Sha256::new();
    inner.update(inner_pad);
    inner.update(message);
    let inner_digest = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner_digest);
    let digest = outer.finalize();
    let mut mac = [0u8; 32];
    mac.copy_from_slice(&digest);
    mac
}

#[cfg(test)]
#[path = "webhook_github_tests.rs"]
mod webhook_github_tests;
