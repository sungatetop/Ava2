//! `tools/subagent.rs` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//! 覆盖 `list_subagent_models` 的纯计算函数（provider 列表 / 模型列表 / 精确检查）。

use super::{agent_key, parse_delegation, parse_wait_args, resolve_preset, ListSubagentModelsTool};
use crate::provider_catalog::{EffortEntry, ModelEntry, ProviderCatalog, ProviderEntry, ReasoningMeta};
use crate::subagent::model_selection::AllowedModelRoute;
use serde_json::json;

/// 构造含两个 provider、带条目级 effort 的测试目录。
fn catalog() -> ProviderCatalog {
    let mut cat = ProviderCatalog::builtin("Qwen3.8-27B");
    cat.providers = vec![
            ProviderEntry {
                id: "local".to_string(),
                name: "local".to_string(),
                base_url: "http://example/v1".to_string(),
                api_key: "k".to_string(),
                models: vec![
                    ModelEntry {
                        id: "Qwen3.8-27B".to_string(),
                        name: "Qwen3.8-27B".to_string(),
                        description: Some("缺省模型".to_string()),
                        reasoning: Some(ReasoningMeta {
                            efforts: vec![
                                EffortEntry {
                                    id: "low".to_string(),
                                    name: "Low".to_string(),
                                    description: None,
                                },
                                EffortEntry {
                                    id: "high".to_string(),
                                    name: "High".to_string(),
                                    description: Some("高推理".to_string()),
                                },
                            ],
                            default_effort: Some("high".to_string()),
                        }),
                    },
                    ModelEntry {
                        id: "DeepSeek-V4-Flash-0731".to_string(),
                        name: "DeepSeek-V4-Flash-0731".to_string(),
                        description: None,
                        reasoning: None,
                    },
                ],
            },
            ProviderEntry {
                id: "empty".to_string(),
                name: "Empty".to_string(),
                base_url: "http://empty/v1".to_string(),
                api_key: "k".to_string(),
                models: vec![],
            },
    ];
    cat
}

fn parent_route() -> AllowedModelRoute {
    AllowedModelRoute::new("local", "Qwen3.8-27B")
}

#[test]
fn lists_all_providers_with_no_args() {
    let text = ListSubagentModelsTool::list_providers(&catalog(), None, &parent_route());
    assert_eq!(text, "local — local");
}

#[test]
fn lists_only_authorized_providers_when_policy_present() {
    let policy = vec![AllowedModelRoute::new("empty", "x")];
    let text = ListSubagentModelsTool::list_providers(&catalog(), Some(&policy), &parent_route());
    assert_eq!(text, "empty — Empty");
}

#[test]
fn lists_placeholder_when_no_providers() {
    let mut empty = ProviderCatalog::builtin("m");
    empty.providers = vec![];
    assert_eq!(
        ListSubagentModelsTool::list_providers(&empty, None, &parent_route()),
        "(no LLM providers)"
    );
}

#[test]
fn find_provider_ok_and_diagnostic() {
    let catalog = catalog();
    let entry = ListSubagentModelsTool::find_provider(&catalog, "local").unwrap();
    assert_eq!(entry.id, "local");
    let err = ListSubagentModelsTool::find_provider(&catalog, "nope").unwrap_err();
    assert_eq!(
        err,
        "LLM provider \"nope\" is not registered; available providers: local, empty"
    );
}

#[test]
fn list_models_renders_with_and_without_description() {
    let catalog = catalog();
    let entry = ListSubagentModelsTool::find_provider(&catalog, "local").unwrap();
    let policy = vec![
        AllowedModelRoute::new("local", "Qwen3.8-27B"),
        AllowedModelRoute::new("local", "DeepSeek-V4-Flash-0731"),
    ];
    let text = ListSubagentModelsTool::list_models(entry, Some(&policy), &parent_route());
    assert!(text.contains("local/Qwen3.8-27B — Qwen3.8-27B: 缺省模型"));
    assert!(text.contains("local/DeepSeek-V4-Flash-0731 — DeepSeek-V4-Flash-0731"));
}

#[test]
fn list_models_without_policy_shows_only_parent_route() {
    let catalog = catalog();
    let entry = ListSubagentModelsTool::find_provider(&catalog, "local").unwrap();
    let text = ListSubagentModelsTool::list_models(entry, None, &parent_route());
    assert_eq!(text, "local/Qwen3.8-27B — Qwen3.8-27B: 缺省模型");
}

