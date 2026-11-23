//! `subagent/model_selection` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::{
    assert_allowed_model_routes, assert_allowed_model_selection, model_route_key,
    policy_from_events, AllowedModelRoute,
};
use crate::session::SessionEvent;

fn route(provider: &str, model: &str) -> AllowedModelRoute {
    AllowedModelRoute::new(provider, model)
}

#[test]
fn route_key_is_provider_nul_model() {
    assert_eq!(model_route_key(&route("local", "m")), "local\0m");
}

#[test]
fn assert_routes_rejects_empty_ids() {
    let err = assert_allowed_model_routes(&[route("", "m")]).unwrap_err();
    assert_eq!(
        err,
        "subagent model selection requires non-empty provider and model ids"
    );
    let err = assert_allowed_model_routes(&[route("local", "")]).unwrap_err();
    assert_eq!(
        err,
        "subagent model selection requires non-empty provider and model ids"
    );
}

#[test]
fn assert_routes_rejects_duplicates() {
    let err = assert_allowed_model_routes(&[route("local", "m"), route("local", "m")]).unwrap_err();
    assert_eq!(err, "subagent model selection repeats route \"local/m\"");
}

#[test]
fn assert_routes_accepts_distinct_routes() {
    assert_allowed_model_routes(&[route("local", "a"), route("local", "b"), route("other", "a")])
        .unwrap();
}

#[test]
fn policy_projection_takes_first_event_only() {
    let events = vec![
        SessionEvent::TurnStart { turn: 1 },
        SessionEvent::SubagentModelSelectionPolicy {
            allowed_models: vec![route("local", "a")],
        },
        SessionEvent::SubagentModelSelectionPolicy {
            allowed_models: vec![route("local", "b")],
        },
    ];
    assert_eq!(policy_from_events(&events), Some(vec![route("local", "a")]));
}

#[test]
fn policy_projection_absent_returns_none() {
    assert_eq!(policy_from_events(&[SessionEvent::TurnStart { turn: 1 }]), None);
}

#[test]
fn selection_without_policy_allows_only_parent_route() {
    let parent = route("local", "Qwen3.8-27B");
    assert_allowed_model_selection(None, Some(&parent), &parent).unwrap();
    let err = assert_allowed_model_selection(None, Some(&parent), &route("local", "other"))
        .unwrap_err();
    assert_eq!(
        err,
        "child LLM route \"local/other\" is not allowed for this Session; without a recorded \
         model-selection policy, explicit selection is limited to the parent route"
    );
}

#[test]
fn selection_with_policy_uses_allowlist() {
    let policy = vec![route("local", "a"), route("local", "b")];
    let parent = route("local", "a");
    assert_allowed_model_selection(Some(&policy), Some(&parent), &route("local", "b")).unwrap();
    let err =
        assert_allowed_model_selection(Some(&policy), Some(&parent), &route("local", "c"))
            .unwrap_err();
    assert_eq!(
        err,
        "child LLM route \"local/c\" is not allowed for this Session"
    );
}

#[test]
fn selection_with_policy_ignores_parent_route() {
    // 白名单存在时父路由本身也必须在列表内（policy 完全取代父路由兜底）。
    let policy = vec![route("local", "b")];
    let parent = route("local", "a");
    let err = assert_allowed_model_selection(Some(&policy), Some(&parent), &parent).unwrap_err();
    assert_eq!(
        err,
        "child LLM route \"local/a\" is not allowed for this Session"
    );
}
