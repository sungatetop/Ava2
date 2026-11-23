//! skill-catalog 注入 listener（会话技能目录）。
//!
//! catalog listener 语义：
//! - pre-step 瀑布内调用 `next()` 后，若为 Enter 则注入/替换 `<available_skills>`
//!   目录消息（`source: SkillCatalog`）；
//! - 仅模型可调用（model-invocable）技能进入目录；
//! - digest 去重：目录身份基于 `source.entries` 而非渲染 prose，避免重复发布；
//! - 首次发布不带 `update` 标记；后续变更带 `update: true`（替换）；
//! - 全部删除时注入「空 tombstone」目录（明确告知模型旧目录失效）；
//! - 目录历史从 durable 事件扫描（`catalog_history`）：会话恢复/续跑时据此判定
//!   是首次发布还是替换。

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use sha2::{Digest, Sha256};

use crate::listener::{ClaimedInput, Listener, ListenerCtx, PreStepDecision};
use crate::session::{SessionEvent, SessionLog};
use crate::types::{AgentMessage, ContentBlock, MessageSource, Role, SkillCatalogEntry};
use crate::waterfall::{BoxFuture, Next};

use super::registry::SkillRegistry;
use super::{escape_text, is_model_invocable};

/// 目录描述最大长度（缺省 500）。
const DEFAULT_CATALOG_DESCRIPTION_MAX_LENGTH: usize = 500;

/// skill-catalog 注入 listener。
pub struct SkillCatalogListener {
    /// 分层注册表（共享句柄）。
    registry: Arc<Mutex<SkillRegistry>>,
    /// 调用方是否可见 `skill` 工具（onion 单层恒 true）。
    tool_visible: bool,
    /// 目录描述最大长度（截断用）。
    description_max_length: usize,
    /// 会话有效 cwd（供 project 级技能 root 发现；未接线时只看固定 roots）。
    session_cwd: Option<Arc<crate::session_cwd::SessionCwd>>,
}

impl SkillCatalogListener {
    pub fn new(registry: Arc<Mutex<SkillRegistry>>) -> Self {
        Self {
            registry,
            tool_visible: true,
            description_max_length: DEFAULT_CATALOG_DESCRIPTION_MAX_LENGTH,
            session_cwd: None,
        }
    }

    /// 接线会话有效 cwd：目录注入按当前 cwd 发现 project 级技能。
    pub fn with_session_cwd(mut self, session_cwd: Arc<crate::session_cwd::SessionCwd>) -> Self {
        self.session_cwd = Some(session_cwd);
        self
    }

    /// 显式指定工具可见性（scope 工具限制的语义 seam）。
    pub fn with_tool_visible(mut self, tool_visible: bool) -> Self {
        self.tool_visible = tool_visible;
        self
    }

    /// 显式指定目录描述长度上限（须 >= 3）。
    pub fn with_description_max_length(mut self, max: usize) -> Self {
        self.description_max_length = max.max(3);
        self
    }
}

impl Listener for SkillCatalogListener {
    fn pre_step(
        &self,
        ctx: ListenerCtx,
        input: ClaimedInput,
        next: Next<ClaimedInput, PreStepDecision>,
    ) -> BoxFuture<PreStepDecision> {
        let registry = self.registry.clone();
        let tool_visible = self.tool_visible;
        let description_max_length = self.description_max_length;
        let session_cwd = self.session_cwd.clone();
        Box::pin(async move {
            let decision = next(input).await;
            let PreStepDecision::Enter { messages, starts_request_series } = decision else {
                return decision;
            };
            // 仅当 `skill` 工具对调用方可见时注入目录。
            let cwd = session_cwd
                .as_ref()
                .map(|cwd| cwd.current())
                .map(|current| PathBuf::from(current.as_path()));
            let cwd_ref = cwd.as_deref();
            let skills = if tool_visible {
                registry
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .list(cwd_ref)
            } else {
                Vec::new()
            };
            // 仅模型可调用技能进入目录（model-invocable 过滤）。
            let skills: Vec<_> = skills
                .into_iter()
                .filter(|s| is_model_invocable(&s.invocation))
                .collect();
            let entries: Vec<SkillCatalogEntry> = skills
                .iter()
                .map(|s| SkillCatalogEntry {
                    name: s.name.clone(),
                    description: catalog_description(&s.description, description_max_length),
                })
                .collect();
            let digest = digest_entries(&entries);
            let (visible_digest, published) = catalog_history(&ctx.session);
            let existing = catalog_message_index(&messages);

            // 与最近可见目录相同 → 不发布；若本步已提出则移除。
            if visible_digest.as_deref() == Some(digest.as_str()) {
                return if let Some(idx) = existing {
                    let mut messages = messages;
                    messages.remove(idx);
                    PreStepDecision::Enter { messages, starts_request_series }
                } else {
                    PreStepDecision::Enter { messages, starts_request_series }
                };
            }
            // 本步已提出且 digest 相同 → 保留（去重）。
            if let Some(idx) = existing {
                if let Some(MessageSource::SkillCatalog { entries: proposed, .. }) =
                    messages[idx].source.as_ref()
                {
                    if digest_entries(proposed) == digest {
                        return PreStepDecision::Enter { messages, starts_request_series };
                    }
                }
            }
            // 未发布且无模型可调用技能 → 不注入目录（空会话无目录）。
            if !published && skills.is_empty() {
                return if let Some(idx) = existing {
                    let mut messages = messages;
                    messages.remove(idx);
                    PreStepDecision::Enter { messages, starts_request_series }
                } else {
                    PreStepDecision::Enter { messages, starts_request_series }
                };
            }
            // 构建目录消息（首次无 update 标记；后续带 update: true）。
            let update = published.then_some(true);
            let text = if published {
                render_catalog_update(&entries)
            } else {
                render_catalog_message(&entries)
            };
            let message = AgentMessage {
                role: Role::User,
                blocks: vec![ContentBlock::Text { text }],
                source: Some(MessageSource::SkillCatalog { update, entries }),
            };
            let mut messages = messages;
            if let Some(idx) = existing {
                messages[idx] = message;
            } else {
                messages.push(message);
            }
            PreStepDecision::Enter { messages, starts_request_series }
        })
    }
}