#[test]
fn list_models_placeholder_when_empty() {
    let catalog = catalog();
    let entry = ListSubagentModelsTool::find_provider(&catalog, "empty").unwrap();
    assert_eq!(
        ListSubagentModelsTool::list_models(entry, None, &parent_route()),
        "(no advertised models for empty)"
    );
}

#[test]
fn inspect_model_renders_default_effort_marker() {
    let catalog = catalog();
    let entry = ListSubagentModelsTool::find_provider(&catalog, "local").unwrap();
    let text = ListSubagentModelsTool::inspect_model(entry, "Qwen3.8-27B").unwrap();
    assert!(text.contains("local/Qwen3.8-27B — Qwen3.8-27B: 缺省模型"));
    assert!(text.contains("Reasoning efforts:"));
    assert!(text.contains("low — Low"));
    assert!(text.contains("high (default) — High: 高推理"));
}

#[test]
fn inspect_model_rejects_unlisted_route() {
    let catalog = catalog();
    let entry = ListSubagentModelsTool::find_provider(&catalog, "local").unwrap();
    let err = ListSubagentModelsTool::inspect_model(entry, "ghost").unwrap_err();
    assert_eq!(err, "child LLM route \"local/ghost\" is not allowed for this Session");
}

#[test]
fn inspect_model_no_reasoning_efforts() {
    let catalog = catalog();
    let entry = ListSubagentModelsTool::find_provider(&catalog, "local").unwrap();
    let text = ListSubagentModelsTool::inspect_model(entry, "DeepSeek-V4-Flash-0731").unwrap();
    assert!(text.contains("Reasoning efforts:\n(no advertised reasoning efforts)"));
}

#[test]
fn preset_omitted_inherits_parent() {
    let choices = vec!["coding".to_string(), "work".to_string()];
    assert_eq!(resolve_preset(None, &choices).unwrap(), None);
    // 白名单为空时省略同样合法（继承父会话预设）。
    assert_eq!(resolve_preset(None, &[]).unwrap(), None);
}

#[test]
fn preset_accepts_whitelisted_id() {
    let choices = vec!["coding".to_string(), "work".to_string()];
    assert_eq!(
        resolve_preset(Some("work"), &choices).unwrap(),
        Some("work".to_string())
    );
}

#[test]
fn preset_disabled_when_no_choices() {
    let err = resolve_preset(Some("coding"), &[]).unwrap_err();
    assert_eq!(
        err,
        "agent_preset selection is disabled for this tool instance (omit agent_preset)"
    );
}

#[test]
fn preset_rejects_unknown_id_with_choices_list() {
    let choices = vec!["coding".to_string(), "work".to_string()];
    let err = resolve_preset(Some("ghost"), &choices).unwrap_err();
    assert_eq!(err, "agent_preset must be one of: coding, work");
}

#[test]
fn parse_delegation_carries_preset_and_cwd() {
    let choices = vec!["coding".to_string(), "creator".to_string()];
    let input = json!({ "prompt": "do it", "agent_preset": "creator", "cwd": "sub" });
    let (prompt, options) =
        parse_delegation(&input, &catalog(), None, &parent_route(), &choices).unwrap();
    assert_eq!(prompt, "do it");
    assert_eq!(options.preset, Some("creator".to_string()));
    assert_eq!(options.cwd, Some("sub".to_string()));
    assert!(options.route.is_none());
}

#[test]
fn parse_delegation_preset_requires_prompt() {
    let choices = vec!["coding".to_string()];
    let err = parse_delegation(
        &json!({ "agent_preset": "coding" }),
        &catalog(),
        None,
        &parent_route(),
        &choices,
    )
    .unwrap_err();
    assert_eq!(err, "prompt is required");
}

#[test]
fn parse_delegation_rejects_unknown_preset() {
    let choices = vec!["coding".to_string(), "work".to_string()];
    let input = json!({ "prompt": "x", "agent_preset": "ghost" });
    let err = parse_delegation(&input, &catalog(), None, &parent_route(), &choices).unwrap_err();
    assert_eq!(err, "agent_preset must be one of: coding, work");
}

#[test]
fn parse_delegation_route_limited_to_parent_without_policy() {
    let choices: Vec<String> = Vec::new();
    // 父路由本身允许。
    let input = json!({ "prompt": "x", "provider": "local", "model": "Qwen3.8-27B" });
    let (_, options) =
        parse_delegation(&input, &catalog(), None, &parent_route(), &choices).unwrap();
    assert_eq!(options.route.map(|r| r.model), Some("Qwen3.8-27B".to_string()));
    // 目录内但非父路由 → 拒绝（无白名单时只允许父路由）。
    let input = json!({ "prompt": "x", "provider": "local", "model": "DeepSeek-V4-Flash-0731" });
    let err = parse_delegation(&input, &catalog(), None, &parent_route(), &choices).unwrap_err();
    assert_eq!(
        err,
        "child LLM route \"local/DeepSeek-V4-Flash-0731\" is not allowed for this Session; \
         without a recorded model-selection policy, explicit selection is limited to the parent route"
    );
}

