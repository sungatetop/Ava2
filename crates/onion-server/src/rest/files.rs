//! `GET /v1/workspace/file?path=<相对或权限根内绝对路径>`：工作区只读文件读取。
//!
//! 供控制台预览交付物卡片与文件提及。只读、只允许工作区内**常规文件**：
//! 拒绝控制字符，`canonicalize` 后校验前缀（防符号链接逃逸与 `..` 越界），
//! 与内核 [`onion::session_cwd::SessionCwd`] 一致——根内绝对路径与 `..` 合法。
//! 大小超 [`MAX_FILE_BYTES`] 返回 415。图片与 Office 文档（docx/xlsx/pptx）按
//! 原始字节返回（后者由前端解析库渲染），其余一律按文本（HTML / SVG 也降级为
//! `text/plain`，避免内联执行脚本）。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::rest::error::ApiError;
use crate::router::RouterState;

/// 单文件预览上限（2 MiB）：超出即不再预览。
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

/// 工作区相对路径 → 校验后的绝对路径。
fn resolve_workspace_file(root: &Path, raw: &str) -> Result<PathBuf, ApiError> {
    if raw.trim().is_empty() {
        return Err(ApiError::bad_request("path_required", "query parameter `path` is required"));
    }
    if raw.chars().any(char::is_control) {
        return Err(ApiError::bad_request(
            "path_invalid",
            format!("path `{raw}` contains control characters"),
        ));
    }
    let requested = Path::new(raw);
    let root = root
        .canonicalize()
        .map_err(|error| ApiError::internal(format!("workspace unavailable: {error}")))?;
    let candidate = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        root.join(requested)
    };
    let target = candidate.canonicalize().map_err(|_| {
        ApiError::not_found("file_not_found", format!("file `{raw}` not found"))
    })?;
    if !target.starts_with(&root) {
        return Err(ApiError::bad_request(
            "path_invalid",
            format!("path `{raw}` escapes the workspace"),
        ));
    }
    Ok(target)
}

/// 预览内容类型（未知扩展名按纯文本处理）。
fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "md" | "markdown" => "text/markdown; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        _ => "text/plain; charset=utf-8",
    }
}

/// Office OpenXML 二进制（docx/xlsx/pptx）：原始字节返回，前端解析库渲染。
fn is_office_binary(mime: &str) -> bool {
    mime.starts_with("application/vnd.openxmlformats-officedocument")
}

/// 读取工作区文件（`?session=` 时用该会话权限根，否则兜底工作目录）。
pub(crate) async fn read(
    State(state): State<RouterState>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let raw = query
        .get("path")
        .ok_or_else(|| ApiError::bad_request("path_required", "query parameter `path` is required"))?
        .clone();
    let root = match query.get("session") {
        Some(id) => crate::rest::require_session(&state, id).await?.root.clone(),
        None => state.registry.workspace_root().clone(),
    };
    let target = resolve_workspace_file(&root, &raw)?;
    let metadata = tokio::fs::metadata(&target)
        .await
        .map_err(|_| ApiError::not_found("file_not_found", format!("file `{raw}` not found")))?;
    if !metadata.is_file() {
        return Err(ApiError::bad_request(
            "path_invalid",
            format!("`{raw}` is not a regular file"),
        ));
    }
    if metadata.len() > MAX_FILE_BYTES {
        return Err(ApiError::unsupported_media_type(
            "file_too_large",
            format!("file `{raw}` exceeds {MAX_FILE_BYTES} bytes"),
        ));
    }
    let bytes = tokio::fs::read(&target)
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;
    let mime = content_type(&target);
    let body = if mime.starts_with("image/") || is_office_binary(mime) {
        Body::from(bytes)
    } else {
        match String::from_utf8(bytes) {
            Ok(text) => Body::from(text),
            Err(_) => {
                return Err(ApiError::unsupported_media_type(
                    "file_not_previewable",
                    format!("file `{raw}` is not UTF-8 text or a supported image"),
                ));
            }
        }
    };
    let mut response = (StatusCode::OK, body).into_response();
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(mime));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    Ok(response)
}

#[cfg(test)]
#[path = "files_tests.rs"]
mod tests;
