//! `config` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

fn base_config() -> crate::config::Config {
    crate::config::Config::from_env()
}

#[test]
fn resolves_default_spec_from_env_config() {
    let config = base_config();
    let spec = super::resolve_compact_spec(&config).expect("default config must resolve");
    // 128000 窗口，2048 预留，65536 headroom。
    // messageBudget = 128000 - 2048 = 125952
    // pressureBudget = 125952 - 65536 = 60416
    // threshold = floor(min(128000*0.8=102400, 60416)) = 60416
    // retain = floor(125952*0.16) = 20152
    assert_eq!(spec.context_window_tokens, 128_000);
    assert_eq!(spec.threshold_tokens, 60_416);
    assert_eq!(spec.retain_tokens, 20_152);
    assert_eq!(spec.compact_max_retries, 1);
}

#[test]
fn rejects_zero_context_window() {
    let mut config = base_config();
    config.context_window_tokens = 0;
    assert!(super::resolve_compact_spec(&config).is_err());
}

#[test]
fn rejects_retain_at_or_above_threshold() {
    let mut config = base_config();
    config.compact_retain_ratio = 0.9;
    config.compact_threshold_ratio = 0.5;
    assert!(super::resolve_compact_spec(&config).is_err());
}

#[test]
fn threshold_capped_by_pressure_budget_not_ratio() {
    let mut config = base_config();
    config.context_window_tokens = 100_000;
    config.compact_threshold_ratio = 0.8;
    config.reserved_completion_tokens = 4_000;
    config.compact_headroom_tokens = 40_000;
    // messageBudget = 96000; pressureBudget = 56000; ratio*window = 80000
    // threshold = floor(min(80000, 56000)) = 56000
    let spec = super::resolve_compact_spec(&config).expect("must resolve");
    assert_eq!(spec.threshold_tokens, 56_000);
}

#[test]
fn retain_scales_message_budget_before_headroom() {
    let mut config = base_config();
    config.context_window_tokens = 100_000;
    config.reserved_completion_tokens = 4_000;
    config.compact_headroom_tokens = 40_000;
    config.compact_retain_ratio = 0.2;
    // retain = floor(96000 * 0.2) = 19200
    let spec = super::resolve_compact_spec(&config).expect("must resolve");
    assert_eq!(spec.retain_tokens, 19_200);
}

/// 构造最小策略，其余字段经闭包覆写。
fn policy(model: &str, apply: impl FnOnce(&mut super::ModelPolicy)) -> super::ModelPolicy {
    let mut policy = super::ModelPolicy {
        model: model.to_string(),
        threshold_ratio: None,
        retain_ratio: None,
        retain_tokens: None,
        headroom_tokens: None,
        context_window_tokens: None,
        compact_max_retries: None,
        max_overflow_retries: None,
    };
    apply(&mut policy);
    policy
}

#[test]
fn policy_overrides_take_effect_and_rest_falls_back_to_global() {
    let config = base_config();
    let overrides = policy("local-model", |policy| {
        policy.context_window_tokens = Some(100_000);
        policy.threshold_ratio = Some(0.5);
        policy.compact_max_retries = Some(3);
        policy.max_overflow_retries = Some(2);
    });
    let spec = super::resolve_with_overrides(&config, Some(&overrides)).expect("must resolve");
    // 覆写字段生效：messageBudget = 100000-2048 = 97952；pressureBudget =
    // 97952-65536 = 32416；threshold = floor(min(50000, 32416)) = 32416。
    assert_eq!(spec.context_window_tokens, 100_000);
    assert_eq!(spec.threshold_tokens, 32_416);
    assert_eq!(spec.compact_max_retries, 3);
    assert_eq!(spec.max_overflow_retries, 2);
    // 未覆写字段回退全局：retain = floor(97952 * 0.16) = 15672。
    assert_eq!(spec.retain_tokens, 15_672);
}

#[test]
fn retain_tokens_absolute_wins_over_ratio() {
    let config = base_config();
    let overrides = policy("local-model", |policy| {
        policy.retain_tokens = Some(1_000);
    });
    let spec = super::resolve_with_overrides(&config, Some(&overrides)).expect("must resolve");
    assert_eq!(spec.retain_tokens, 1_000);
}

#[test]
fn empty_policy_resolves_to_global_spec() {
    let config = base_config();
    let overrides = policy("local-model", |_| {});
    let spec = super::resolve_with_overrides(&config, Some(&overrides)).expect("must resolve");
    // 整体对象相等：全缺省策略派生 = 全局派生（含 max_overflow_retries 默认）。
    assert_eq!(spec, super::resolve_compact_spec(&config).expect("must resolve"));
    assert_eq!(spec.max_overflow_retries, super::DEFAULT_MAX_OVERFLOW_RETRIES);
}

#[test]
fn matching_selects_policy_by_exact_model() {
    let policies = vec![policy("model-a", |_| {}), policy("model-b", |_| {})];
    assert_eq!(
        super::ModelPolicy::matching(&policies, "model-b").map(|policy| policy.model.as_str()),
        Some("model-b")
    );
    assert!(super::ModelPolicy::matching(&policies, "model-c").is_none());
}

#[test]
fn resolve_for_applies_matching_policy() {
    let mut config = base_config();
    config.context_window_tokens = 200_000;
    config.compact_model_policies = vec![policy("fake", |policy| {
        policy.context_window_tokens = Some(100_000);
        policy.max_overflow_retries = Some(2);
    })];
    let spec =
        super::resolve_compact_spec_for(&config, "fake").expect("must resolve");
    // 命中策略：窗口按策略 100000 派生（threshold = 60416 同默认派生）。
    assert_eq!(spec.context_window_tokens, 100_000);
    assert_eq!(spec.max_overflow_retries, 2);
}

#[test]
fn resolve_for_without_match_falls_back_to_global() {
    let config = base_config();
    let spec = super::resolve_compact_spec_for(&config, "unknown-model")
        .expect("must resolve");
    // 整体对象相等：未命中策略时等价全局派生。
    assert_eq!(spec, super::resolve_compact_spec(&config).expect("must resolve"));
}
