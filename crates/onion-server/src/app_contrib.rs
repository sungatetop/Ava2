//! App 能力贡献装配：把 `<apps_dir>/<id>/manifest.json` 的
//! `contributes.{skills,mcpServers,hooks}` 解析成会话装配期可直接消费的
//! 额外来源（技能 provider / MCP server 配置 / Claude Code 兼容 hooks 配置）。
//!
//! `contributes.apps`（UI 多页）由前端宿主消费，`contributes.agents` 暂不装配，
//! 均不在本模块职责内。所有声明路径都限制在 App 根目录内（拒绝绝对路径与 `..`），
//! 与安装校验（`rest::apps::validate_manifest`）同口径。
//!
//! 只在**已启用**的 App 上生效，且每次会话装配时重新扫描登记表，
//! 因此安装 / 启停后新会话即时拿到最新贡献。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use onion::hooks::config::SubstitutionVars;
use onion::mcp_resources::{self, McpServerConfig};
use onion::skill::filesystem::FileSystemSkillProvider;
use onion::skill::registry::SkillProvider;
use onion::skill::{SkillCandidate, SkillDefinition, SkillSource};
use serde_json::{Map, Value};

/// 一次会话装配所需的全部 App 贡献。
#[derive(Default)]
pub(crate) struct AppContributions {
    /// 技能 provider（每声明一项技能目录 / 文件一条，provider 名 `app:<id>:<序号>`）
    pub skills: Vec<Arc<dyn SkillProvider>>,
    /// MCP server 配置（就地解析；与既有配置重名时由装配侧跳过）
    pub mcp: Vec<McpServerConfig>,
    /// hooks 配置源（变量替换需要会话根目录，故延后到装配期）
    pub hooks: Vec<HookSource>,
    /// 装配提示（非法项 / 跳过项），由装配侧打日志
    pub notes: Vec<String>,
}

/// 单个 App 的 hooks 配置源（保留归属目录，供 `${CLAUDE_PLUGIN_ROOT}` 替换）。
pub(crate) struct HookSource {
    pub dir: PathBuf,
    pub raw: Value,
}

/// App 安装根目录：`ONION_APPS_DIR` 覆盖，缺省 `<data_home>/apps`。
pub(crate) fn apps_dir() -> PathBuf {
    std::env::var("ONION_APPS_DIR")
        .ok()
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| onion::paths::data_home().join("apps"))
}

/// 读取全部已启用 App 的能力贡献。
pub(crate) fn load() -> AppContributions {
    load_from(&apps_dir())
}

fn load_from(apps_dir: &Path) -> AppContributions {
    let mut out = AppContributions::default();
    for (app_id, dir) in crate::rest::apps::enabled_app_dirs(apps_dir) {
        let manifest = match crate::rest::apps::read_manifest(&dir) {
            Ok(manifest) => manifest,
            Err(_) => continue,
        };
        let Some(contributes) = contributes_of(&manifest) else {
            continue;
        };
        collect_skills(&app_id, &dir, contributes.get("skills"), &mut out);
        collect_mcp(&app_id, &dir, contributes.get("mcpServers"), &mut out);
        collect_hooks(&app_id, &dir, contributes.get("hooks"), &mut out);
    }
    out
}

/// `contributes` 对象（hostApi≥2 的能力贡献声明）。
fn contributes_of(manifest: &Value) -> Option<&Map<String, Value>> {
    manifest.get("contributes").and_then(Value::as_object)
}

// ---- 技能 ----

fn collect_skills(app_id: &str, dir: &Path, raw: Option<&Value>, out: &mut AppContributions) {
    let Some(raw) = raw else {
        return;
    };
    for (index, rel) in declared_paths(raw).into_iter().enumerate() {
        let Some(path) = resolve_inside(dir, rel) else {
            out.notes.push(format!(
                "App `{app_id}` 技能路径 `{rel}` 非法（须为 App 根目录内相对路径）"
            ));
            continue;
        };
        let name = format!("app:{app_id}:{index}");
        if path.is_dir() {
            out.skills
                .push(Arc::new(FileSystemSkillProvider::new(name, path, SkillSource::Custom)));
        } else if path.is_file() {
            out.skills.push(Arc::new(SingleFileSkillProvider::new(path, name)));
        } else {
            out.notes
                .push(format!("App `{app_id}` 技能路径 `{rel}` 不存在，已跳过"));
        }
    }
}

