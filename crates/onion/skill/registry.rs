//! 分层技能注册表。
//!
//! 核心语义：
//! - provider 注册（`register_provider`）进调用方 scope 的层；runtime 注册
//!   （`register`）进同层 runtime 表；重复名同层 first-wins；
//! - 读取合并 global 层 + scope chain（最内层同名胜出），同层内 rank → provider
//!   order → local order 决胜；
//! - collect 缓存（`(cwd, scope chain, revision)` 为 key）+ `skills/change` 事件
//!   + digest 去重；
//! - `list` 返回排序后的调用中立 summaries；`get` 加载获胜 candidate 的完整 body。
//!
//! onion 教学实现将 scope chain 简化为单层 global（无 agent preset 分层），但保留
//! 分层合并骨架与 rank 决胜语义。

use std::sync::Arc;

use crate::skill::{
    is_skill_name, DisablePolicy, SkillCandidate, SkillDefinition, SkillInvocationPolicy,
    SkillRegistration, SkillSummary,
};

/// runtime 条目 rank。
const RUNTIME_RANK: u32 = 250;

/// collect 缓存最大条目数。
const DEFAULT_COLLECT_CACHE_ENTRIES: usize = 128;

/// 一次 collect 的 revision 竞争重试上限。
const MAX_COLLECT_ATTEMPTS: u32 = 2;

/// provider 接口：一个技能来源（本地目录 / 远程注册表）。
pub trait SkillProvider: Send + Sync {
    /// 注册表内的唯一 provider 名。
    fn name(&self) -> &str;
    /// 列出当前 lookup 上下文可用的技能 candidate；`cwd` 供 project 级 root
    /// 发现使用。
    fn list(&self, cwd: Option<&std::path::Path>) -> Vec<SkillCandidate>;
    /// 加载此前列出的 candidate 的完整 body；不可加载返回 None。
    fn get(&self, candidate: &SkillCandidate) -> Option<SkillDefinition>;
}

/// 注册表变更通知回调（`skills/change` 语义）。
type ChangeCallback = Arc<dyn Fn() + Send + Sync>;

/// 一条已索引 candidate（含决胜用的 provider/local order）。
#[derive(Clone)]
struct IndexedCandidate {
    candidate: SkillCandidate,
    provider: Arc<dyn SkillProvider>,
    provider_order: usize,
    local_order: usize,
}

