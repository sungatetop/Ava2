//! 本地文件系统技能 provider。
//!
//! 核心语义：
//! - 目录 bundle 与 flat .md 两种形态：`<root>/skills/<name>/SKILL.md`（目录）与
//!   `<root>/skills/<name>.md`（flat）；
//! - YAML frontmatter 解析（`---` 包围块），必填 `name` / `description`；
//! - invocation policy 解析（`disable-model-invocation` / `user-invocable`）；
//! - provider 各 root 携带稳定 rank（project-onion 100 … bundled 600）。
//!
//! root 布局（project 级自 cwd 向上找 `.git` 定位 project root，找不到落 cwd 本身；
//! user 级为固定目录；目录不存在时静默跳过）：
//! - project：`.onion/skills`(100) → `.claude/skills`(110，claude code 兼容) →
//!   `.dsh/skills`(130，dsh 兼容) → `.agents/skills`(200，codex / 开放标准兼容)；
//! - user：`<ONION_HOME>/skills`(400) → `~/.claude/skills`(410) →
//!   `~/.dsh/skills`(430) → `~/.agents/skills`(500)。
//! 不做 chokidar 文件监听，底层变更由宿主调 `registry.refresh()` 触发。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::{
    is_skill_name, SkillCandidate, SkillDefinition, SkillInvocationPolicy, SkillResourceBase,
    SkillSource, SKILL_INSTRUCTION_FILE,
};
use super::registry::SkillProvider;

/// project 级 onion 自有根 rank。
const PROJECT_ONION_RANK: u32 = 100;

/// project 级 claude 兼容根 rank。
const PROJECT_CLAUDE_RANK: u32 = 110;

/// project 级 dsh 兼容根 rank。
const PROJECT_DSH_RANK: u32 = 130;

/// project 级共享 agent 根 rank（codex 仓库作用域）。
const PROJECT_AGENTS_RANK: u32 = 200;

/// 宿主挂载根 rank。
const CUSTOM_RANK: u32 = 300;

/// user 级 onion 自有根 rank。
const USER_ONION_RANK: u32 = 400;

/// user 级 claude 兼容根 rank。
const USER_CLAUDE_RANK: u32 = 410;

/// user 级 dsh 兼容根 rank。
const USER_DSH_RANK: u32 = 430;

/// user 级共享 agent 根 rank（codex 个人作用域）。
const USER_AGENTS_RANK: u32 = 500;

/// 一个来源 root 规格：project root 下的相对目录名 + 来源桶 + rank。
struct SourceRootSpec {
    /// project root 下的目录名（如 `.onion`）；或 user home 下的目录名。
    dir_name: &'static str,
    source: SkillSource,
    rank: u32,
}

/// project 级 roots 表（按 rank 优先序排列；`.agents/skills` 即 codex
/// 仓库作用域技能目录）。
const PROJECT_ROOT_SPECS: &[SourceRootSpec] = &[
    SourceRootSpec { dir_name: ".onion", source: SkillSource::ProjectOnion, rank: PROJECT_ONION_RANK },
    SourceRootSpec { dir_name: ".claude", source: SkillSource::ProjectClaude, rank: PROJECT_CLAUDE_RANK },
    SourceRootSpec { dir_name: ".dsh", source: SkillSource::ProjectDsh, rank: PROJECT_DSH_RANK },
    SourceRootSpec { dir_name: ".agents", source: SkillSource::ProjectAgents, rank: PROJECT_AGENTS_RANK },
];

/// user 级 roots 表（home 下的第三方兼容目录，`<ONION_HOME>` root 由装配注入）。
const USER_HOME_ROOT_SPECS: &[SourceRootSpec] = &[
    SourceRootSpec { dir_name: ".claude", source: SkillSource::UserClaude, rank: USER_CLAUDE_RANK },
    SourceRootSpec { dir_name: ".dsh", source: SkillSource::UserDsh, rank: USER_DSH_RANK },
    SourceRootSpec { dir_name: ".agents", source: SkillSource::UserAgents, rank: USER_AGENTS_RANK },
];

