//! 子代理 LLM 路由白名单。
//!
//! 语义要点：
//! - 白名单是**会话级 durable 事实**（`subagent/model-selection-policy` 事件），
//!   在首个模型请求之前 append 一次；子会话继承父会话的白名单。
//! - **没有**该事件时，显式选择只允许父会话当前路由（不是「目录内任意路由」）。
//! - 事件里的路由列表必须非空、无重复、provider/model 均非空。

use serde::{Deserialize, Serialize};

use crate::session::SessionEvent;

/// 一条被授权的精确子代理路由。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllowedModelRoute {
    /// 已注册的 LLM provider id。
    pub provider: String,
    /// provider 自有的精确模型 id。
    pub model: String,
}

impl AllowedModelRoute {
    pub fn new(provider: impl Into<String>, model: impl Into<String>) -> Self {
        Self { provider: provider.into(), model: model.into() }
    }
}

/// 路由稳定标识（`provider\0model`）。
pub fn model_route_key(route: &AllowedModelRoute) -> String {
    format!("{}\0{}", route.provider, route.model)
}

/// 校验路由列表：非空 id、无重复。
pub fn assert_allowed_model_routes(routes: &[AllowedModelRoute]) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    for route in routes {
        if route.provider.is_empty() || route.model.is_empty() {
            return Err(
                "subagent model selection requires non-empty provider and model ids".to_string(),
            );
        }
        if !seen.insert(model_route_key(route)) {
            return Err(format!(
                "subagent model selection repeats route \"{}/{}\"",
                route.provider, route.model
            ));
        }
    }
    Ok(())
}

/// 从 durable 事件投影出会话白名单（**首条**生效，后续忽略）。
///
/// 返回 `None` = 该会话没有记录白名单 → 显式选择只允许父路由。
pub fn policy_from_events(events: &[SessionEvent]) -> Option<Vec<AllowedModelRoute>> {
    events.iter().find_map(|event| match event {
        SessionEvent::SubagentModelSelectionPolicy { allowed_models } => {
            Some(allowed_models.clone())
        }
        _ => None,
    })
}

/// 校验显式选择的路由是否被授权。
///
/// - `policy = Some(routes)`：必须在列表内；
/// - `policy = None`：只允许父会话当前路由。
pub fn assert_allowed_model_selection(
    policy: Option<&[AllowedModelRoute]>,
    parent_route: Option<&AllowedModelRoute>,
    route: &AllowedModelRoute,
) -> Result<(), String> {
    let allowed = match policy {
        Some(routes) => routes.iter().any(|r| r == route),
        None => parent_route.is_some_and(|parent| parent == route),
    };
    if allowed {
        return Ok(());
    }
    let hint = if policy.is_none() {
        "; without a recorded model-selection policy, explicit selection is limited to the parent route"
    } else {
        ""
    };
    Err(format!(
        "child LLM route \"{}/{}\" is not allowed for this Session{hint}",
        route.provider, route.model
    ))
}

/// 在首个模型请求之前写入会话白名单。
///
/// - `routes = None`（未启用）→ 不写事件（显式选择只允许父路由）；
/// - 已有该事件（冷恢复 / 子会话继承）→ 不重复写；
/// - 空列表 → 报错（`requires at least one route`）。
pub fn seed_policy(
    log: &crate::session::SessionLog,
    routes: Option<&[AllowedModelRoute]>,
) -> Result<(), String> {
    let Some(routes) = routes else {
        return Ok(());
    };
    if policy_from_events(&log.snapshot()).is_some() {
        return Ok(());
    }
    if routes.is_empty() {
        return Err("subagent/model-selection-policy requires at least one route".to_string());
    }
    assert_allowed_model_routes(routes)?;
    log.append(SessionEvent::SubagentModelSelectionPolicy {
        allowed_models: routes.to_vec(),
    });
    Ok(())
}

#[cfg(test)]
#[path = "model_selection_tests.rs"]
mod tests;
