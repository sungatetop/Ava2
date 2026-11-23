//! `skill` 工具：经分层注册表加载技能指令。
//!
//! 语义：
//! - 输入 `name: string`，须匹配 kebab-case `^[a-z0-9]+(?:-[a-z0-9]+)*$`；
//! - 从 `SkillRegistry` 加载（provider 合并 + rank 决胜），而非直接读目录；
//! - summary 与 loaded definition 各查一次 model-invocable（双检查）；
//! - 返回 `{name, provider, resourceBase, content}`，渲染 `<skill_content>` 块；
//! - 只读（ReadOnly）、非 deferred；
//! - 无 owning session（非 agent 调用方）拒绝。

use std::sync::{Arc, Mutex};

use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::{is_model_invocable, is_skill_name, render_skill_content};
use super::registry::SkillRegistry;
use crate::session::SessionLog;
use crate::tools::ToolPort;
use crate::types::{ContentBlock, ToolCall, ToolDefinition};
use crate::waterfall::BoxFuture;

/// `skill` 工具：分层注册表 + 会话接线槽。
#[derive(Clone)]
pub struct SkillTool {
    /// 分层技能注册表（provider 合并 + rank 决胜）。
    registry: Arc<Mutex<SkillRegistry>>,
    /// 会话接线槽（Runtime 装配后 wire）。
    session: Arc<std::sync::OnceLock<Arc<SessionLog>>>,
    /// 会话有效 cwd（供 project 级技能 root 发现；未接线时只看固定 roots）。
    session_cwd: Option<Arc<crate::session_cwd::SessionCwd>>,
}

impl SkillTool {
    pub fn new(registry: Arc<Mutex<SkillRegistry>>) -> Self {
        Self {
            registry,
            session: Arc::new(std::sync::OnceLock::new()),
            session_cwd: None,
        }
    }

    /// 接线会话有效 cwd：加载按当前 cwd 发现 project 级技能。
    pub fn with_session_cwd(mut self, session_cwd: Arc<crate::session_cwd::SessionCwd>) -> Self {
        self.session_cwd = Some(session_cwd);
        self
    }

    /// 延迟接线会话（宿主装配完成后调用）。
    pub fn wire_session(&self, session: Arc<SessionLog>) {
        let _ = self.session.set(session);
    }

    fn session(&self) -> Result<Arc<SessionLog>, String> {
        self.session
            .get()
            .cloned()
            .ok_or_else(|| "skill requires an owning agent session".to_string())
    }
}

impl ToolPort for SkillTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "skill".to_string(),
            description: "Load the full instructions for a skill. Call it before acting on a task \
                that names or clearly matches a skill in the session skill catalog."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "The exact skill name from the available skills list."
                    }
                },
                "required": ["name"]
            }),
        }
    }

    fn execute(&self, call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let registry = self.registry.clone();
        let session = self.session();
        let cwd = self
            .session_cwd
            .as_ref()
            .map(|cwd| std::path::PathBuf::from(cwd.current().as_path()));
        Box::pin(async move {
            session?;
            let name = call
                .input
                .get("name")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "skill: missing 'name'".to_string())?;
            if !is_skill_name(name) {
                return Err(format!("invalid skill name \"{name}\""));
            }
            let mut registry = registry.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            // summary 检查 model-invocable（目录面可见性）。
            let Some(summary) = registry.summary(name, cwd.as_deref()) else {
                return Err(format!("skill \"{name}\" is unknown or no longer available"));
            };
            if !is_model_invocable(&summary.invocation) {
                return Err(format!("skill \"{name}\" is not available for model invocation"));
            }
            // 加载后再查一次 loaded definition（双检查）。
            let Some(skill) = registry.get(name, cwd.as_deref()) else {
                return Err(format!("skill \"{name}\" is unknown or no longer available"));
            };
            if !is_model_invocable(&skill.invocation) {
                return Err(format!("skill \"{name}\" is not available for model invocation"));
            }
            let text = render_skill_content(&skill);
            Ok(vec![ContentBlock::Text { text }])
        })
    }
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
