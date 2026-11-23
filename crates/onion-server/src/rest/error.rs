//! REST 统一错误模型：`{"error":{"code","message"}}`。
//!
//! 所有 `/v1` handler 统一返回 [`ApiError`]，由 `IntoResponse` 落为
//! `application/json`；`code` 为稳定机器标签，`message` 供人阅读。

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

/// REST 错误（status + 稳定 code + 人类可读消息）。
#[derive(Clone, Debug)]
pub(crate) struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

#[derive(Serialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: &'static str,
    message: String,
}

impl ApiError {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self { status, code, message: message.into() }
    }

    /// 稳定错误码（仅供测试断言错误类别，生产侧不透出）。
    #[cfg(test)]
    pub(crate) fn code(&self) -> &'static str {
        self.code
    }

    /// 401：缺失 / 错误的访问令牌。
    pub(crate) fn unauthorized() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "missing or invalid access token",
        )
    }

    /// 400：请求体 / 参数非法。
    pub(crate) fn bad_request(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, message)
    }

    /// 404：资源不存在。
    pub(crate) fn not_found(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, code, message)
    }

    /// 409：状态冲突（重复回填 / 已结束）。
    pub(crate) fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, code, message)
    }

    /// 415：媒体类型不可预览（过大 / 非文本非图片）。
    pub(crate) fn unsupported_media_type(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNSUPPORTED_MEDIA_TYPE, code, message)
    }

    /// 500：内部失败。
    pub(crate) fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", message)
    }

    /// 会话不存在（统一 404 文案）。
    pub(crate) fn session_not_found(id: &str) -> Self {
        Self::not_found("session_not_found", format!("session `{id}` not found"))
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorBody {
            error: ErrorDetail { code: self.code, message: self.message },
        };
        (self.status, Json(body)).into_response()
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