/// 一个技能 root：扫描目录 + 来源桶 + rank。
#[derive(Clone)]
struct SkillRoot {
    path: PathBuf,
    source: SkillSource,
    rank: u32,
}

/// 来源桶的缺省 rank（project 100 … bundled 600）。
fn rank_for_source(source: &SkillSource) -> u32 {
    match source {
        SkillSource::ProjectOnion => PROJECT_ONION_RANK,
        SkillSource::ProjectClaude => PROJECT_CLAUDE_RANK,
        SkillSource::ProjectDsh => PROJECT_DSH_RANK,
        SkillSource::ProjectAgents => PROJECT_AGENTS_RANK,
        SkillSource::Runtime => super::RUNTIME_RANK,
        SkillSource::UserOnion => USER_ONION_RANK,
        SkillSource::UserClaude => USER_CLAUDE_RANK,
        SkillSource::UserDsh => USER_DSH_RANK,
        SkillSource::UserAgents => USER_AGENTS_RANK,
        SkillSource::Custom => CUSTOM_RANK,
        SkillSource::Bundled => super::BUNDLED_SKILL_RANK,
    }
}

/// 本地文件系统 provider：扫描固定 roots，可选叠加 project 级 roots。
pub struct FileSystemSkillProvider {
    /// provider 名（缺省 `filesystem`）。
    name: String,
    /// 与 cwd 无关的固定 roots（user / custom 等）。
    fixed_roots: Vec<SkillRoot>,
    /// 是否按 cwd 发现 project 级 roots。
    project_roots: bool,
}

impl FileSystemSkillProvider {
    /// 固定单 root 构造（custom 挂载 / 单文件 provider 复用）。
    pub fn new(name: impl Into<String>, root: PathBuf, source: SkillSource) -> Self {
        Self {
            name: name.into(),
            fixed_roots: vec![SkillRoot { path: root, rank: rank_for_source(&source), source }],
            project_roots: false,
        }
    }

    /// 多固定 root 构造（user 级多来源兼容挂载）。
    pub fn with_fixed_roots(
        name: impl Into<String>,
        roots: Vec<(PathBuf, SkillSource)>,
    ) -> Self {
        Self {
            name: name.into(),
            fixed_roots: roots
                .into_iter()
                .map(|(path, source)| SkillRoot { rank: rank_for_source(&source), path, source })
                .collect(),
            project_roots: false,
        }
    }

    /// user root + project 级 root 发现（onion 装配默认形态：
    /// project → user 多层）。
    pub fn with_project_roots(name: impl Into<String>, root: PathBuf, source: SkillSource) -> Self {
        Self { project_roots: true, ..Self::new(name, root, source) }
    }

    /// user root + 多固定 root + project 级 root 发现（onion 装配形态：
    /// user onion 根 + home 级第三方兼容根 + project 级多层）。
    pub fn with_user_roots_and_project(
        name: impl Into<String>,
        user_root: PathBuf,
        extra_fixed_roots: Vec<(PathBuf, SkillSource)>,
    ) -> Self {
        let mut provider = Self::with_fixed_roots(
            name,
            [(user_root, SkillSource::UserOnion)].into_iter().chain(extra_fixed_roots).collect(),
        );
        provider.project_roots = true;
        provider
    }

    /// 组装本次 lookup 的全部 roots：project 级（按 cwd 解析）在前，固定 roots 在后
    /// （roots 顺序 project → custom → user；合并决胜由 rank 决定，顺序
    /// 只影响同 rank 平局）。
    fn roots_for(&self, cwd: Option<&Path>) -> Vec<SkillRoot> {
        let mut roots = Vec::new();
        if self.project_roots {
            if let Some(cwd) = cwd {
                let project_root = find_project_root(cwd);
                for spec in PROJECT_ROOT_SPECS {
                    roots.push(SkillRoot {
                        path: project_root.join(spec.dir_name).join("skills"),
                        source: spec.source.clone(),
                        rank: spec.rank,
                    });
                }
            }
        }
        roots.extend(self.fixed_roots.iter().cloned());
        roots
    }
}

