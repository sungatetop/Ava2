//! `retry` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;
use crate::llm::{LlmFailure, LlmFailureCode};

fn failure(code: LlmFailureCode, retry_after_ms: Option<u64>) -> LlmFailure {
    LlmFailure { code, message: "测试失败".into(), retry_after_ms }
}

#[test]
fn retryable_codes_match_default_set() {
    assert!(is_retryable(LlmFailureCode::EmptyResponse));
    assert!(is_retryable(LlmFailureCode::RateLimit));
    assert!(is_retryable(LlmFailureCode::Server));
    assert!(is_retryable(LlmFailureCode::Timeout));
    assert!(is_retryable(LlmFailureCode::Transport));
    assert!(!is_retryable(LlmFailureCode::Auth));
    assert!(!is_retryable(LlmFailureCode::Quota));
    assert!(!is_retryable(LlmFailureCode::InvalidRequest));
    assert!(!is_retryable(LlmFailureCode::ContextWindowExceeded));
    assert!(!is_retryable(LlmFailureCode::Http(418)));
}

#[test]
fn local_delay_grows_exponentially_and_caps() {
    // random=0.5 → jitter 恰为 1.0：各档指数值不抖动。
    assert_eq!(local_delay(1, 0.5), 500);
    assert_eq!(local_delay(2, 0.5), 1_000);
    assert_eq!(local_delay(3, 0.5), 2_000);
    assert_eq!(local_delay(5, 0.5), 8_000);
    assert_eq!(local_delay(6, 0.5), 10_000, "封顶 max_delay");
    assert_eq!(local_delay(40, 0.5), 10_000);
}

#[test]
fn local_delay_jitters_symmetrically() {
    // ratio 0.1：下界 500×0.9=450，上界 500×1.1=550。
    assert_eq!(local_delay(1, 0.0), 450);
    assert_eq!(local_delay(1, 1.0), 550);
}

#[test]
fn resolve_retry_delay_prefers_retry_after_within_cap() {
    let value = failure(LlmFailureCode::RateLimit, Some(2_500));
    assert_eq!(resolve_retry_delay(Some(&value), 1, 0.5), Some(2_500));
}

#[test]
fn resolve_retry_delay_over_cap_gives_up() {
    let value = failure(LlmFailureCode::RateLimit, Some(20_000));
    assert_eq!(resolve_retry_delay(Some(&value), 1, 0.5), None);
}

#[test]
fn resolve_retry_delay_falls_back_to_backoff_without_retry_after() {
    let value = failure(LlmFailureCode::Server, None);
    assert_eq!(resolve_retry_delay(Some(&value), 2, 0.5), Some(1_000));
    assert_eq!(resolve_retry_delay(None, 2, 0.5), Some(1_000));
}