/// 单文件技能：复用目录 provider 的解析逻辑，只放行声明的那一个 `.md` 文件
/// （避免把同目录里未声明的技能一并暴露）。
struct SingleFileSkillProvider {
    name: String,
    file: PathBuf,
    inner: FileSystemSkillProvider,
}

impl SingleFileSkillProvider {
    fn new(file: PathBuf, name: String) -> Self {
        let root = file.parent().map(Path::to_path_buf).unwrap_or_else(|| file.clone());
        Self {
            inner: FileSystemSkillProvider::new(name.clone(), root, SkillSource::Custom),
            name,
            file,
        }
    }
}

impl SkillProvider for SingleFileSkillProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn list(&self, cwd: Option<&Path>) -> Vec<SkillCandidate> {
        self.inner
            .list(cwd)
            .into_iter()
            .filter(|candidate| candidate.path.as_deref() == Some(self.file.as_path()))
            .collect()
    }

    fn get(&self, candidate: &SkillCandidate) -> Option<SkillDefinition> {
        self.inner.get(candidate)
    }
}

// ---- MCP ----

fn collect_mcp(app_id: &str, dir: &Path, raw: Option<&Value>, out: &mut AppContributions) {
    let Some(raw) = raw else {
        return;
    };
    // Claude Code 风格变量替换：`${CLAUDE_PLUGIN_ROOT}` → App 根目录（command/args/env/cwd 均可引用）。
    let vars = SubstitutionVars {
        plugin_root: Some(dir.to_string_lossy().into_owned()),
        project_dir: None,
    };
    let substituted = substitute(raw, &vars);
    let raw = &substituted;
    // `{ "path": "mcp.json" }`：外部配置文件，格式与 `<data_home>/mcp.json` 一致。
    if let Some(rel) = raw.get("path").and_then(Value::as_str) {
        let Some(path) = resolve_inside(dir, rel) else {
            out.notes.push(format!(
                "App `{app_id}` MCP 配置路径 `{rel}` 非法（须为 App 根目录内相对路径）"
            ));
            return;
        };
        match mcp_resources::load_servers(&path) {
            Ok(servers) => out.mcp.extend(servers),
            Err(error) => out
                .notes
                .push(format!("App `{app_id}` MCP 配置 `{rel}` 无效: {error}")),
        }
        return;
    }
    let mut servers = Vec::new();
    for item in as_items(raw) {
        match mcp_entry(item) {
            Ok(server) => servers.push(server),
            Err(error) => out.notes.push(format!("App `{app_id}` {error}")),
        }
    }
    if servers.is_empty() {
        return;
    }
    match mcp_resources::parse_servers(&serde_json::json!({ "servers": servers })) {
        Ok(parsed) => out.mcp.extend(parsed),
        Err(error) => out
            .notes
            .push(format!("App `{app_id}` MCP 配置无效: {error}")),
    }
}

/// 内联 MCP 声明（`{name, command|url, args?, env?, headers?}`）→
/// `parse_servers` 认识的 `{transport, serverName, ...}` 形态。
fn mcp_entry(item: &Value) -> Result<Value, String> {
    let name = item
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| "mcpServers 每项必须有非空 name".to_string())?;
    let mut server = Map::new();
    server.insert("serverName".into(), Value::String(name.to_string()));
    if let Some(url) = item.get("url").and_then(Value::as_str).filter(|url| !url.is_empty()) {
        server.insert("transport".into(), Value::String("streamable-http".into()));
        server.insert("url".into(), Value::String(url.to_string()));
        if let Some(headers) = item.get("headers") {
            server.insert("headers".into(), headers.clone());
        }
    } else {
        let command = item
            .get("command")
            .and_then(Value::as_str)
            .filter(|command| !command.is_empty())
            .ok_or_else(|| format!("MCP server `{name}` 需要 command 或 url"))?;
        server.insert("transport".into(), Value::String("stdio".into()));
        server.insert("command".into(), Value::String(command.to_string()));
        for key in ["args", "env", "cwd"] {
            if let Some(value) = item.get(key) {
                server.insert(key.into(), value.clone());
            }
        }
    }
    if let Some(timeout) = item.get("toolCallTimeoutMs") {
        server.insert("toolCallTimeoutMs".into(), timeout.clone());
    }
    Ok(Value::Object(server))
}

