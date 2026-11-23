//! `error` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;

/// 把错误落成响应后读回 `(status, JSON body)`。
async fn body_of(error: ApiError) -> (StatusCode, serde_json::Value) {
    let response = error.into_response();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body");
    (status, serde_json::from_slice(&bytes).expect("json body"))
}

#[tokio::test]
async fn unauthorized_shape_is_stable() {
    let (status, body) = body_of(ApiError::unauthorized()).await;
    assert_eq!(StatusCode::UNAUTHORIZED, status);
    assert_eq!(
        serde_json::json!({
            "error": {
                "code": "unauthorized",
                "message": "missing or invalid access token",
            },
        }),
        body
    );
}

#[tokio::test]
async fn constructors_map_status_and_code() {
    let cases = [
        (
            ApiError::bad_request("invalid_decision", "bad decision"),
            StatusCode::BAD_REQUEST,
            "invalid_decision",
        ),
        (
            ApiError::not_found("request_not_found", "no such request"),
            StatusCode::NOT_FOUND,
            "request_not_found",
        ),
        (
            ApiError::conflict("already_resolved", "resolved"),
            StatusCode::CONFLICT,
            "already_resolved",
        ),
        (
            ApiError::internal("boom"),
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
        ),
        (
            ApiError::session_not_found("s1"),
            StatusCode::NOT_FOUND,
            "session_not_found",
        ),
    ];
    for (error, status, code) in cases {
        let (actual_status, body) = body_of(error).await;
        assert_eq!(status, actual_status);
        assert_eq!(code, body["error"]["code"].as_str().expect("code"));
    }
}

#[tokio::test]
async fn session_not_found_message_names_session() {
    let (_, body) = body_of(ApiError::session_not_found("abc")).await;
    assert_eq!(
        "session `abc` not found",
        body["error"]["message"].as_str().expect("message")
    );
}
