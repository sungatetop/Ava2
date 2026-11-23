//! 模型管理写面：供应商 / 模型 CRUD、默认模型、连接测试。
//!
//! 持久层是 [`onion::provider_catalog::ProviderCatalog`]（`<data_home>/
//! providers.json`），每次请求 load → 变更 → save（低频管理操作，无需缓存）。
//! 密钥沿用 onion 现有语义内联存储于 providers.json，读面（GET /v1/models）
//! 一律不回显；测试连接经 OpenAI 兼容 `/chat/completions`（max_tokens 收敛）。

use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;

use crate::rest::error::ApiError;
use crate::router::RouterState;

use onion::provider_catalog::ProviderCatalog;

/// 从 data_home 加载目录（`config.model` 作为 env 兜底默认）。
fn load_catalog(state: &RouterState) -> ProviderCatalog {
    ProviderCatalog::load(&onion::paths::data_home(), &state.config.model)
}

/// save 失败统一 500。
fn save_catalog(catalog: &ProviderCatalog) -> Result<(), ApiError> {
    catalog
        .save(&onion::paths::data_home())
        .map(|_| ())
        .map_err(|error| ApiError::internal(format!("providers.json 写入失败: {error}")))
}

// ---------------- 模型 ----------------

/// `POST /v1/models` 请求体：向供应商追加模型；供应商不存在时须携带
/// `base_url` + `api_key`（顺带创建供应商，语义对齐 catalog::add_model）。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AddModelRequest {
    pub provider: String,
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
}

/// `POST /v1/models`：新增（或替换同 id）模型。
pub(crate) async fn add_model(
    State(state): State<RouterState>,
    Json(body): Json<AddModelRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let id = body.id.trim();
    let provider = body.provider.trim();
    if id.is_empty() || provider.is_empty() {
        return Err(ApiError::bad_request(
            "model_fields_missing",
            "`provider` 与 `id` 均不能为空",
        ));
    }
    let mut catalog = load_catalog(&state);
    let known = catalog.providers.iter().any(|p| p.id == provider);
    if !known {
        let Some(base_url) = body.base_url.as_deref().map(str::trim).filter(|v| !v.is_empty()) else {
            return Err(ApiError::bad_request(
                "provider_fields_missing",
                format!("供应商 `{provider}` 不存在：新增供应商需携带 `baseUrl` 与 `apiKey`"),
            ));
        };
        let Some(api_key) = body.api_key.as_deref().map(str::trim).filter(|v| !v.is_empty()) else {
            return Err(ApiError::bad_request(
                "provider_fields_missing",
                format!("供应商 `{provider}` 不存在：新增供应商需携带 `baseUrl` 与 `apiKey`"),
            ));
        };
        catalog.add_provider(provider.to_string(), provider.to_string(), base_url.to_string(), api_key.to_string());
    }
    let name = body
        .name
        .clone()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| id.to_string());
    catalog.add_model(provider, id.to_string(), name, body.description);
    save_catalog(&catalog)?;
    Ok(Json(serde_json::json!({ "provider": provider, "id": id })))
}

/// `DELETE /v1/models/{provider}/{model}`：移除模型。默认模型不可删（先切换）。
pub(crate) async fn remove_model(
    State(state): State<RouterState>,
    Path((provider, model)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut catalog = load_catalog(&state);
    if catalog.default_model == model {
        return Err(ApiError::conflict(
            "model_is_default",
            format!("`{model}` 是当前默认模型，请先切换默认模型再删除"),
        ));
    }
    if !catalog.remove_model(&provider, &model) {
        return Err(ApiError::not_found(
            "model_not_found",
            format!("模型 `{provider}/{model}` 不存在"),
        ));
    }
    save_catalog(&catalog)?;
    Ok(Json(serde_json::json!({ "provider": provider, "id": model })))
}

/// `PUT /v1/models/default` 请求体。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetDefaultRequest {
    pub model: String,
}

/// `PUT /v1/models/default`：设置默认模型（写 providers.json 显式声明，
/// 仅影响之后装配的新会话；已有会话用会话内 /model 切换）。
pub(crate) async fn set_default(
    State(state): State<RouterState>,
    Json(body): Json<SetDefaultRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let model = body.model.trim();
    let mut catalog = load_catalog(&state);
    if !catalog.contains_model(model) {
        return Err(ApiError::bad_request(
            "unknown_model",
            format!("模型 `{model}` 不在供应商目录中"),
        ));
    }
    catalog.set_default_model(model);
    save_catalog(&catalog)?;
    Ok(Json(serde_json::json!({ "defaultModel": catalog.default_model })))
}

// ---------------- 供应商 ----------------

/// `POST /v1/providers` 请求体：新增（或替换同 id）供应商。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AddProviderRequest {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    pub base_url: String,
    pub api_key: String,
}