/// 分层技能注册表（onion 单层 global 教学实现）。
pub struct SkillRegistry {
    /// 已注册 provider（insertion-ordered）。
    providers: Vec<(String, Arc<dyn SkillProvider>, usize)>,
    /// runtime 注册的技能（按名）。
    runtime: std::collections::HashMap<String, SkillDefinition>,
    /// collect 缓存（key = (cwd, revision)）。
    collect_cache: std::collections::HashMap<String, std::collections::HashMap<String, IndexedCandidate>>,
    revision: u64,
    next_provider_order: usize,
    /// `skills/change` 通知回调（可多个）。
    change_callbacks: std::sync::RwLock<Vec<ChangeCallback>>,
    /// 禁用策略（可选）：接线后 `list` / `get` / `summary` 按名过滤禁用技能。
    disable_policy: Option<Arc<dyn DisablePolicy>>,
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            runtime: std::collections::HashMap::new(),
            collect_cache: std::collections::HashMap::new(),
            revision: 0,
            next_provider_order: 0,
            change_callbacks: std::sync::RwLock::new(Vec::new()),
            disable_policy: None,
        }
    }

    /// 接线禁用策略（builder 形态）：接线后 list 不含禁用技能、get/summary
    /// 命中禁用返回 None；未接线时行为与现状一致。
    pub fn with_disable_policy(mut self, policy: Arc<dyn DisablePolicy>) -> Self {
        self.disable_policy = Some(policy);
        self
    }

    /// 当前 cwd 的禁用名集合（无策略时空集）。
    fn disabled_names(&self, cwd: Option<&std::path::Path>) -> std::collections::HashSet<String> {
        match &self.disable_policy {
            Some(policy) => policy.disabled_names(cwd),
            None => std::collections::HashSet::new(),
        }
    }

    /// 注册一个 provider（onion 单层 global）。重复名（含 runtime 保留名）拒绝。
    pub fn register_provider(&mut self, provider: Arc<dyn SkillProvider>) -> Result<(), String> {
        let name = provider.name().to_string();
        if name == crate::skill::PROVIDER_RUNTIME {
            return Err(format!("\"{name}\" is reserved for runtime skill registrations"));
        }
        if self.providers.iter().any(|(n, _, _)| n == &name) {
            return Err(format!("a skill provider named \"{name}\" is already registered"));
        }
        let order = self.next_provider_order;
        self.next_provider_order += 1;
        self.providers.push((name, provider, order));
        self.invalidate();
        Ok(())
    }

    /// 注册一个 runtime 技能。同层重名 first-wins（忽略）。
    pub fn register(&mut self, skill: SkillRegistration) {
        if !is_skill_name(&skill.name) {
            return;
        }
        if skill.description.is_empty() {
            return;
        }
        if self.runtime.contains_key(&skill.name) {
            tracing::warn!(name = %skill.name, "runtime skill ignored because it is already registered");
            return;
        }
        let definition = SkillDefinition {
            path: None,
            name: skill.name.clone(),
            description: skill.description,
            when_to_use: skill.when_to_use,
            invocation: skill.invocation.unwrap_or_default(),
            source: skill.source,
            provider: skill.provider.unwrap_or_else(|| crate::skill::PROVIDER_RUNTIME.to_string()),
            resource_base: skill.resource_base,
            content: skill.content,
        };
        self.runtime.insert(definition.name.clone(), definition);
        self.invalidate();
    }

    /// 注册 `skills/change` 通知回调。
    pub fn on_change(&self, callback: ChangeCallback) {
        self.change_callbacks
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(callback);
    }

    /// 强制刷新 collect 缓存并广播 `skills/change`（provider 底层变更后由宿主调用，
    /// provider 变更 → registry 缓存失效）。
    pub fn refresh(&mut self) {
        self.invalidate();
    }

    /// 列出调用中立的获胜 summaries（按名排序；禁用技能不出现）。
    pub fn list(&mut self, cwd: Option<&std::path::Path>) -> Vec<SkillSummary> {
        let disabled = self.disabled_names(cwd);
        let mut summaries = self.list_all(cwd);
        summaries.retain(|summary| !disabled.contains(&summary.name));
        summaries
    }

    /// 列出全部获胜 summaries（按名排序；**含禁用技能**）——skills-manager
    /// 管理面读视图用，禁用仅体现为 enabled 标记而非列表缺席。
    pub fn list_all(&mut self, cwd: Option<&std::path::Path>) -> Vec<SkillSummary> {
        let mut summaries: Vec<SkillSummary> = self
            .collect(cwd)
            .into_values()
            .map(|entry| to_summary(&entry.candidate))
            .collect();
        summaries.sort_by(|a, b| a.name.cmp(&b.name));
        summaries
    }

    /// 加载指定技能的完整 body；未知 / 非法 / 禁用名返回 None。
    pub fn get(&mut self, name: &str, cwd: Option<&std::path::Path>) -> Option<SkillDefinition> {
        if !is_skill_name(name) || self.disabled_names(cwd).contains(name) {
            return None;
        }
        let collected = self.collect(cwd);
        let entry = collected.get(name)?;
        let definition = entry.provider.get(&entry.candidate)?;
        if definition.name == entry.candidate.name {
            Some(definition)
        } else {
            None
        }
    }

    /// 按名查 summary（`skill` 工具用）；未知 / 禁用名返回 None。
    pub fn summary(&mut self, name: &str, cwd: Option<&std::path::Path>) -> Option<SkillSummary> {
        if self.disabled_names(cwd).contains(name) {
            return None;
        }
        let collected = self.collect(cwd);
        let entry = collected.get(name)?;
        Some(to_summary(&entry.candidate))
    }

    /// 收集并合并全部层（onion 单层 global + runtime）。
    fn collect(
        &mut self,
        cwd: Option<&std::path::Path>,
    ) -> std::collections::HashMap<String, IndexedCandidate> {
        let key = self.cache_key(cwd);
        if let Some(cached) = self.collect_cache.get(&key) {
            return cached.clone();
        }
        let mut merged: std::collections::HashMap<String, IndexedCandidate> =
            std::collections::HashMap::new();
        // 统一收集 runtime + provider 全部候选，按 (rank → provider order → local order)
        // 排序，first-wins 按名去重（RUNTIME_RANK=250 与 provider rank 同场决胜）。
        let mut all: Vec<IndexedCandidate> = Vec::new();
        // runtime 条目（provider = runtime 兜底 get）。
        let mut runtime_order = 0usize;
        let mut runtime_names: Vec<String> = self.runtime.keys().cloned().collect();
        runtime_names.sort();
        for name in runtime_names {
            let Some(definition) = self.runtime.get(&name) else {
                continue;
            };
            let candidate = SkillCandidate {
                path: None,
                name: definition.name.clone(),
                description: definition.description.clone(),
                when_to_use: definition.when_to_use.clone(),
                invocation: definition.invocation.clone(),
                source: definition.source.clone(),
                provider: definition.provider.clone(),
                resource_base: definition.resource_base.clone(),
                rank: RUNTIME_RANK,
                locator: None,
            };
            let provider = Arc::new(RuntimeSkillProvider { definition: definition.clone() });
            all.push(IndexedCandidate {
                candidate,
                provider,
                provider_order: usize::MAX,
                local_order: runtime_order,
            });
            runtime_order += 1;
        }
        // provider 条目。
        for (_, provider, order) in &self.providers {
            for (local_order, candidate) in provider.list(cwd).into_iter().enumerate() {
                all.push(IndexedCandidate {
                    candidate,
                    provider: provider.clone(),
                    provider_order: *order,
                    local_order,
                });
            }
        }
        all.sort_by(|a, b| {
            a.candidate
                .rank
                .cmp(&b.candidate.rank)
                .then(a.provider_order.cmp(&b.provider_order))
                .then(a.local_order.cmp(&b.local_order))
        });
        let mut seen = std::collections::HashSet::new();
        for entry in all {
            if seen.insert(entry.candidate.name.clone()) {
                merged.insert(entry.candidate.name.clone(), entry);
            }
        }
        if self.collect_cache.len() >= DEFAULT_COLLECT_CACHE_ENTRIES {
            self.collect_cache.clear();
        }
        self
            .collect_cache
            .insert(key, merged.clone());
        merged
    }

    fn cache_key(&self, cwd: Option<&std::path::Path>) -> String {
        let cwd = cwd.map(|p| p.display().to_string()).unwrap_or_default();
        format!("{cwd}#{}", self.revision)
    }

    /// 失效缓存并通知 `skills/change`。
    fn invalidate(&mut self) {
        self.revision += 1;
        self.collect_cache.clear();
        let callbacks = self
            .change_callbacks
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        for callback in callbacks {
            callback();
        }
    }
}

/// runtime 条目对应的 provider（仅拥有 `get()`；list 由 registry 直接注入）。
struct RuntimeSkillProvider {
    definition: SkillDefinition,
}

impl SkillProvider for RuntimeSkillProvider {
    fn name(&self) -> &str {
        crate::skill::PROVIDER_RUNTIME
    }

    fn list(&self, _cwd: Option<&std::path::Path>) -> Vec<SkillCandidate> {
        Vec::new()
    }

    fn get(&self, _candidate: &SkillCandidate) -> Option<SkillDefinition> {
        Some(self.definition.clone())
    }
}

fn to_summary(candidate: &SkillCandidate) -> SkillSummary {
    SkillSummary {
        path: candidate.path.clone(),
        name: candidate.name.clone(),
        description: candidate.description.clone(),
        when_to_use: candidate.when_to_use.clone(),
        invocation: candidate.invocation.clone(),
        source: candidate.source.clone(),
        provider: candidate.provider.clone(),
        resource_base: candidate.resource_base.clone(),
    }
}

/// 便捷构造：默认 invocation（对齐 `SkillInvocationPolicy` 缺省）。
pub fn default_invocation() -> SkillInvocationPolicy {
    SkillInvocationPolicy::default()
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