/// 首次发布的目录消息（带 `<system-reminder>` 引导）。
fn render_catalog_message(entries: &[SkillCatalogEntry]) -> String {
    [
        "<system-reminder>".to_string(),
        "A skill is a reusable set of task-specific instructions. The following skills are available in this session:".to_string(),
        String::new(),
        "<available_skills>".to_string(),
    ]
    .into_iter()
    .chain(render_catalog_entries(entries))
    .chain([
        "</available_skills>".to_string(),
        String::new(),
        "If the user names a skill, or the task clearly matches a skill's description, call the `skill` tool with the exact skill name before taking task actions. Load all applicable skills, then follow their full instructions. This catalog contains summaries only; do not infer or follow a skill's instructions until it has been loaded.".to_string(),
        "A user may also invoke a skill directly; its <skill_content> block then appears in this conversation. Follow it, and do not call the `skill` tool again for that skill.".to_string(),
        "</system-reminder>".to_string(),
    ])
    .collect::<Vec<_>>()
    .join("\n")
}

/// 替换目录消息（`update` 标记 + 空 tombstone 支持）。
fn render_catalog_update(entries: &[SkillCatalogEntry]) -> String {
    let availability: Vec<String> = if entries.is_empty() {
        vec![
            "No skills are currently available through the `skill` tool. Do not use names from earlier skill catalogs.".to_string(),
            "A user may still invoke a skill directly; its <skill_content> block then appears in this conversation. Follow it, and do not call the `skill` tool for it.".to_string(),
        ]
    } else {
        vec![
            "Use only names in this replacement catalog. If the user names a listed skill, or the task clearly matches its description, call the `skill` tool with the exact name before acting.".to_string(),
            "A user may also invoke a skill directly; its <skill_content> block then appears in this conversation. Follow it, and do not call the `skill` tool again for that skill.".to_string(),
        ]
    };
    [
        "<system-reminder>".to_string(),
        "The available skill catalog changed. This complete catalog replaces every earlier available-skills list in this session:".to_string(),
        String::new(),
        "<available_skills>".to_string(),
    ]
    .into_iter()
    .chain(render_catalog_entries(entries))
    .chain([
        "</available_skills>".to_string(),
        String::new(),
    ])
    .chain(availability)
    .chain(["</system-reminder>".to_string()])
    .collect::<Vec<_>>()
    .join("\n")
}

/// 目录行：`- \`name\`: <escaped description>`（转义属于本帧，不落 durable）。
fn render_catalog_entries(entries: &[SkillCatalogEntry]) -> Vec<String> {
    entries
        .iter()
        .map(|entry| format!("- `{}`: {}", entry.name, escape_text(&entry.description)))
        .collect()
}

/// 目录身份 digest：对 `[name, description]` 的规范 JSON 行做 sha256。
pub fn digest_entries(entries: &[SkillCatalogEntry]) -> String {
    let canonical: Vec<String> = entries
        .iter()
        .map(|e| serde_json::to_string(&[e.name.as_str(), e.description.as_str()]).unwrap_or_default())
        .collect();
    let joined = canonical.join("\n");
    let mut hasher = Sha256::new();
    hasher.update(joined.as_bytes());
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

/// 规范化 + 截断目录描述（空白折叠后截断）。
fn catalog_description(value: &str, max_length: usize) -> String {
    let normalized: String = value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if normalized.chars().count() <= max_length {
        normalized
    } else {
        let mut truncated: String = normalized.chars().take(max_length.saturating_sub(3)).collect();
        truncated.push_str("...");
        truncated
    }
}

/// 从 durable 事件扫描最新可见 skill-catalog 的 digest 与是否已发布。
/// 返回 (visible_digest, published)。onion 单层：全部事件可见，最新可读目录即
/// 可见目录；`published` = 是否曾发布过任一可读目录。
fn catalog_history(session: &SessionLog) -> (Option<String>, bool) {
    let mut published = false;
    let mut visible_digest: Option<String> = None;
    for event in session.snapshot() {
        if let SessionEvent::UserMessage { message, .. } = event {
            if let Some(MessageSource::SkillCatalog { entries, .. }) = &message.source {
                published = true;
                visible_digest = Some(digest_entries(entries));
            }
        }
    }
    (visible_digest, published)
}

/// 在批次消息中定位 skill-catalog 消息的索引（首个）。
fn catalog_message_index(messages: &[AgentMessage]) -> Option<usize> {
    messages.iter().position(|m| {
        matches!(m.source, Some(MessageSource::SkillCatalog { .. }))
    })
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
