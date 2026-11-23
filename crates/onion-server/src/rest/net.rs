//! net：AppHost 统一联网服务（App 外联唯一出口）。
//!
//! 浏览器侧的 App（web-ui / Tauri webview）受目标站点 CORS 白名单约束，无法
//! 直连第三方公开 API；本服务为 App 提供**受控外联转发**：
//!
//! - `POST /v1/net/fetch`：通用形态——`{url, method?, headers?, body?, encoding?}`；
//! - `GET  /v1/net/fetch?url=`：只读便捷形态（等价 method=GET）。
//!
//! 安全边界（两层中 服务端是底线，App 级域白名单在 host 层）：
//!
//! - 仅 https + 公网地址（URL 字面量校验：拒绝 localhost / 私网 / 环回 /
//!   链路本地 IP / 无点单标签主机名），重定向逐跳复检（防跳转绕过）；
//! - 方法仅 GET / POST；请求头白名单透传（authorization 等鉴权刚需）；
//! - 响应上限 4 MiB；上游超时 15s；上游 HTTP 状态码透传（非 200 不是网关
//!   错误，调用方自行判定）。

use std::collections::HashMap;
use std::time::Duration;

use axum::extract::{Query, State};
use axum::Json;
use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::rest::error::ApiError;
use crate::router::RouterState;

/// 上游访问超时。
const UPSTREAM_TIMEOUT: Duration = Duration::from_secs(15);

/// 响应体上限（4 MiB）。
const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;

/// 允许的重定向跳数上限。
const MAX_REDIRECTS: usize = 5;

/// 请求头透传白名单（鉴权与协商刚需；其余一律丢弃，防 CRLF 注入与头滥用）。
const ALLOWED_HEADERS: [&str; 4] = ["accept", "authorization", "content-type", "user-agent"];

/// `POST /v1/net/fetch` 入参（camelCase 对齐前端契约）。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NetFetchRequest {
    pub url: String,
    /// GET | POST（缺省 GET；大写归一）。
    pub method: Option<String>,
    /// 透传头（白名单字段生效，其余丢弃）。
    pub headers: Option<HashMap<String, String>>,
    /// 请求体（仅 POST；UTF-8 文本）。
    pub body: Option<String>,
    /// 响应编码：text（缺省，UTF-8 lossy）| dataUrl（`data:<type>;base64,`）。
    pub encoding: Option<String>,
}

/// 外联出参（两形态共用）：上游状态码透传 + 响应体。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NetFetchResponse {
    pub status: u16,
    pub content_type: String,
    pub body: String,
}

/// URL 安全校验：仅 https + 公网目标（字面量层面；DNS rebinding 由部署形态
/// （单用户本机服务）兜底，不做解析期复核）。
pub(crate) fn validate_public_https_url(raw: &str) -> Result<reqwest::Url, ApiError> {
    let url = reqwest::Url::parse(raw)
        .map_err(|error| ApiError::bad_request("invalid_url", format!("invalid url: {error}")))?;
    if url.scheme() != "https" {
        return Err(ApiError::bad_request("invalid_url", "only https URLs are allowed"));
    }
    let Some(host) = url.host_str() else {
        return Err(ApiError::bad_request("invalid_url", "url has no host"));
    };
    let host = host.trim_matches(['[', ']']);
    if host.eq_ignore_ascii_case("localhost")
        || host.to_ascii_lowercase().ends_with(".localhost")
        || !host.contains('.')
    {
        return Err(ApiError::bad_request(
            "invalid_url",
            "only public internet hosts are allowed",
        ));
    }
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        if !is_public_ip(ip) {
            return Err(ApiError::bad_request(
                "invalid_url",
                "only public internet hosts are allowed",
            ));
        }
    }
    Ok(url)
}

/// IP 字面量是否公网可达（stable 替代 unstable 的 `is_global`：手工枚举私网 /
/// 保留段）。
fn is_public_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            !(v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_documentation()
                || a == 0                         // 0.0.0.0/8「本网络」
                || (a == 100 && (64..=127).contains(&b)) // 100.64/10 CGNAT
                || (a == 198 && (18..=19).contains(&b)) // 198.18/15 基准测试保留
                || a >= 240)                     // 240/4 保留 + 广播
        }
        std::net::IpAddr::V6(v6) => {
            if let Some(mapped) = v6.to_ipv4_mapped() {
                return is_public_ip(std::net::IpAddr::V4(mapped));
            }
            let first = v6.segments()[0];
            !(v6.is_loopback()
                || v6.is_unspecified()
                || (first & 0xfe00) == 0xfe00    // fc00::/7 唯一本地（ULA）
                || (first & 0xffc0) == 0xfe80)   // fe80::/10 链路本地
        }
    }
}

