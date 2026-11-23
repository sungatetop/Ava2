//! LLM 请求退避重试策略（retry-policy + llm-retry normal 模式）。
//!
//! 指数退避 500ms→10s、对称 jitter ±10%、Retry-After 透传优先；可重试分类表
//! EMPTY_RESPONSE / RATE_LIMIT / SERVER / TIMEOUT / TRANSPORT（默认可重试集）。
//! 由 kernel 的 `request-error` 链尾默认决策调用。

use crate::llm::{LlmFailure, LlmFailureCode};

/// 退避初始延迟（500ms）。
pub const RETRY_INITIAL_DELAY_MS: u64 = 500;

/// 退避上限延迟（10s）。
pub const RETRY_MAX_DELAY_MS: u64 = 10_000;

/// 对称 jitter 比例：延迟在 [1-r, 1+r] 倍区间抖动。
pub const RETRY_JITTER_RATIO: f32 = 0.1;

/// 可重试分类判定（默认可重试集）：鉴权 / 配额 / 请求非法 /
/// 上下文溢出 / 其余状态兜底不可重试——上下文溢出走 compaction listener 的
/// 显式恢复决策，不进退避表。
pub fn is_retryable(code: LlmFailureCode) -> bool {
    matches!(
        code,
        LlmFailureCode::EmptyResponse
            | LlmFailureCode::RateLimit
            | LlmFailureCode::Server
            | LlmFailureCode::Timeout
            | LlmFailureCode::Transport
    )
}

/// 指数退避 + 对称 jitter。`retry` 从 1 计：
/// `initial × 2^(retry-1)` 封顶 max_delay，乘 `[1-ratio, 1+ratio]` 区间 jitter
/// （`random` ∈ [0, 1]，测试注入固定值保证确定性），结果封顶 max_delay。
pub fn local_delay(retry: u32, random: f32) -> u64 {
    let exponent = retry.saturating_sub(1).min(1024);
    let mut exponential = RETRY_INITIAL_DELAY_MS;
    for _ in 0..exponent {
        exponential = exponential.saturating_mul(2);
    }
    let exponential = exponential.min(RETRY_MAX_DELAY_MS);
    let jitter = 1.0 - RETRY_JITTER_RATIO + 2.0 * RETRY_JITTER_RATIO * random.clamp(0.0, 1.0);
    // 四舍五入到毫秒：f32 直接截断会在 450 这类边界引入 off-by-one。
    ((exponential as f32 * jitter).round() as u64).min(RETRY_MAX_DELAY_MS)
}

/// 单次重试延迟决策（normal 模式）：
/// Retry-After 透传优先——有效值不超过上限时直接采用，超过上限视为放弃重试
/// （返回 None）；无 Retry-After 时走指数退避。可重试性由调用方先行判定。
pub fn resolve_retry_delay(failure: Option<&LlmFailure>, retry: u32, random: f32) -> Option<u64> {
    if let Some(after) = failure.and_then(|failure| failure.retry_after_ms) {
        if after > RETRY_MAX_DELAY_MS {
            return None;
        }
        return Some(after);
    }
    Some(local_delay(retry, random))
}

/// 轻量伪随机采样（0..1）：jitter 为非密码学场景，时间纳秒取模即可。
pub fn sample_random() -> f32 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    (nanos % 1000) as f32 / 1000.0
}

#[cfg(test)]
#[path = "retry_tests.rs"]
mod tests;
