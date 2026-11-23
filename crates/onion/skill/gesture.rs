//! `/name` 用户手势注入 listener（skill-invocation）。
//!
//! pre-step 手势 listener 语义：
//! - 仅扫描无 `source` 标记的直接用户输入（系统注入如 catalog / runtime 均带
//!   source，无法伪造手势）；
//! - 用 `SKILL_GESTURE` 正则（`/(^|\s)\/([a-z0-9]+(?:-[a-z0-9]+)*)(?=\s|$)/g`）提取
//!   `/name` token，任意文本块扫描，first-seen 去重；
//! - 命中 user-invocable 技能时注入 `render_skill_content` 作为 user 角色 instructions，
//!   `source: SkillInvocation { name }`；
//! - 未知名 / 非 user-invocable 保持普通 prose（手势从未被本边界认领）；
//! - 注入追加在批次末尾——background 先、material 最后（listener 注册早于 catalog，
//!   瀑布把目录承载列表交给它扩展）。
//!
//! 这是 `disable-model-invocation` 技能的唯一入口：目录与 `skill` 工具永远看不到它们。

use std::sync::{Arc, Mutex};

use crate::listener::{ClaimedInput, Listener, ListenerCtx, PreStepDecision};
use crate::types::{AgentMessage, ContentBlock, MessageSource, Role};
use crate::waterfall::{BoxFuture, Next};

use super::registry::SkillRegistry;
use super::{is_user_invocable, render_skill_content};

/// `/name` 手势 token：空白界定的 kebab-case 技能名。
///
/// Rust `regex` crate 不支持 lookahead，故匹配「name + 前导空白/行首」后在代码里
/// 手动校验 name 之后必须是空白或串尾——第二个 `/` 或非边界字符打破匹配，把
/// 文件路径（`/usr/bin`）与分数（`5/8`）排除。
const SKILL_GESTURE: &str = r"(^|\s)/([a-z0-9]+(?:-[a-z0-9]+)*)";

/// `/name` 用户手势注入 listener。
pub struct SkillGestureListener {
    /// 分层注册表（共享句柄）。
    registry: Arc<Mutex<SkillRegistry>>,
    /// 会话有效 cwd（供 project 级技能 root 发现；未接线时只看固定 roots）。
    session_cwd: Option<Arc<crate::session_cwd::SessionCwd>>,
}

impl SkillGestureListener {
    pub fn new(registry: Arc<Mutex<SkillRegistry>>) -> Self {
        Self { registry, session_cwd: None }
    }

    /// 接线会话有效 cwd：手势解析按当前 cwd 发现 project 级技能。
    pub fn with_session_cwd(mut self, session_cwd: Arc<crate::session_cwd::SessionCwd>) -> Self {
        self.session_cwd = Some(session_cwd);
        self
    }
}

impl Listener for SkillGestureListener {
    fn pre_step(
        &self,
        _ctx: ListenerCtx,
        input: ClaimedInput,
        next: Next<ClaimedInput, PreStepDecision>,
    ) -> BoxFuture<PreStepDecision> {
        let registry = self.registry.clone();
        let cwd = self
            .session_cwd
            .as_ref()
            .map(|cwd| std::path::PathBuf::from(cwd.current().as_path()));
        Box::pin(async move {
            let decision = next(input).await;
            let PreStepDecision::Enter { mut messages, starts_request_series } = decision else {
                return decision;
            };
            let names = invoked_skill_names(&messages);
            if names.is_empty() {
                return PreStepDecision::Enter { messages, starts_request_series };
            }
            let mut injections: Vec<AgentMessage> = Vec::new();
            for name in names {
                let skill = {
                    let mut reg = registry
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    reg.get(&name, cwd.as_deref())
                };
                let Some(skill) = skill else { continue };
                if !is_user_invocable(&skill.invocation) {
                    continue;
                }
                let text = render_skill_content(&skill);
                injections.push(AgentMessage {
                    role: Role::User,
                    blocks: vec![ContentBlock::Text { text }],
                    source: Some(MessageSource::SkillInvocation { name }),
                });
            }
            if injections.is_empty() {
                return PreStepDecision::Enter { messages, starts_request_series };
            }
            // 追加在批次末尾：background 先、material 最后。
            messages.extend(injections);
            PreStepDecision::Enter { messages, starts_request_series }
        })
    }
}

/// `/name` 手势 token：从直接用户消息（`source` 为 None）的全部文本块提取，
/// first-seen 去重。
fn invoked_skill_names(messages: &[AgentMessage]) -> Vec<String> {
    let re = match regex::Regex::new(SKILL_GESTURE) {
        Ok(re) => re,
        Err(_) => return Vec::new(),
    };
    let mut names: Vec<String> = Vec::new();
    for message in messages {
        // 仅直接用户输入可伪造手势（无 source 标记）。
        if message.source.is_some() {
            continue;
        }
        for block in &message.blocks {
            let ContentBlock::Text { text } = block else { continue };
            for capture in re.captures_iter(text) {
                let Some(name) = capture.get(2) else { continue };
                // 手动校验 name 之后必须是空白或串尾（等价 lookahead `(?=\s|$)`）。
                let after = &text[name.end()..];
                let boundary_ok = after.is_empty() || after.starts_with(char::is_whitespace);
                if !boundary_ok {
                    continue;
                }
                let name = name.as_str();
                if !names.iter().any(|n| n == name) {
                    names.push(name.to_string());
                }
            }
        }
    }
    names
}

#[cfg(test)]
#[path = "gesture_tests.rs"]
mod tests;
