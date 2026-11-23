//! skill 领域模块：kebab-case 名校验 + 三态 resourceBase + invocation policy +
//! 分层注册表 + filesystem provider + skill-catalog 注入 + `/name` 用户手势。
//!
//! 三合一设计（用户明确要求不缩减）：
//! - `skill` 工具本体（`tools.rs`）加载技能指令并渲染 `<skill_content>`；
//! - 会话 `<available_skills>` 目录注入（`catalog.rs`）；
//! - `/name` 用户手势注入（`gesture.rs`）；
//! - `SkillRegistry` 分层注册表（`registry.rs`，provider/runtime + rank 决胜 + 缓存）；
//! - filesystem provider（`filesystem.rs`，目录 bundle + flat md + frontmatter 解析）。
//!
//! 语义要点：
//! - 技能名须匹配 kebab-case `^[a-z0-9]+(?:-[a-z0-9]+)*$`（防路径穿越）；
//! - `SkillResourceBase` 三态：`directory` / `url` / `opaque`；
//! - `SkillInvocationPolicy { modelInvocable, userInvocable }`；
//! - 技能是受信任的本地内容：body 原样嵌入（verbatim），仅 name 属性与 resource
//!   提示做伪 XML 转义（`escapeAttr` / `escapeText`）。

pub mod catalog;
pub mod disable;
pub mod filesystem;
pub mod gesture;
pub mod registry;
pub mod tools;

pub use disable::{
    manager_file_path, read_disabled_names, write_disabled_names, DisablePolicy, FileDisablePolicy,
};

use std::path::{Path, PathBuf};

/// 技能名语法：`^[a-z0-9]+(?:-[a-z0-9]+)*$`。
const SKILL_NAME: &str = r"^[a-z0-9]+(?:-[a-z0-9]+)*$";

/// 技能指令文件名（ava `skills/` 目录约定）。
pub const SKILL_INSTRUCTION_FILE: &str = "SKILL.md";

/// provider 标签（onion 固定 filesystem）。
pub const PROVIDER_FILESYSTEM: &str = "filesystem";

/// runtime provider 标签。
pub const PROVIDER_RUNTIME: &str = "runtime";

/// runtime 条目 rank。
pub const RUNTIME_RANK: u32 = 250;

/// 打包技能根 rank。
pub const BUNDLED_SKILL_RANK: u32 = 600;

/// 技能来源桶（值是可提示元数据，本身不决定优先序）。
///
/// 同层 rank 优先序：onion 自有目录 > claude > dsh > agents（codex / 开放标准）。
/// `ProjectDsh` / `UserDsh` 是第三方兼容桶，指向 dsh 工具的技能目录；
/// onion 自有目录使用 `ProjectOnion` / `UserOnion`。
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SkillSource {
    /// `<projectRoot>/.onion/skills`（onion 自有，rank 100）。
    ProjectOnion,
    /// `<projectRoot>/.claude/skills`（claude code 兼容，rank 110）。
    ProjectClaude,
    /// `<projectRoot>/.dsh/skills`（dsh 兼容，rank 130）。
    ProjectDsh,
    /// `<projectRoot>/.agents/skills`（codex / 开放 agent skills 标准，rank 200）。
    ProjectAgents,
    Runtime,
    Custom,
    /// `<ONION_HOME>/skills`（onion 自有，rank 400）。
    UserOnion,
    /// `~/.claude/skills`（claude code 兼容，rank 410）。
    UserClaude,
    /// `~/.dsh/skills`（dsh 兼容，rank 430）。
    UserDsh,
    /// `~/.agents/skills`（codex / 开放 agent skills 标准，rank 500）。
    UserAgents,
    Bundled,
}

impl SkillSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            SkillSource::ProjectOnion => "project-onion",
            SkillSource::ProjectClaude => "project-claude",
            SkillSource::ProjectDsh => "project-dsh",
            SkillSource::ProjectAgents => "project-agents",
            SkillSource::Runtime => "runtime",
            SkillSource::Custom => "custom",
            SkillSource::UserOnion => "user-onion",
            SkillSource::UserClaude => "user-claude",
            SkillSource::UserDsh => "user-dsh",
            SkillSource::UserAgents => "user-agents",
            SkillSource::Bundled => "bundled",
        }
    }
}

/// 可选 provider-specific 基准，用于加载的技能 body 解析相对资源（三态 oneOf）。
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SkillResourceBase {
    Directory { path: PathBuf },
    Url { url: String },
    Opaque { description: String },
}

/// 技能发现消费者共享的调用控制。
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SkillInvocationPolicy {
    /// 模型面目录与加载器是否包含此技能。
    pub model_invocable: bool,
    /// 人工面命令目录与加载器是否包含此技能。
    pub user_invocable: bool,
}

impl Default for SkillInvocationPolicy {
    fn default() -> Self {
        Self { model_invocable: true, user_invocable: true }
    }
}

/// 列表返回的调用中立元数据。
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SkillSummary {
    /// 指令文件绝对路径（provider 提供时；虚拟技能缺失）。
    pub path: Option<PathBuf>,
    /// kebab-case 技能标识。
    pub name: String,
    /// 发现消费者展示的短路由描述。
    pub description: String,
    /// 可选额外路由引导。
    pub when_to_use: Option<String>,
    /// 解析后的模型与人工调用控制。
    pub invocation: SkillInvocationPolicy,
    /// 产出此获胜技能的来源。
    pub source: SkillSource,
    /// 拥有此技能 body 的 provider。
    pub provider: String,
    /// 相对资源 provider-specific 基准（可选）。
    pub resource_base: Option<SkillResourceBase>,
}