/// 自 `cwd` 向上查找包含 `.git` 的最近祖先目录作为 project root；
/// 到文件系统根仍未找到则返回 `cwd` 本身。
///
/// `.git` 允许是目录（普通仓库）或文件（worktree / submodule）。
pub fn find_project_root(cwd: &Path) -> PathBuf {
    let mut current = cwd.to_path_buf();
    loop {
        if current.join(".git").exists() {
            return current;
        }
        match current.parent() {
            Some(parent) if parent != current => current = parent.to_path_buf(),
            _ => return cwd.to_path_buf(),
        }
    }
}

impl SkillProvider for FileSystemSkillProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn list(&self, cwd: Option<&Path>) -> Vec<SkillCandidate> {
        let mut candidates = Vec::new();
        for root in self.roots_for(cwd) {
            candidates.extend(discover_root(&root.path, &root.source, &self.name, root.rank));
        }
        candidates
    }

    fn get(&self, candidate: &SkillCandidate) -> Option<SkillDefinition> {
        let path = candidate.locator.clone()?;
        let parsed = parse_skill_file(&path)?;
        let directory = match &candidate.resource_base {
            Some(SkillResourceBase::Directory { path }) => path.clone(),
            _ => path.parent()?.to_path_buf(),
        };
        Some(SkillDefinition {
            path: Some(path),
            name: parsed.name,
            description: parsed.description,
            when_to_use: parsed.when_to_use,
            invocation: parsed.invocation,
            source: candidate.source.clone(),
            provider: self.name.clone(),
            resource_base: Some(SkillResourceBase::Directory { path: directory }),
            content: parsed.content,
        })
    }
}

/// 一次 root 扫描的候选列表。
pub fn discover_root(
    root: &Path,
    source: &SkillSource,
    provider: &str,
    rank: u32,
) -> Vec<SkillCandidate> {
    let mut candidates = Vec::new();
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return candidates,
    };
    let mut names: Vec<String> = Vec::new();
    for entry in entries.flatten() {
        names.push(entry.file_name().to_string_lossy().into_owned());
    }
    names.sort();
    for name in names {
        let path = root.join(&name);
        let (locator, directory) = match std::fs::metadata(&path) {
            Ok(meta) if meta.is_dir() => {
                (path.join(SKILL_INSTRUCTION_FILE), path)
            }
            Ok(meta) if meta.is_file() && name.ends_with(".md") => {
                (path, root.to_path_buf())
            }
            _ => continue,
        };
        let Some(parsed) = parse_skill_file(&locator) else {
            continue;
        };
        let skill_name = parsed.name;
        candidates.push(SkillCandidate {
            path: Some(locator.clone()),
            name: skill_name.clone(),
            description: parsed.description,
            when_to_use: parsed.when_to_use,
            invocation: parsed.invocation,
            source: source.clone(),
            provider: provider.to_string(),
            resource_base: Some(SkillResourceBase::Directory { path: directory }),
            rank,
            locator: Some(locator),
        });
        let _ = skill_name;
    }
    candidates
}

/// 解析后的技能文本。
pub struct ParsedSkill {
    name: String,
    description: String,
    when_to_use: Option<String>,
    invocation: SkillInvocationPolicy,
    content: String,
}

