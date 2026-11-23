//! `skill::registry` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::sync::Arc;

use super::{SkillProvider, SkillRegistry};
use crate::skill::{
    SkillCandidate, SkillDefinition, SkillInvocationPolicy, SkillRegistration, SkillResourceBase,
    SkillSource,
};

/// 内存 provider：按给定 candidate 列表构造，`get` 回读候选。
fn mem_provider(name: &str, candidates: Vec<SkillCandidate>) -> Arc<dyn SkillProvider> {
    Arc::new(MemProvider { name: name.to_string(), candidates })
}

struct MemProvider {
    name: String,
    candidates: Vec<SkillCandidate>,
}

impl SkillProvider for MemProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn list(&self, _cwd: Option<&std::path::Path>) -> Vec<SkillCandidate> {
        self.candidates.clone()
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

fn candidate(name: &str, rank: u32, source: SkillSource, provider: &str) -> SkillCandidate {
    SkillCandidate {
        path: None,
        name: name.to_string(),
        description: format!("desc {name}"),
        when_to_use: None,
        invocation: SkillInvocationPolicy::default(),
        source,
        provider: provider.to_string(),
        resource_base: None,
        rank,
        locator: None,
    }
}

#[test]
fn register_runtime_skill_and_list() {
    let mut registry = SkillRegistry::new();
    registry.register(SkillRegistration {
        name: "a-skill".to_string(),
        description: "A skill".to_string(),
        when_to_use: None,
        source: SkillSource::Runtime,
        resource_base: Some(SkillResourceBase::Opaque { description: "runtime".to_string() }),
        content: "A body.".to_string(),
        invocation: None,
        provider: None,
    });
    let summaries = registry.list(None);
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].name, "a-skill");
    assert_eq!(summaries[0].provider, "runtime");
    assert_eq!(summaries[0].invocation, SkillInvocationPolicy::default());

    let loaded = registry.get("a-skill", None).expect("skill loads");
    assert_eq!(loaded.content, "A body.");
}

#[test]
fn runtime_duplicate_first_wins() {
    let mut registry = SkillRegistry::new();
    let reg = |name: &str, content: &str| SkillRegistration {
        name: name.to_string(),
        description: "d".to_string(),
        when_to_use: None,
        source: SkillSource::Runtime,
        resource_base: None,
        content: content.to_string(),
        invocation: None,
        provider: None,
    };
    registry.register(reg("dup", "first"));
    registry.register(reg("dup", "second"));
    let loaded = registry.get("dup", None).expect("loads");
    assert_eq!(loaded.content, "first");
}

#[test]
fn invalid_or_empty_runtime_registration_ignored() {
    let mut registry = SkillRegistry::new();
    registry.register(SkillRegistration {
        name: "Bad_Name".to_string(),
        description: "d".to_string(),
        when_to_use: None,
        source: SkillSource::Runtime,
        resource_base: None,
        content: "b".to_string(),
        invocation: None,
        provider: None,
    });
    registry.register(SkillRegistration {
        name: "empty-desc".to_string(),
        description: String::new(),
        when_to_use: None,
        source: SkillSource::Runtime,
        resource_base: None,
        content: "b".to_string(),
        invocation: None,
        provider: None,
    });
    assert!(registry.list(None).is_empty());
}

#[test]
fn provider_rank_decides_duplicate_name() {
    let mut registry = SkillRegistry::new();
    let low_rank = candidate("shared", 100, SkillSource::ProjectOnion, "p1");
    let high_rank = candidate("shared", 600, SkillSource::Bundled, "p2");
    let p1 = mem_provider("p1", vec![low_rank]);
    let p2 = mem_provider("p2", vec![high_rank]);
    registry.register_provider(p1).expect("register p1");
    registry.register_provider(p2).expect("register p2");

    let loaded = registry.get("shared", None).expect("loads");
    // 低 rank 胜出。
    assert_eq!(loaded.provider, "p1");
}

#[test]
fn provider_provider_order_breaks_rank_tie() {
    let mut registry = SkillRegistry::new();
    // 同 rank：先注册的 provider（provider order 小）胜出。
    let first = candidate("tie", 300, SkillSource::Custom, "first");
    let second = candidate("tie", 300, SkillSource::Custom, "second");
    registry.register_provider(mem_provider("first", vec![first])).expect("register first");
    registry.register_provider(mem_provider("second", vec![second])).expect("register second");

    let loaded = registry.get("tie", None).expect("loads");
    assert_eq!(loaded.provider, "first");
}

#[test]
fn runtime_outranks_provider_duplicate() {
    let mut registry = SkillRegistry::new();
    // runtime rank = 250；provider rank = 600 → runtime 胜出。
    registry.register(SkillRegistration {
        name: "shared".to_string(),
        description: "runtime skill".to_string(),
        when_to_use: None,
        source: SkillSource::Runtime,
        resource_base: None,
        content: "runtime body".to_string(),
        invocation: None,
        provider: None,
    });
    registry.register_provider(mem_provider("p", vec![candidate("shared", 600, SkillSource::Bundled, "p")]))
        .expect("register p");

    let loaded = registry.get("shared", None).expect("loads");
    assert_eq!(loaded.provider, "runtime");
    assert_eq!(loaded.content, "runtime body");
}

#[test]
fn register_provider_rejects_runtime_reserved_name_and_duplicate() {
    let mut registry = SkillRegistry::new();
    assert!(registry.register_provider(mem_provider("runtime", vec![])).is_err());
    registry.register_provider(mem_provider("dup", vec![])).expect("register dup");
    assert!(registry.register_provider(mem_provider("dup", vec![])).is_err());
}

#[test]
fn get_returns_none_for_unknown_or_invalid_name() {
    let mut registry = SkillRegistry::new();
    registry.register(SkillRegistration {
        name: "known".to_string(),
        description: "d".to_string(),
        when_to_use: None,
        source: SkillSource::Runtime,
        resource_base: None,
        content: "b".to_string(),
        invocation: None,
        provider: None,
    });
    assert!(registry.get("unknown", None).is_none());
    assert!(registry.get("Bad_Name", None).is_none());
    assert!(registry.get("", None).is_none());
}

#[test]
fn on_change_notified_on_registration() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let mut registry = SkillRegistry::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_clone = calls.clone();
    registry.on_change(Arc::new(move || {
        calls_clone.fetch_add(1, Ordering::SeqCst);
    }));
    registry.register(SkillRegistration {
        name: "a".to_string(),
        description: "d".to_string(),
        when_to_use: None,
        source: SkillSource::Runtime,
        resource_base: None,
        content: "b".to_string(),
        invocation: None,
        provider: None,
    });
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