/// `POST /v1/providers`。
pub(crate) async fn add_provider(
    State(state): State<RouterState>,
    Json(body): Json<AddProviderRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let id = body.id.trim();
    if id.is_empty() {
        return Err(ApiError::bad_request("provider_id_missing", "`id` 不能为空"));
    }
    let base_url = body.base_url.trim();
    if base_url.is_empty() {
        return Err(ApiError::bad_request("provider_fields_missing", "`baseUrl` 不能为空"));
    }
    let name = body
        .name
        .clone()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| id.to_string());
    let mut catalog = load_catalog(&state);
    catalog.add_provider(id.to_string(), name, base_url.to_string(), body.api_key.clone());
    save_catalog(&catalog)?;
    Ok(Json(serde_json::json!({ "id": id })))
}

/// `DELETE /v1/providers/{id}`：移除供应商；组内仍有模型时 409（先清空模型）。
pub(crate) async fn remove_provider(
    State(state): State<RouterState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut catalog = load_catalog(&state);
    let removed = catalog
        .remove_provider(&id)
        .ok_or_else(|| ApiError::not_found("provider_not_found", format!("供应商 `{id}` 不存在")))?;
    if !removed.models.is_empty() {
        return Err(ApiError::conflict(
            "provider_not_empty",
            format!("供应商 `{id}` 下仍有 {} 个模型，请先移除", removed.models.len()),
        ));
    }
    save_catalog(&catalog)?;
    Ok(Json(serde_json::json!({ "id": id })))
}

// ---------------- 连接测试 ----------------

/// `POST /v1/models/test` 请求体：`provider` + `model` 时从目录解析目标
///（密钥不回传前端）；否则须直接携带 `baseUrl` + `apiKey` + `model`。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TestModelRequest {
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
}

/// `POST /v1/models/test` 响应体。
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TestModelResult {
    pub ok: bool,
    pub latency_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 连接测试：OpenAI 兼容 `POST {base_url}/chat/completions`，`max_tokens=8`
/// 收敛探测开销；10s 超时。
async fn probe(base_url: &str, api_key: &str, model: &str) -> TestModelResult {
    let started = std::time::Instant::now();
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return TestModelResult { ok: false, latency_ms: 0, error: Some(error.to_string()) };
        }
    };
    let result = client
        .post(url)
        .bearer_auth(api_key)
        .json(&serde_json::json!({
            "model": model,
            "messages": [{ "role": "user", "content": "ping" }],
            "max_tokens": 8,
        }))
        .send()
        .await;
    let latency = started.elapsed().as_millis() as u64;
    match result {
        Ok(resp) if resp.status().is_success() => TestModelResult { ok: true, latency_ms: latency, error: None },
        Ok(resp) => {
            let status = resp.status();
            let detail = resp.text().await.unwrap_or_default();
            TestModelResult {
                ok: false,
                latency_ms: latency,
                error: Some(format!("HTTP {status}: {}", truncate(&detail, 300))),
            }
        }
        Err(error) => TestModelResult { ok: false, latency_ms: latency, error: Some(error.to_string()) },
    }
}

/// 错误详情截断（测试连接响应不回传完整上游报文）。
fn truncate(raw: &str, max: usize) -> String {
    if raw.chars().count() <= max {
        raw.to_string()
    } else {
        raw.chars().take(max).collect()
    }
}

/// `POST /v1/models/test`。
pub(crate) async fn test_model(
    State(state): State<RouterState>,
    Json(body): Json<TestModelRequest>,
) -> Result<Json<TestModelResult>, ApiError> {
    // 目录解析路径：provider+model → ModelTarget（密钥不出边车）。
    let target: (String, String, String) = match (
        body.provider.as_deref().map(str::trim),
        body.model.as_deref().map(str::trim),
    ) {
        (Some(provider), Some(model)) if !provider.is_empty() && !model.is_empty() => {
            let catalog = load_catalog(&state);
            let entry = catalog.providers.iter().find(|p| p.id == provider).ok_or_else(|| {
                ApiError::not_found("provider_not_found", format!("供应商 `{provider}` 不存在"))
            })?;
            let model_entry = entry.models.iter().find(|m| m.id == model).ok_or_else(|| {
                ApiError::not_found("model_not_found", format!("模型 `{provider}/{model}` 不存在"))
            })?;
            (entry.base_url.clone(), entry.api_key.clone(), model_entry.id.clone())
        }
        _ => {
            // 直连路径：body 自带目标（表单未保存前的测试）。
            let base_url = body.base_url.as_deref().map(str::trim).filter(|v| !v.is_empty());
            let api_key = body.api_key.as_deref().filter(|v| !v.is_empty());
            let model = body.model.as_deref().map(str::trim).filter(|v| !v.is_empty());
            match (base_url, api_key, model) {
                (Some(base_url), Some(api_key), Some(model)) => {
                    (base_url.to_string(), api_key.to_string(), model.to_string())
                }
                _ => {
                    return Err(ApiError::bad_request(
                        "test_fields_missing",
                        "需携带 `provider`+`model`，或 `baseUrl`+`apiKey`+`model`",
                    ));
                }
            }
        }
    };
    let (base_url, api_key, model) = target;
    Ok(Json(probe(&base_url, &api_key, &model).await))
}