// ---- hooks ----

fn collect_hooks(app_id: &str, dir: &Path, raw: Option<&Value>, out: &mut AppContributions) {
    let Some(raw) = raw else {
        return;
    };
    // `{ "path": "hooks.json" }`：文件已是 Claude Code 兼容格式（`{hooks:{...}}` 或裸事件 map）。
    if let Some(rel) = raw.get("path").and_then(Value::as_str) {
        let Some(path) = resolve_inside(dir, rel) else {
            out.notes.push(format!(
                "App `{app_id}` hooks 配置路径 `{rel}` 非法（须为 App 根目录内相对路径）"
            ));
            return;
        };
        match read_json(&path) {
            Ok(value) => out.hooks.push(HookSource {
                dir: dir.to_path_buf(),
                raw: value,
            }),
            Err(error) => out
                .notes
                .push(format!("App `{app_id}` hooks 配置 `{rel}` 无效: {error}")),
        }
        return;
    }
    // 内联声明：[{ event, matcher?, handler: { type: "command"|"bus", command? } }]
    let mut events: Map<String, Value> = Map::new();
    for item in as_items(raw) {
        let event = item.get("event").and_then(Value::as_str).unwrap_or("");
        if event.is_empty() {
            out.notes
                .push(format!("App `{app_id}` hooks 项缺少 event，已跳过"));
            continue;
        }
        let handler = item.get("handler").cloned().unwrap_or(Value::Null);
        let kind = handler.get("type").and_then(Value::as_str).unwrap_or("");
        let hook = match kind {
            "command" => {
                let command = handler.get("command").and_then(Value::as_str).unwrap_or("");
                if command.is_empty() {
                    out.notes.push(format!(
                        "App `{app_id}` 事件 `{event}` 的 command 处理器缺少 command，已跳过"
                    ));
                    continue;
                }
                serde_json::json!({ "type": "command", "command": command })
            }
            // 前端回调需要宿主把调用转发给 App 并等待答复，边车路径暂不支持。
            "bus" => {
                out.notes.push(format!(
                    "App `{app_id}` 事件 `{event}` 的 bus 处理器暂不支持，已跳过"
                ));
                continue;
            }
            other => {
                out.notes.push(format!(
                    "App `{app_id}` 事件 `{event}` 的处理器类型 `{other}` 不可识别，已跳过"
                ));
                continue;
            }
        };
        let mut group = Map::new();
        if let Some(matcher) = item.get("matcher").and_then(Value::as_str) {
            if !matcher.is_empty() {
                group.insert("matcher".into(), Value::String(matcher.to_string()));
            }
        }
        group.insert("hooks".into(), Value::Array(vec![hook]));
        let slot = events
            .entry(event.to_string())
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Some(list) = slot.as_array_mut() {
            list.push(Value::Object(group));
        }
    }
    if !events.is_empty() {
        out.hooks.push(HookSource {
            dir: dir.to_path_buf(),
            raw: serde_json::json!({ "hooks": events }),
        });
    }
}

/// 合并全部 App 的 hooks 配置为单份 Claude Code 兼容 JSON：
/// `${CLAUDE_PLUGIN_ROOT}` 按各 App 根目录替换，`${CLAUDE_PROJECT_DIR}` 按会话根目录替换。
impl AppContributions {
    pub(crate) fn hooks_config(&self, project_dir: &Path) -> Option<Value> {
        let mut merged: Map<String, Value> = Map::new();
        for source in &self.hooks {
            let vars = SubstitutionVars {
                plugin_root: Some(source.dir.display().to_string()),
                project_dir: Some(project_dir.display().to_string()),
            };
            let substituted = substitute(&source.raw, &vars);
            let Some(map) = substituted.get("hooks").and_then(Value::as_object) else {
                continue;
            };
            for (event, groups) in map {
                let slot = merged
                    .entry(event.clone())
                    .or_insert_with(|| Value::Array(Vec::new()));
                if let (Some(slot), Some(groups)) = (slot.as_array_mut(), groups.as_array()) {
                    slot.extend(groups.iter().cloned());
                }
            }
        }
        if merged.is_empty() {
            None
        } else {
            Some(serde_json::json!({ "hooks": merged }))
        }
    }
}