impl ParsedSkill {
    /// frontmatter `name`（已过 `is_skill_name` 校验）。
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// 解析单个技能文件：frontmatter + body。无 frontmatter / 非法名 / 缺字段 → None。
pub fn parse_skill_file(path: &Path) -> Option<ParsedSkill> {
    let raw = std::fs::read_to_string(path).ok()?;
    let (data, body) = parse_frontmatter(&raw)?;
    let name = string_field(&data, "name")?;
    let description = string_field(&data, "description")?;
    if !is_skill_name(&name) {
        return None;
    }
    let invocation = parse_invocation_policy(&data);
    Some(ParsedSkill {
        name,
        description,
        when_to_use: optional_string(&data, "whenToUse"),
        invocation,
        content: body.trim().to_string(),
    })
}

/// 解析 `---` 包围的 YAML frontmatter，返回 (data, body)。
/// 无 frontmatter / 解析失败 / 非映射 → None。
pub fn parse_frontmatter(raw: &str) -> Option<(serde_yaml::Mapping, String)> {
    let first_line_end = raw.find('\n')?;
    let first_line = raw[..first_line_end].trim_end_matches('\r');
    if first_line != "---" {
        return None;
    }
    let start = first_line_end + 1;
    let closing = find_closing_frontmatter(raw, start)?;
    let yaml = &raw[start..closing.start];
    let parsed: serde_yaml::Value = serde_yaml::from_str(yaml).ok()?;
    match parsed {
        serde_yaml::Value::Mapping(map) => {
            Some((map, raw[closing.body_start..].to_string()))
        }
        _ => None,
    }
}

fn find_closing_frontmatter(raw: &str, start: usize) -> Option<Closing> {
    let mut line_start = start;
    while line_start <= raw.len() {
        let next_newline = raw[line_start..].find('\n').map(|i| line_start + i);
        let line_end = next_newline.unwrap_or(raw.len());
        let line = raw[line_start..line_end].trim_end_matches('\r');
        if line == "---" {
            let body_start = next_newline.map(|i| i + 1).unwrap_or(raw.len());
            return Some(Closing { start: line_start, body_start });
        }
        let next = next_newline?;
        line_start = next + 1;
    }
    None
}

struct Closing {
    start: usize,
    body_start: usize,
}

/// 解析 invocation policy frontmatter：
/// `disable-model-invocation`（默认 false）与 `user-invocable`（默认 true）。
pub fn parse_invocation_policy(data: &serde_yaml::Mapping) -> SkillInvocationPolicy {
    let disable_model = frontmatter_bool(data, "disable-model-invocation").unwrap_or(false);
    let user_invocable = frontmatter_bool(data, "user-invocable").unwrap_or(true);
    SkillInvocationPolicy {
        model_invocable: !disable_model,
        user_invocable,
    }
}

fn string_field(data: &serde_yaml::Mapping, key: &str) -> Option<String> {
    match data.get(serde_yaml::Value::String(key.to_string())) {
        Some(serde_yaml::Value::String(value)) if !value.is_empty() => Some(value.clone()),
        _ => None,
    }
}

fn optional_string(data: &serde_yaml::Mapping, key: &str) -> Option<String> {
    string_field(data, key)
}

fn frontmatter_bool(data: &serde_yaml::Mapping, key: &str) -> Option<bool> {
    match data.get(serde_yaml::Value::String(key.to_string())) {
        Some(serde_yaml::Value::Bool(b)) => Some(*b),
        Some(serde_yaml::Value::Number(n)) => {
            if let Some(i) = n.as_i64() {
                match i {
                    1 => Some(true),
                    0 => Some(false),
                    _ => None,
                }
            } else {
                None
            }
        }
        Some(serde_yaml::Value::String(s)) => match s.to_lowercase().as_str() {
            "true" | "yes" | "on" => Some(true),
            "false" | "no" | "off" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

/// 便捷构造：filesystem provider（onion 装配用）——user onion 根 + home 级
/// 第三方兼容根（claude / dsh / agents）+ project 级 root 发现（按调用方 cwd 解析）。
pub fn filesystem_provider() -> Arc<dyn SkillProvider> {
    let user_root = super::skills_dir(&crate::paths::data_home());
    let extra_fixed_roots: Vec<(PathBuf, SkillSource)> = crate::paths::user_home()
        .map(|home| {
            USER_HOME_ROOT_SPECS
                .iter()
                .map(|spec| (home.join(spec.dir_name).join("skills"), spec.source.clone()))
                .collect()
        })
        .unwrap_or_default();
    Arc::new(FileSystemSkillProvider::with_user_roots_and_project(
        super::PROVIDER_FILESYSTEM,
        user_root,
        extra_fixed_roots,
    ))
}

#[cfg(test)]
#[path = "filesystem_tests.rs"]
mod tests;