#[test]
fn parse_delegation_route_uses_policy_allowlist() {
    let choices: Vec<String> = Vec::new();
    let policy = vec![AllowedModelRoute::new("local", "DeepSeek-V4-Flash-0731")];
    let input = json!({ "prompt": "x", "provider": "local", "model": "DeepSeek-V4-Flash-0731" });
    let (_, options) =
        parse_delegation(&input, &catalog(), Some(&policy), &parent_route(), &choices).unwrap();
    assert_eq!(
        options.route.map(|r| r.model),
        Some("DeepSeek-V4-Flash-0731".to_string())
    );
    // 白名单存在时父路由不在列表内同样被拒。
    let input = json!({ "prompt": "x", "provider": "local", "model": "Qwen3.8-27B" });
    let err =
        parse_delegation(&input, &catalog(), Some(&policy), &parent_route(), &choices).unwrap_err();
    assert_eq!(
        err,
        "child LLM route \"local/Qwen3.8-27B\" is not allowed for this Session"
    );
}

/// 路由预检（#12）：不可达的 base_url 必须在创建子会话前失败，且错误带模型名。
#[tokio::test]
async fn preflight_route_rejects_unreachable_base_url() {
    let target = crate::model_target::ModelTarget {
        model: "ghost-model".to_string(),
        // 保留端口 1：本机必然拒绝连接（真实网络调用，非 mock）。
        base_url: "http://127.0.0.1:1/v1".to_string(),
        api_key: "k".to_string(),
        effort: None,
    };
    let err = super::preflight_route(&target).await.unwrap_err();
    assert!(err.contains("ghost-model"), "错误应带模型名：{err}");
    assert!(err.contains("preflight failed"), "错误应标明预检失败：{err}");
}

/// wait_agent 入参校验：省略取默认 30s。
#[test]
fn wait_args_default_when_omitted() {
    assert_eq!(parse_wait_args(&json!({})).unwrap(), 30_000);
    assert_eq!(parse_wait_args(&json!({ "timeout_ms": null })).unwrap(), 30_000);
}

/// 边界内取值原样通过（10s 与 3600s 均在闭区间内）。
#[test]
fn wait_args_accepts_inclusive_bounds() {
    assert_eq!(parse_wait_args(&json!({ "timeout_ms": 10_000 })).unwrap(), 10_000);
    assert_eq!(parse_wait_args(&json!({ "timeout_ms": 3_600_000 })).unwrap(), 3_600_000);
}

/// 越界 / 非整数一律报错（严格校验，不夹取）。
#[test]
fn wait_args_rejects_out_of_range() {
    for value in [json!({ "timeout_ms": 9_999 }), json!({ "timeout_ms": 3_600_001 }), json!({ "timeout_ms": "30000" })] {
        let err = parse_wait_args(&value).expect_err("越界/非整数必须报错");
        assert!(err.contains("timeout_ms must be an integer"), "错误文案不符：{err}");
    }
}

/// 声明外字段必须报错（`additionalProperties: false`）：模型误带 `agent`
/// 时不能静默忽略，否则「等谁」的语义会被误解。
#[test]
fn wait_args_rejects_unknown_fields() {
    let err = parse_wait_args(&json!({ "agent": "docs-scout" })).expect_err("未知字段必须报错");
    assert!(err.contains("unexpected argument `agent`"), "错误文案不符：{err}");
}

/// 寻址键：`agent`（模型可读名）优先，回落历史字段 `agent_id`。
#[test]
fn agent_key_prefers_name_and_falls_back_to_id() {
    assert_eq!(agent_key(&json!({ "agent": "docs-scout" })).as_deref(), Some("docs-scout"));
    assert_eq!(
        agent_key(&json!({ "agent": "docs-scout", "agent_id": "sess-1" })).as_deref(),
        Some("docs-scout")
    );
    assert_eq!(agent_key(&json!({ "agent_id": "sess-1" })).as_deref(), Some("sess-1"));
    assert_eq!(agent_key(&json!({ "agent": "  " })), None);
    assert_eq!(agent_key(&json!({})), None);
}
