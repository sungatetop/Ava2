//! `skill::catalog` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::sync::{Arc, Mutex};

use crate::listener::{ClaimSource, ClaimedInput, Listener};
use crate::session::SessionLog;
use crate::skill::catalog::{catalog_description, digest_entries, SkillCatalogListener};
use crate::skill::registry::{SkillProvider, SkillRegistry};
use crate::skill::{SkillCandidate, SkillDefinition, SkillSource};
use crate::types::{AgentMessage, MessageSource, SkillCatalogEntry};

/// 可变 provider：测试中可增删 candidate，验证目录随 provider 变化（runtime
/// `register` 是 first-wins 无法降级，故用 provider 承载可变模型可调用状态）。
struct MutableProvider {
    candidates: Mutex<Vec<SkillCandidate>>,
}

impl MutableProvider {
    /// 修改候选列表（测试驱动目录变化）。
    fn list_mut(&self) -> std::sync::MutexGuard<'_, Vec<SkillCandidate>> {
        self.candidates.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl SkillProvider for MutableProvider {
    fn name(&self) -> &str {
        "mutable"
    }

    fn list(&self, _cwd: Option<&std::path::Path>) -> Vec<SkillCandidate> {
        self.candidates.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
    }

    fn get(&self, candidate: &SkillCandidate) -> Option<SkillDefinition> {
        Some(SkillDefinition {
            path: candidate.path.clone(),
            name: candidate.name.clone(),
            description: candidate.description.clone(),
            when_to_use: candidate.when_to_use.clone(),
            invocation: candidate.invocation.clone(),
            source: candidate.source.clone(),
            provider: candidate.provider.clone(),
            resource_base: candidate.resource_base.clone(),
            content: format!("body of {}", candidate.name),
        })
    }
}

fn candidate(name: &str, description: &str, model_invocable: bool) -> SkillCandidate {
    SkillCandidate {
        path: None,
        name: name.to_string(),
        description: description.to_string(),
        when_to_use: None,
        invocation: crate::skill::SkillInvocationPolicy {
            model_invocable,
            user_invocable: true,
        },
        source: SkillSource::Runtime,
        provider: "mutable".to_string(),
        resource_base: None,
        rank: 100,
        locator: None,
    }
}

fn mutable_registry(initial: Vec<SkillCandidate>) -> (Arc<Mutex<SkillRegistry>>, Arc<MutableProvider>) {
    let provider = Arc::new(MutableProvider { candidates: Mutex::new(initial) });
    let mut registry = SkillRegistry::new();
    registry.register_provider(provider.clone()).expect("register mutable provider");
    (Arc::new(Mutex::new(registry)), provider)
}

fn input(messages: Vec<AgentMessage>) -> ClaimedInput {
    ClaimedInput {
        turn: 1,
        step: 1,
        source: ClaimSource::TurnOpen,
        messages,
        starts_request_series: true,
    }
}

fn enter(messages: Vec<AgentMessage>) -> crate::listener::PreStepDecision {
    crate::listener::PreStepDecision::Enter { messages, starts_request_series: true }
}

async fn run(listener: &SkillCatalogListener, session: Arc<SessionLog>, messages: Vec<AgentMessage>) -> Vec<AgentMessage> {
    let ctx = crate::listener::ListenerCtx {
        inbox: Arc::new(crate::inbox::Inbox::new(session.bus().clone())),
        session,
        state: "Running".to_string(),
    };
    match listener.pre_step(ctx, input(messages), Box::new(|i| Box::pin(async move { enter(i.messages) }))).await {
        crate::listener::PreStepDecision::Enter { messages, .. } => messages,
        crate::listener::PreStepDecision::Reject { reason } => panic!("unexpected reject: {reason}"),
    }
}

fn find_catalog(messages: &[AgentMessage]) -> Option<&MessageSource> {
    messages.iter().find_map(|m| m.source.as_ref())
}

#[tokio::test]
async fn injects_catalog_on_first_step_with_model_skills() {
    let (reg, _provider) = mutable_registry(vec![
        candidate("a-skill", "Use {{placeholder}} <safely> & carefully.", true),
        candidate("model-only", "Model only.", true),
        candidate("user-only", "User only.", false), // 不进目录
    ]);
    let session = Arc::new(SessionLog::in_memory());
    let messages = run(&SkillCatalogListener::new(reg), session, vec![]).await;

    let source = find_catalog(&messages).expect("catalog injected");
    let MessageSource::SkillCatalog { update, entries } = source else {
        panic!("expected skill-catalog source");
    };
    assert_eq!(*update, None);
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    // 模型可调用技能按名排序；user-only 被过滤。
    assert_eq!(names, vec!["a-skill", "model-only"]);
    // prose 含转义描述。
    let text = messages.iter().find_map(|m| {
        let t = m.text_content();
        if t.is_empty() { None } else { Some(t) }
    }).unwrap_or_default();
    assert!(text.contains("<available_skills>"), "{text}");
    assert!(text.contains("&lt;safely&gt; &amp; carefully."), "{text}");
    assert!(!text.contains("User only."), "{text}");
}

#[tokio::test]
async fn no_catalog_when_no_model_skills() {
    let (reg, _provider) = mutable_registry(vec![candidate("user-only", "User only.", false)]);
    let session = Arc::new(SessionLog::in_memory());
    let messages = run(&SkillCatalogListener::new(reg), session, vec![]).await;
    assert!(find_catalog(&messages).is_none());
}

#[tokio::test]
async fn dedups_same_catalog_across_steps_via_session_history() {
    let (reg, _provider) = mutable_registry(vec![candidate("a-skill", "A skill.", true)]);
    let session = Arc::new(SessionLog::in_memory());
    // 第一步注入。
    let first = run(&SkillCatalogListener::new(reg.clone()), session.clone(), vec![]).await;
    let source = find_catalog(&first).cloned().expect("catalog");
    // 模拟持久化：把 catalog 消息 append 进 session（durable）。
    if let Some(msg) = first.iter().find(|m| matches!(m.source, Some(MessageSource::SkillCatalog { .. }))) {
        session.append(crate::session::SessionEvent::UserMessage {
            turn: 1,
            step: 1,
            message: msg.clone(),
        });
    }
    // 第二步：目录未变，不重复注入。
    let second = run(&SkillCatalogListener::new(reg), session, vec![]).await;
    assert!(find_catalog(&second).is_none());
    let _ = source;
}

#[tokio::test]
async fn injects_replacement_when_catalog_changes() {
    let (reg, provider) = mutable_registry(vec![candidate("a-skill", "A skill.", true)]);
    let session = Arc::new(SessionLog::in_memory());
    let first = run(&SkillCatalogListener::new(reg.clone()), session.clone(), vec![]).await;
    if let Some(msg) = first.iter().find(|m| matches!(m.source, Some(MessageSource::SkillCatalog { .. }))) {
        session.append(crate::session::SessionEvent::UserMessage {
            turn: 1,
            step: 1,
            message: msg.clone(),
        });
    }
    // 新增技能 → catalog 变化。
    provider.list_mut().push(candidate("b-skill", "B skill.", true));
    reg.lock().unwrap_or_else(std::sync::PoisonError::into_inner).refresh();
    let second = run(&SkillCatalogListener::new(reg), session, vec![]).await;
    let source = find_catalog(&second).expect("replacement injected");
    let MessageSource::SkillCatalog { update, entries } = source else {
        panic!("expected skill-catalog");
    };
    assert_eq!(*update, Some(true));
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, vec!["a-skill", "b-skill"]);
}