/// 完整解析的技能定义，含 `get()` 加载的 body。
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SkillDefinition {
    pub path: Option<PathBuf>,
    pub name: String,
    pub description: String,
    pub when_to_use: Option<String>,
    pub invocation: SkillInvocationPolicy,
    pub source: SkillSource,
    pub provider: String,
    pub resource_base: Option<SkillResourceBase>,
    /// 去除 provider-specific 元数据后的 markdown 指令 body。
    pub content: String,
}

/// provider 目录条目：注册表合并、后加载用。
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SkillCandidate {
    pub path: Option<PathBuf>,
    pub name: String,
    pub description: String,
    pub when_to_use: Option<String>,
    pub invocation: SkillInvocationPolicy,
    pub source: SkillSource,
    pub provider: String,
    pub resource_base: Option<SkillResourceBase>,
    /// 同层重名决胜 rank（低者胜）。
    pub rank: u32,
    /// provider-owned 定位句柄（回传 `provider.get()`）。
    pub locator: Option<std::path::PathBuf>,
}

/// runtime 注册输入（缺省 invocation/provider 补默认）。
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SkillRegistration {
    pub name: String,
    pub description: String,
    pub when_to_use: Option<String>,
    pub source: SkillSource,
    pub resource_base: Option<SkillResourceBase>,
    pub content: String,
    pub invocation: Option<SkillInvocationPolicy>,
    pub provider: Option<String>,
}

/// 校验技能名是否匹配 kebab-case 语法。
pub fn is_skill_name(name: &str) -> bool {
    let re = match regex::Regex::new(SKILL_NAME) {
        Ok(re) => re,
        Err(_) => return false,
    };
    re.is_match(name)
}

/// 该技能是否可被模型广告并加载。
pub fn is_model_invocable(policy: &SkillInvocationPolicy) -> bool {
    policy.model_invocable
}

/// 该技能是否可被人工面命令广告并加载。
pub fn is_user_invocable(policy: &SkillInvocationPolicy) -> bool {
    policy.user_invocable
}

/// 技能目录根：`<root>/skills`（对齐 ava 全局 `skills/` 目录约定）。
pub fn skills_dir(root: &Path) -> PathBuf {
    root.join("skills")
}

/// 单个技能的指令文件：`<root>/skills/<name>/SKILL.md`。
/// 名字先经 kebab-case 校验——非法名返回 None，杜绝路径穿越。
pub fn skill_dir(root: &Path, name: &str) -> Option<PathBuf> {
    if !is_skill_name(name) {
        return None;
    }
    Some(skills_dir(root).join(name))
}

/// 单个技能的指令文件：`<root>/skills/<name>/SKILL.md`（名字须已校验合法）。
pub fn skill_instruction_file(root: &Path, name: &str) -> Option<PathBuf> {
    skill_dir(root, name).map(|dir| dir.join(SKILL_INSTRUCTION_FILE))
}

/// 伪 XML 属性转义：`&` → `&amp;`、`"` → `&quot;`、`<` → `&lt;`。
pub fn escape_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
}

/// 伪 XML 文本转义：`&` → `&amp;`、`<` → `&lt;`、`>` → `&gt;`。
pub fn escape_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// 渲染 `<skill_content>` 块（name 属性转义、body
/// verbatim、resource 提示经 escapeText）。resourceBase 三态经 `render_resource_hint`。
pub fn render_skill_content(skill: &SkillDefinition) -> String {
    [
        format!("<skill_content name=\"{}\">", escape_attr(&skill.name)),
        "<skill_resources>".to_string(),
        render_resource_hint(skill),
        "</skill_resources>".to_string(),
        String::new(),
        "<skill_instructions>".to_string(),
        skill.content.clone(),
        "</skill_instructions>".to_string(),
        "</skill_content>".to_string(),
    ]
    .join("\n")
}

/// 渲染 resource 提示（四分支：directory / url /
/// opaque / provider-managed）。
fn render_resource_hint(skill: &SkillDefinition) -> String {
    match &skill.resource_base {
        None => [
            format!("Resources for this skill are managed by provider \"{}\".", escape_text(&skill.provider)),
            "Load referenced resources only as needed.".to_string(),
        ]
        .join("\n"),
        Some(SkillResourceBase::Directory { path }) => [
            format!("Base directory for this skill: {}", escape_text(&path.display().to_string())),
            "Resolve relative paths mentioned by this skill against the base directory before using them. Load referenced resources only as needed."
                .to_string(),
        ]
        .join("\n"),
        Some(SkillResourceBase::Url { url }) => [
            format!("Base URL for this skill: {}", escape_text(url)),
            "Resolve relative URLs mentioned by this skill against the base URL before using them. Load referenced resources only as needed."
                .to_string(),
        ]
        .join("\n"),
        Some(SkillResourceBase::Opaque { description }) => [
            format!("Resources for this skill: {}", escape_text(description)),
            "Load referenced resources only as needed.".to_string(),
        ]
        .join("\n"),
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
