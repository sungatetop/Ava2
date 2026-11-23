//! 把命中的钩子结果折叠为一个最严格结果（两方言共享）。
//!
//! 权限优先级 `deny > ask > allow`；
//! 首个 `continue:false` stop 粘滞；胜出 rank 的 reasons 连接；context 与
//! system messages 按钩子顺序累积。

use crate::hooks::types::{HookDecision, HookOutput, MergedDecision, MergedHookOutcome};

/// 单钩子决策的 rank（deny>ask>allow 优先级；越高越严格）。
fn rank(decision: Option<HookDecision>) -> u8 {
    match decision {
        Some(HookDecision::Deny | HookDecision::Block) => 3,
        Some(HookDecision::Ask) => 2,
        Some(HookDecision::Approve | HookDecision::Allow) => 1,
        None => 0,
    }
}

/// 把折叠后的 rank 还原为合并枚举。
fn decision_for_rank(max_rank: u8) -> MergedDecision {
    match max_rank {
        3 => MergedDecision::Deny,
        2 => MergedDecision::Ask,
        1 => MergedDecision::Allow,
        _ => MergedDecision::None,
    }
}

/// 折叠 `outputs`（某点全部命中钩子的结果，按钩子顺序）为一个合并结果。
/// 空列表产生中性结果（decision `None`、无 stop、空 context）——调用方视为
/// 「无钩子表态」。
pub fn merge_hook_outputs(outputs: &[HookOutput]) -> MergedHookOutcome {
    let mut max_rank = 0u8;
    // 分 rank 保留 reasons：只有解释胜出决策的反对意见浮现。
    let mut reasons_by_rank: std::collections::BTreeMap<u8, Vec<String>> = Default::default();
    let mut stop = false;
    let mut stop_reason: Option<String> = None;
    let mut additional_context: Vec<String> = Vec::new();
    let mut system_messages: Vec<String> = Vec::new();

    for out in outputs {
        let r = rank(out.decision);
        if r > max_rank {
            max_rank = r;
        }
        if r == 3 || r == 2 {
            if let Some(reason) = out.reason.as_deref().filter(|s| !s.is_empty()) {
                reasons_by_rank.entry(r).or_default().push(reason.to_string());
            }
        }
        if out.continue_ == Some(false) && !stop {
            stop = true;
            if let Some(reason) = &out.stop_reason {
                stop_reason = Some(reason.clone());
            }
        }
        if let Some(ctx) = out.additional_context.as_deref().filter(|s| !s.is_empty()) {
            additional_context.push(ctx.to_string());
        }
        if let Some(msg) = out.system_message.as_deref().filter(|s| !s.is_empty()) {
            system_messages.push(msg.to_string());
        }
    }

    let reasons = reasons_by_rank.remove(&max_rank).unwrap_or_default();
    MergedHookOutcome {
        decision: decision_for_rank(max_rank),
        reason: if reasons.is_empty() { None } else { Some(reasons.join("\n\n")) },
        stop,
        stop_reason,
        additional_context,
        system_messages,
    }
}

#[cfg(test)]
#[path = "merge_tests.rs"]
mod tests;
