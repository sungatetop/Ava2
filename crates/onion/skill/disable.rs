//! 工作区技能禁用策略（skills-manager）。
//!
//! 契约：禁用清单落盘于 `<projectRoot>/.onion/skills-manager.json`
//! （随仓库走，可提交共享），schema 为 `{"disabled": ["name", ...]}`。
//!
//! 语义：
//! - **fail-open 读取**：文件不存在 → 空集；JSON 解析失败 / 字段类型不符 →
//!   视为空集并 `warn!`（不阻断会话）；未知字段忽略；
//! - 名字经 kebab-case 校验（[`is_skill_name`]）+ 去重，非法名忽略；
//! - **逐调用重读**：文件极小（几十个名字），`list` / `get` 每个 pre-step 才被
//!   调用一次，每次重读换来「外部手改文件 / REST PUT 后下一轮立即生效」，
//!   免去缓存失效通知；
//! - 写入走 tmp + rename 原子写（全量替换，无并发合并），`.onion` 目录缺省创建；
//! - projectRoot 复用技能发现同款 [`find_project_root`]（自 cwd 向上找 `.git`）。

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::filesystem::find_project_root;
use super::is_skill_name;

/// 禁用清单文件名（`<projectRoot>/.onion/` 下）。
const MANAGER_FILE: &str = "skills-manager.json";

/// 禁用策略：按 cwd 解析当前生效的禁用名集合（每次调用重读）。
///
/// 实现方预期：读取必须 fail-open（任何异常按空集处理），绝不阻断会话主流程。
pub trait DisablePolicy: Send + Sync {
    fn disabled_names(&self, cwd: Option<&Path>) -> HashSet<String>;
}

/// 文件实现：读 `<projectRoot>/.onion/skills-manager.json`（无状态）。
#[derive(Debug, Default, Clone)]
pub struct FileDisablePolicy;

impl DisablePolicy for FileDisablePolicy {
    fn disabled_names(&self, cwd: Option<&Path>) -> HashSet<String> {
        match manager_file_path(cwd) {
            Some(path) => read_disabled_names(&path),
            None => HashSet::new(),
        }
    }
}

/// 禁用清单文件契约（serde：未知字段忽略；`disabled` 缺省视为空表）。
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct DisabledList {
    #[serde(default)]
    disabled: Vec<String>,
}

/// 解析禁用清单文件路径：`<projectRoot>/.onion/skills-manager.json`；
/// cwd 缺失时返回 None（无法定位 project root）。
pub fn manager_file_path(cwd: Option<&Path>) -> Option<PathBuf> {
    let cwd = cwd?;
    Some(find_project_root(cwd).join(".onion").join(MANAGER_FILE))
}

/// 读禁用清单（fail-open：缺文件 → 空集；解析失败 → 空集 + warn）。
pub fn read_disabled_names(path: &Path) -> HashSet<String> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return HashSet::new();
    };
    match serde_json::from_str::<DisabledList>(&raw) {
        Ok(list) => sanitize(list.disabled),
        Err(error) => {
            tracing::warn!(
                path = %path.display(),
                %error,
                "skills-manager.json 解析失败，按空禁用集处理"
            );
            HashSet::new()
        }
    }
}

/// 全量替换禁用清单（tmp + rename 原子写；`.onion` 目录缺省创建），
/// 返回落盘文件路径。名字校验 + 去重 + 排序后落盘。
pub fn write_disabled_names(cwd: &Path, names: &[String]) -> std::io::Result<PathBuf> {
    let path = manager_file_path(Some(cwd))
        .ok_or_else(|| std::io::Error::other("cwd is required to locate skills-manager.json"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let list = DisabledList { disabled: sanitize_sorted(names) };
    let tmp = path.with_extension("json.tmp");
    {
        use std::io::Write as _;
        let body = serde_json::to_string_pretty(&list).map_err(std::io::Error::other)?;
        let mut file = std::fs::File::create(&tmp)?;
        writeln!(file, "{body}")?;
        file.flush()?;
    }
    std::fs::rename(&tmp, &path)?;
    Ok(path)
}

/// 校验 + 去重为集合（成员判定用；非法名忽略）。
fn sanitize(names: Vec<String>) -> HashSet<String> {
    names.into_iter().filter(|name| is_skill_name(name)).collect()
}

/// 校验 + 去重 + 排序为稳定列表（落盘用）。
fn sanitize_sorted(names: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out: Vec<String> = names
        .iter()
        .filter(|name| is_skill_name(name.as_str()) && seen.insert((*name).clone()))
        .cloned()
        .collect();
    out.sort();
    out
}

#[cfg(test)]
#[path = "disable_tests.rs"]
mod tests;