/// 递归替换 JSON 内所有字符串的 hook 变量（事件名 / matcher / command 均为字符串）。
fn substitute(value: &Value, vars: &SubstitutionVars) -> Value {
    match value {
        Value::String(text) => Value::String(onion::hooks::config::substitute_command(text, vars)),
        Value::Array(items) => Value::Array(items.iter().map(|item| substitute(item, vars)).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| (key.clone(), substitute(value, vars)))
                .collect(),
        ),
        other => other.clone(),
    }
}

// ---- 安装期校验 ----

/// 安装期校验能力贡献：声明路径必须在 App 根目录内且存在，MCP / hooks 配置可解析。
/// `bus` 处理器不视为错误（前端宿主支持，边车装配期跳过并提示）。
pub(crate) fn validate(manifest: &Value, dir: &Path) -> Result<(), String> {
    let Some(contributes) = contributes_of(manifest) else {
        return Ok(());
    };
    let mut problems: Vec<String> = Vec::new();

    for rel in declared_paths(contributes.get("skills").unwrap_or(&Value::Null)) {
        match resolve_inside(dir, rel) {
            Some(path) if path.exists() => {}
            Some(_) => problems.push(format!("skills 路径 `{rel}` 不存在")),
            None => problems.push(format!("skills 路径 `{rel}` 必须是 App 根目录内相对路径")),
        }
    }

    if let Some(raw) = contributes.get("mcpServers") {
        if let Some(rel) = raw.get("path").and_then(Value::as_str) {
            match resolve_inside(dir, rel) {
                Some(path) => {
                    if let Err(error) = mcp_resources::load_servers(&path) {
                        problems.push(format!("mcpServers `{rel}` 无效: {error}"));
                    }
                }
                None => problems.push(format!("mcpServers 路径 `{rel}` 必须是 App 根目录内相对路径")),
            }
        } else {
            for item in as_items(raw) {
                if let Err(error) = mcp_entry(item) {
                    problems.push(error);
                }
            }
        }
    }

    if let Some(raw) = contributes.get("hooks") {
        if let Some(rel) = raw.get("path").and_then(Value::as_str) {
            match resolve_inside(dir, rel) {
                Some(path) => {
                    if let Err(error) = read_json(&path) {
                        problems.push(format!("hooks `{rel}` 无效: {error}"));
                    }
                }
                None => problems.push(format!("hooks 路径 `{rel}` 必须是 App 根目录内相对路径")),
            }
        } else {
            for item in as_items(raw) {
                let event = item.get("event").and_then(Value::as_str).unwrap_or("");
                if event.is_empty() {
                    problems.push("hooks 每项必须有 event".to_string());
                }
            }
        }
    }

    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("; "))
    }
}

// ---- 通用助手 ----

/// 声明路径列表：接受 `["skills/"]`、`[{ "path": "skills/" }]` 或单个同形值。
fn declared_paths(raw: &Value) -> Vec<&str> {
    as_items(raw)
        .into_iter()
        .filter_map(|item| match item {
            Value::String(path) => Some(path.as_str()),
            Value::Object(obj) => obj.get("path").and_then(Value::as_str),
            _ => None,
        })
        .filter(|path| !path.is_empty())
        .collect()
}

/// 数组原样展开；单个对象 / 字符串视为单元素列表。
fn as_items(raw: &Value) -> Vec<&Value> {
    match raw {
        Value::Array(items) => items.iter().collect(),
        Value::Null => Vec::new(),
        other => vec![other],
    }
}

/// App 根目录内的相对路径 → 绝对路径（拒绝空串、绝对路径与 `..` 逃逸）。
fn resolve_inside(dir: &Path, rel: &str) -> Option<PathBuf> {
    if rel.is_empty() || Path::new(rel).is_absolute() || rel.contains("..") {
        return None;
    }
    Some(dir.join(rel))
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

#[cfg(test)]
#[path = "app_contrib_tests.rs"]
mod tests;
