//! `merge` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use crate::hooks::merge::merge_hook_outputs;
use crate::hooks::types::{HookDecision, HookOutput, MergedDecision};

fn output(decision: Option<HookDecision>) -> HookOutput {
    HookOutput { decision, ..Default::default() }
}

#[test]
fn empty_input_is_neutral() {
    let merged = merge_hook_outputs(&[]);
    assert_eq!(merged.decision, MergedDecision::None);
    assert!(!merged.stop);
    assert!(merged.additional_context.is_empty());
}

#[test]
fn deny_beats_allow() {
    let merged = merge_hook_outputs(&[
        output(Some(HookDecision::Allow)),
        output(Some(HookDecision::Deny)),
    ]);
    assert_eq!(merged.decision, MergedDecision::Deny);
}

#[test]
fn deny_beats_ask() {
    let merged = merge_hook_outputs(&[
        output(Some(HookDecision::Ask)),
        output(Some(HookDecision::Block)),
    ]);
    assert_eq!(merged.decision, MergedDecision::Deny);
}

#[test]
fn ask_beats_allow() {
    let merged = merge_hook_outputs(&[
        output(Some(HookDecision::Allow)),
        output(Some(HookDecision::Ask)),
    ]);
    assert_eq!(merged.decision, MergedDecision::Ask);
}

#[test]
fn allow_folds_to_allow() {
    let merged = merge_hook_outputs(&[
        output(Some(HookDecision::Approve)),
        output(Some(HookDecision::Allow)),
    ]);
    assert_eq!(merged.decision, MergedDecision::Allow);
}

#[test]
fn first_stop_is_sticky() {
    let mut a = output(None);
    a.continue_ = Some(false);
    a.stop_reason = Some("first".to_string());
    let mut b = output(None);
    b.continue_ = Some(false);
    b.stop_reason = Some("second".to_string());
    let merged = merge_hook_outputs(&[a, b]);
    assert!(merged.stop);
    assert_eq!(merged.stop_reason.as_deref(), Some("first"));
}

#[test]
fn winning_rank_reasons_joined() {
    let mut a = output(Some(HookDecision::Deny));
    a.reason = Some("r1".to_string());
    let mut b = output(Some(HookDecision::Deny));
    b.reason = Some("r2".to_string());
    let mut allow = output(Some(HookDecision::Allow));
    allow.reason = Some("irrelevant".to_string());
    let merged = merge_hook_outputs(&[a, b, allow]);
    assert_eq!(merged.decision, MergedDecision::Deny);
    assert_eq!(merged.reason.as_deref(), Some("r1\n\nr2"));
}

#[test]
fn context_and_messages_accumulate_in_order() {
    let mut a = output(None);
    a.additional_context = Some("ctx1".to_string());
    a.system_message = Some("sys1".to_string());
    let mut b = output(None);
    b.additional_context = Some("ctx2".to_string());
    let merged = merge_hook_outputs(&[a, b]);
    assert_eq!(merged.additional_context, vec!["ctx1".to_string(), "ctx2".to_string()]);
    assert_eq!(merged.system_messages, vec!["sys1".to_string()]);
}