/// 外联客户端：超时 + 重定向逐跳复检公网 https（任一跳不合规即停止跟随）。
fn outbound_client() -> Result<reqwest::Client, ApiError> {
    reqwest::Client::builder()
        .timeout(UPSTREAM_TIMEOUT)
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= MAX_REDIRECTS {
                return attempt.error("too many redirects");
            }
            match validate_public_https_url(attempt.url().as_str()) {
                Ok(_) => attempt.follow(),
                Err(_) => attempt.stop(),
            }
        }))
        .build()
        .map_err(|error| ApiError::internal(format!("build http client failed: {error}")))
}

/// 校验请求方法（仅 GET / POST，大小写归一）。
fn validate_method(method: Option<&str>) -> Result<reqwest::Method, ApiError> {
    match method.unwrap_or("GET").trim().to_ascii_uppercase().as_str() {
        "GET" => Ok(reqwest::Method::GET),
        "POST" => Ok(reqwest::Method::POST),
        other => Err(ApiError::bad_request(
            "invalid_method",
            format!("only GET/POST are allowed, got \"{other}\""),
        )),
    }
}

/// 组装请求头：白名单字段透传（大小写不敏感），其余丢弃。
fn apply_allowed_headers(
    mut builder: reqwest::RequestBuilder,
    headers: Option<&HashMap<String, String>>,
) -> reqwest::RequestBuilder {
    let Some(headers) = headers else {
        return builder;
    };
    for (name, value) in headers {
        if ALLOWED_HEADERS.iter().any(|allowed| allowed.eq_ignore_ascii_case(name)) {
            let Ok(name) = reqwest::header::HeaderName::from_bytes(name.as_bytes()) else {
                continue;
            };
            let Ok(value) = reqwest::header::HeaderValue::from_str(value) else {
                continue;
            };
            builder = builder.header(name, value);
        }
    }
    builder
}

/// 执行外联请求并读响应（分块累计，超限即断）。
async fn execute(
    url: reqwest::Url,
    method: reqwest::Method,
    headers: Option<&HashMap<String, String>>,
    body: Option<&str>,
    encoding: &str,
) -> Result<Json<NetFetchResponse>, ApiError> {
    let mut builder = apply_allowed_headers(outbound_client()?.request(method, url), headers);
    if let Some(body) = body {
        builder = builder.body(body.to_string());
    }
    let mut response = builder
        .send()
        .await
        .map_err(|error| ApiError::internal(format!("outbound fetch failed: {error}")))?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let mut raw: Vec<u8> = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| ApiError::internal(format!("outbound fetch failed: {error}")))?
    {
        if raw.len() + chunk.len() > MAX_BODY_BYTES {
            return Err(ApiError::unsupported_media_type(
                "payload_too_large",
                format!("outbound response exceeds {MAX_BODY_BYTES} bytes limit"),
            ));
        }
        raw.extend_from_slice(&chunk);
    }
    let body = match encoding {
        "dataUrl" => {
            let mime = if content_type.is_empty() { "application/octet-stream" } else { &content_type };
            format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(&raw))
        }
        // text（缺省）：UTF-8 lossy。
        _ => String::from_utf8_lossy(&raw).into_owned(),
    };
    Ok(Json(NetFetchResponse { status, content_type, body }))
}

/// `POST /v1/net/fetch`：通用受控外联转发（统一联网服务主形态）。
pub(crate) async fn fetch_request(
    State(_state): State<RouterState>,
    Json(request): Json<NetFetchRequest>,
) -> Result<Json<NetFetchResponse>, ApiError> {
    let url = validate_public_https_url(request.url.trim())?;
    let method = validate_method(request.method.as_deref())?;
    let encoding = match request.encoding.as_deref() {
        None | Some("text") => "text",
        Some("dataUrl") => "dataUrl",
        Some(other) => {
            return Err(ApiError::bad_request(
                "invalid_encoding",
                format!("only text/dataUrl are allowed, got \"{other}\""),
            ));
        }
    };
    // body 仅 POST 携带（GET 带 body 无意义，丢弃）。
    let body = if method == reqwest::Method::POST { request.body.as_deref() } else { None };
    execute(url, method, request.headers.as_ref(), body, encoding).await
}

/// `GET /v1/net/fetch?url=<encoded>`：只读便捷形态（等价 method=GET）。
pub(crate) async fn fetch(
    State(_state): State<RouterState>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<NetFetchResponse>, ApiError> {
    let raw = query.get("url").map(|value| value.trim()).unwrap_or_default();
    if raw.is_empty() {
        return Err(ApiError::bad_request("url_required", "query parameter `url` is required"));
    }
    let url = validate_public_https_url(raw)?;
    execute(url, reqwest::Method::GET, None, None, "text").await
}

#[cfg(test)]
#[path = "net_tests.rs"]
mod tests;
