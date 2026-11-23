//! 压缩压力配置：阈值派生。
//!
//! 公式：
//! - messageBudgetTokens = contextWindow - reservedCompletionTokens
//! - pressureBudgetTokens = messageBudgetTokens - headroomTokens
//! - thresholdTokens = floor(min(contextWindow * thresholdRatio, pressureBudgetTokens))
//! - retainTokens = floor(messageBudgetTokens * retainRatio)
//! - 校验 retainTokens < thresholdTokens

use crate::config::Config;

/// 默认触发阈值比例。
pub const DEFAULT_THRESHOLD_RATIO: f32 = 0.8;
/// 默认尾部保留比例。
pub const DEFAULT_RETAIN_RATIO: f32 = 0.16;
/// 默认 headroom（为压力预算预留的呼吸空间）。
pub const DEFAULT_HEADROOM_TOKENS: u64 = 65536;
/// 压缩重试上限。
pub const DEFAULT_COMPACT_MAX_RETRIES: u32 = 1;
/// 溢出恢复上限默认（每 step 计数，独立于压缩重试）。
pub const DEFAULT_MAX_OVERFLOW_RETRIES: u32 = 1;

/// 一次压缩派生出的不可变压力预算。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompactSpec {
    pub context_window_tokens: u64,
    pub threshold_tokens: u64,
    pub retain_tokens: u64,
    pub compact_max_retries: u32,
    /// 溢出恢复上限：同一 step 内最多触发的压缩恢复次数。
    pub max_overflow_retries: u32,
}

/// per-model 覆写策略。onion 为单一 OpenAI 兼容目标，
/// 以 model 精确匹配；其余字段缺省时回退全局配置。
#[derive(Clone, Debug, PartialEq)]
pub struct ModelPolicy {
    pub model: String,
    /// 覆写触发阈值比例。
    pub threshold_ratio: Option<f32>,
    /// 覆写尾部保留比例（与 retain_tokens 互斥，retain_tokens 存在时优先）。
    pub retain_ratio: Option<f32>,
    /// 绝对保留 token（优先于 retain_ratio）。
    pub retain_tokens: Option<u64>,
    /// 覆写 headroom（压力预算呼吸空间）。
    pub headroom_tokens: Option<u64>,
    /// 覆写上下文窗口。
    pub context_window_tokens: Option<u64>,
    /// 覆写压缩重试上限。
    pub compact_max_retries: Option<u32>,
    /// 覆写溢出恢复上限（每 step）。
    pub max_overflow_retries: Option<u32>,
}

impl ModelPolicy {
    /// 按 model 精确匹配选策略。
    pub fn matching<'a>(policies: &'a [ModelPolicy], model: &str) -> Option<&'a ModelPolicy> {
        policies.iter().find(|policy| policy.model == model)
    }
}

/// 阈值配置错误。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompactSpecError(pub String);

impl std::fmt::Display for CompactSpecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CompactSpecError {}

/// 由容量与比例派生具体 token 预算（全局配置，无 per-model 覆写）。
pub fn resolve_compact_spec(config: &Config) -> Result<CompactSpec, CompactSpecError> {
    resolve_with_overrides(config, None)
}

/// 全局派生 + per-model 策略匹配一步到位：
/// 装配点按当前路由 model 解析；未命中策略时等价 `resolve_compact_spec`。
pub fn resolve_compact_spec_for(config: &Config, model: &str) -> Result<CompactSpec, CompactSpecError> {
    resolve_with_overrides(config, ModelPolicy::matching(&config.compact_model_policies, model))
}

/// 按 per-model 策略覆写派生：
/// 策略字段优先，缺省回退全局配置；retain_tokens 绝对形态存在时优先于保留比例。
pub fn resolve_with_overrides(
    config: &Config,
    policy: Option<&ModelPolicy>,
) -> Result<CompactSpec, CompactSpecError> {
    let context_window = policy
        .and_then(|policy| policy.context_window_tokens)
        .unwrap_or(config.context_window_tokens);
    let headroom = policy
        .and_then(|policy| policy.headroom_tokens)
        .unwrap_or(config.compact_headroom_tokens);
    let threshold_ratio = policy
        .and_then(|policy| policy.threshold_ratio)
        .unwrap_or(config.compact_threshold_ratio);
    let retain_ratio = policy
        .and_then(|policy| policy.retain_ratio)
        .unwrap_or(config.compact_retain_ratio);
    let compact_max_retries = policy
        .and_then(|policy| policy.compact_max_retries)
        .unwrap_or(config.compact_max_retries);
    if context_window == 0 {
        return Err(CompactSpecError(
            "contextWindowTokens must be a positive integer".to_string(),
        ));
    }
    let message_budget = context_window
        .saturating_sub(config.reserved_completion_tokens);
    if message_budget == 0 {
        return Err(CompactSpecError(
            "reservedCompletionTokens consumes the whole context window; \
             raise contextWindowTokens above the effective request maxTokens"
                .to_string(),
        ));
    }
    let pressure_budget = message_budget.saturating_sub(headroom);
    if pressure_budget == 0 {
        return Err(CompactSpecError(
            "headroomTokens consumes the whole message budget; \
             reduce compactHeadroomTokens or raise contextWindowTokens"
                .to_string(),
        ));
    }
    let threshold_tokens = (context_window as f64 * threshold_ratio as f64)
        .min(pressure_budget as f64)
        .floor() as u64;
    let retain_tokens = match policy.and_then(|policy| policy.retain_tokens) {
        Some(absolute) => absolute,
        None => ((message_budget as f64 * retain_ratio as f64).floor()) as u64,
    };
    if retain_tokens >= threshold_tokens {
        return Err(CompactSpecError(format!(
            "retainTokens ({retain_tokens}) must be less than thresholdTokens ({threshold_tokens})"
        )));
    }
    Ok(CompactSpec {
        context_window_tokens: context_window,
        threshold_tokens,
        retain_tokens,
        compact_max_retries,
        max_overflow_retries: policy
            .and_then(|policy| policy.max_overflow_retries)
            .unwrap_or(DEFAULT_MAX_OVERFLOW_RETRIES),
    })
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