#[tokio::test]
async fn empty_tombstone_on_all_removals() {
    let (reg, provider) = mutable_registry(vec![candidate("a-skill", "A skill.", true)]);
    let session = Arc::new(SessionLog::in_memory());
    let first = run(&SkillCatalogListener::new(reg.clone()), session.clone(), vec![]).await;
    if let Some(msg) = first.iter().find(|m| matches!(m.source, Some(MessageSource::SkillCatalog { .. }))) {
        session.append(crate::session::SessionEvent::UserMessage {
            turn: 1,
            step: 1,
            message: msg.clone(),
        });
    }
    // 移除全部 → 空 tombstone（模型目录空）。
    provider.list_mut().clear();
    reg.lock().unwrap_or_else(std::sync::PoisonError::into_inner).refresh();
    let second = run(&SkillCatalogListener::new(reg), session, vec![]).await;
    let source = find_catalog(&second).expect("tombstone injected");
    let MessageSource::SkillCatalog { update, entries } = source else {
        panic!("expected skill-catalog");
    };
    assert_eq!(*update, Some(true));
    assert!(entries.is_empty());
    let text = second
        .iter()
        .find_map(|m| {
            let t = m.text_content();
            if t.is_empty() { None } else { Some(t) }
        })
        .unwrap_or_default();
    assert!(text.contains("No skills are currently available"), "{text}");
}

#[test]
fn catalog_description_normalizes_and_truncates() {
    let long = "Long   description ".repeat(5);
    let normalized = catalog_description(&long, 50);
    assert!(normalized.len() <= 50);
    assert!(!normalized.contains("  "));
    let short = catalog_description("Use {{x}} <y>.", 50);
    assert_eq!(short, "Use {{x}} <y>.");
}

#[test]
fn digest_entries_ignores_order_of_rendered_prose() {
    let a = vec![SkillCatalogEntry { name: "a".to_string(), description: "d".to_string() }];
    let same = vec![SkillCatalogEntry { name: "a".to_string(), description: "d".to_string() }];
    let different = vec![SkillCatalogEntry { name: "a".to_string(), description: "other".to_string() }];
    assert_eq!(digest_entries(&a), digest_entries(&same));
    assert_ne!(digest_entries(&a), digest_entries(&different));
}
