//! 模型目录（进程内内存投影，加载自 providers.json）。
//!
//! 目录在进程内运行时构建，直接读
//! 文件构建即可。加载顺序：`ONION_PROVIDER_CATALOG` 指定路径 → `<ONION_HOME>/
//! providers.json` → 内嵌缺省表（对齐 workspace 规则 §4 local 供应商冒烟约定）。
//!
//! 目录条目含条目级 effort 元数据（缺口 3，P2）：`reasoning: { efforts, default_effort }`，
//! 供 onion-tui Shift+Tab effort 循环消费。
//!
//! api_key 支持凭据引用（引用式 seam）：providers.json 只存
//! `${credential:NAME}` 引用，值落 `<root>/.credentials`；目录内存投影与 save
//! 落盘始终保留引用原样，仅在 [`ProviderCatalog::target_for`] 构建 ModelTarget
//! 时按次解析（改密钥即时生效，无需重启）。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// 推理 effort 条目（{ id, name, description? }）。
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffortEntry {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// 条目级推理元数据（{ efforts, defaultEffort? }）。
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReasoningMeta {
    pub efforts: Vec<EffortEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_effort: Option<String>,
}

/// 目录条目（{ id, name, description?, reasoning? }）。
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelEntry {
    /// provider-owned 模型 id（如 `Qwen3.8-27B`）。
    pub id: String,
    /// 展示名。
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<ReasoningMeta>,
}

/// provider 目录条目（{ id, name, models }）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderEntry {
    /// provider 标识（如 `local`）。
    pub id: String,
    /// 展示名。
    pub name: String,
    /// OpenAI 兼容基址。
    pub base_url: String,
    /// 鉴权密钥。
    pub api_key: String,
    /// 该 provider 下模型列表。
    pub models: Vec<ModelEntry>,
}

/// 模型目录（{ default, routableProviders, groups }）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCatalog {
    pub providers: Vec<ProviderEntry>,
    /// 当前默认模型 id。
    pub default_model: String,
    /// 默认模型是否来自文件显式声明（`defaultModel` 字段）；false = env 兜底。
    #[serde(skip)]
    pub has_custom_default: bool,
    /// 凭据数据根（凭据文件所在目录）；None = 内嵌表（无凭据解析）。
    #[serde(skip)]
    credential_root: Option<PathBuf>,
}

/// providers.json 磁盘形态（顶层 groups 数组）。
///
/// `default_model` 为可选的「用户显式默认模型」：缺省时目录默认模型取调用方
/// 传入值（进程 env 装配的 Config.model）；写入后新会话装配以它为初始目标。
#[derive(Debug, Deserialize, Serialize)]
struct CatalogFile {
    #[serde(default)]
    groups: Vec<CatalogGroup>,
    #[serde(
        default,
        rename = "defaultModel",
        skip_serializing_if = "Option::is_none"
    )]
    default_model: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogGroup {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    base_url: String,
    #[serde(default)]
    api_key: String,
    #[serde(default)]
    models: Vec<ModelEntry>,
}

impl ProviderCatalog {
    /// 从缺省模型构建内嵌目录（对齐 workspace 规则 §4 local 供应商冒烟约定）。
    pub fn builtin(default_model: &str) -> Self {
        Self {
            providers: vec![ProviderEntry {
                id: "local".to_string(),
                name: "local".to_string(),
                base_url: crate::config::default_base_url(),
                api_key: crate::config::default_api_key(),
                models: vec![
                    ModelEntry {
                        id: "Qwen3.8-27B".to_string(),
                        name: "Qwen3.8-27B".to_string(),
                        description: Some("local 供应商缺省模型".to_string()),
                        reasoning: None,
                    },
                    ModelEntry {
                        id: "DeepSeek-V4-Flash-0731".to_string(),
                        name: "DeepSeek-V4-Flash-0731".to_string(),
                        description: Some("local 供应商备选模型".to_string()),
                        reasoning: None,
                    },
                ],
            }],
            default_model: default_model.to_string(),
            has_custom_default: false,
            credential_root: None,
        }
    }

    /// 加载目录：`ONION_PROVIDER_CATALOG` 指定路径 → `<root>/providers.json` →
    /// 内嵌缺省表。文件不存在 / 解析失败时回退内嵌表（不阻塞启动，
    /// 失败 provider 不影响其余路由）。
    pub fn load(root: &Path, default_model: &str) -> Self {
        let explicit = std::env::var("ONION_PROVIDER_CATALOG")
            .ok()
            .filter(|p| !p.trim().is_empty())
            .map(std::path::PathBuf::from);
        let candidate = explicit.unwrap_or_else(|| root.join("providers.json"));
        let mut catalog =
            load_file(&candidate, default_model).unwrap_or_else(|| Self::builtin(default_model));
        catalog.credential_root = Some(root.to_path_buf());
        catalog
    }

    /// 目录是否包含某模型 id（供 /model 选择器校验）。
    pub fn contains_model(&self, model: &str) -> bool {
        self.providers
            .iter()
            .any(|p| p.models.iter().any(|m| m.id == model))
    }

    /// 设置默认模型（写入 `default_model` 并标记显式声明；save 时落盘）。
    pub fn set_default_model(&mut self, model: &str) {
        self.default_model = model.to_string();
        self.has_custom_default = true;
    }

    /// 从指定供应商移除模型。返回是否发生了移除（不存在为 false）。
    pub fn remove_model(&mut self, provider_id: &str, model_id: &str) -> bool {
        let Some(provider) = self.providers.iter_mut().find(|p| p.id == provider_id) else {
            return false;
        };
        let before = provider.models.len();
        provider.models.retain(|m| m.id != model_id);
        provider.models.len() != before
    }

    /// 移除供应商（连带其模型）。返回被移除的供应商；不存在为 None。
    pub fn remove_provider(&mut self, provider_id: &str) -> Option<ProviderEntry> {
        let idx = self.providers.iter().position(|p| p.id == provider_id)?;
        Some(self.providers.remove(idx))
    }

    /// 由模型 id 派生 `ModelTarget`（base_url/api_key 取自所属 provider）。
    /// api_key 中的 `${credential:NAME}` 引用在此按次解析（见模块文档）。
    /// 目录中不存在时返回 None（视为模型不可用）。
    pub fn target_for(&self, model: &str) -> Option<crate::model_target::ModelTarget> {
        self.providers.iter().find_map(|p| {
            p.models.iter().find(|m| m.id == model).map(|m| crate::model_target::ModelTarget {
                model: m.id.clone(),
                base_url: p.base_url.clone(),
                api_key: resolve_api_key(&p.id, &p.api_key, self.credential_root.as_deref()),
                effort: m.reasoning.as_ref().and_then(|r| r.default_effort.clone()),
            })
        })
    }

    /// 反查某模型所属的精确路由（provider id + model id）。
    ///
    /// 供「无白名单时只允许父路由」判定使用（取父会话的
    /// provider/model）。同 id 模型出现在
    /// 多个 provider 时优先匹配 `base_url` 相同者，否则取首个。
    pub fn route_for(
        &self,
        model: &str,
        base_url: &str,
    ) -> Option<crate::subagent::model_selection::AllowedModelRoute> {
        let matches = |p: &&ProviderEntry| p.models.iter().any(|m| m.id == model);
        let provider = self
            .providers
            .iter()
            .filter(matches)
            .find(|p| p.base_url == base_url)
            .or_else(|| self.providers.iter().find(matches))?;
        Some(crate::subagent::model_selection::AllowedModelRoute::new(
            provider.id.clone(),
            model,
        ))
    }

    /// 新增（或替换同 id）供应商。返回是否发生了替换（存在同 id 供应商）。
    pub fn add_provider(&mut self, id: String, name: String, base_url: String, api_key: String) -> bool {
        let replaced = self.providers.iter().any(|p| p.id == id);
        if let Some(p) = self.providers.iter_mut().find(|p| p.id == id) {
            p.name = name;
            p.base_url = base_url;
            p.api_key = api_key;
        } else {
            self.providers.push(ProviderEntry {
                id,
                name,
                base_url,
                api_key,
                models: Vec::new(),
            });
        }
        replaced
    }

    /// 向指定供应商追加模型（不存在则创建供应商）。返回是否发生了模型
    /// 重复替换（同 id 已存在）。
    pub fn add_model(
        &mut self,
        provider_id: &str,
        id: String,
        name: String,
        description: Option<String>,
    ) -> bool {
        let idx = if let Some(i) = self.providers.iter().position(|p| p.id == provider_id) {
            i
        } else {
            // 供应商不存在：以空路由占位创建（调用方补充 base_url/api_key）。
            self.providers.push(ProviderEntry {
                id: provider_id.to_string(),
                name: provider_id.to_string(),
                base_url: String::new(),
                api_key: String::new(),
                models: Vec::new(),
            });
            self.providers.len() - 1
        };
        let provider = &mut self.providers[idx];
        let replaced = provider.models.iter().any(|m| m.id == id);
        if let Some(m) = provider.models.iter_mut().find(|m| m.id == id) {
            m.name = name;
            m.description = description;
        } else {
            provider.models.push(ModelEntry {
                id,
                name,
                description,
                reasoning: None,
            });
        }
        replaced
    }

    /// 写回 `<root>/providers.json`（与加载同路径解析：`ONION_PROVIDER_CATALOG`
    /// 指定则写该路径，否则写 `<root>/providers.json`）。返回实际写入路径。
    /// 解析失败/写失败均返回 Err，调用方决定反馈。
    pub fn save(&self, root: &Path) -> std::io::Result<PathBuf> {
        let explicit = std::env::var("ONION_PROVIDER_CATALOG")
            .ok()
            .filter(|p| !p.trim().is_empty())
            .map(std::path::PathBuf::from);
        let path = explicit.unwrap_or_else(|| root.join("providers.json"));
        let file = CatalogFile {
            groups: self
                .providers
                .iter()
                .map(|p| CatalogGroup {
                    id: p.id.clone(),
                    name: p.name.clone(),
                    base_url: p.base_url.clone(),
                    api_key: p.api_key.clone(),
                    models: p.models.clone(),
                })
                .collect(),
            default_model: self
                .has_custom_default
                .then(|| self.default_model.clone()),
        };
        let content = serde_json::to_string_pretty(&file)
            .map_err(std::io::Error::other)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, content)?;
        Ok(path)
    }
}

fn load_file(path: &Path, default_model: &str) -> Option<ProviderCatalog> {
    let content = std::fs::read_to_string(path).ok()?;
    let file: CatalogFile = serde_json::from_str(&content).ok()?;
    let providers = file
        .groups
        .into_iter()
        .filter(|g| g.id != "failure")
        .map(|g| {
            let name = if g.name.is_empty() { g.id.clone() } else { g.name };
            ProviderEntry {
                id: g.id,
                name,
                base_url: g.base_url,
                // 引用原样入内存投影：解析收敛在 target_for（save 落盘不回写明文）。
                api_key: g.api_key,
                models: g.models,
            }
        })
        .collect();
    // 文件显式 defaultModel 优先于调用方传入值（env 兜底）；空串视为未声明。
    let (default, custom) = match file.default_model {
        Some(model) if !model.trim().is_empty() => (model, true),
        _ => (default_model.to_string(), false),
    };
    Some(ProviderCatalog {
        providers,
        default_model: default,
        has_custom_default: custom,
        credential_root: None,
    })
}

/// 解析 api_key 中的 `${credential:NAME}` 引用（引用式 seam：providers.json
/// 只存引用，值落 `<root>/.credentials`，构建 ModelTarget 时按次解析）。
/// `credential_root` 为 None 或原值不含 `${` 时原样返回。
///
/// 解析失败不阻断取目标（对齐失败 provider 隔离语义）：语法非法 → 原值保留 +
/// warn；引用未配置 → 空 api_key + warn（请求期鉴权失败可见，模型列表不受影响）。
fn resolve_api_key(provider_id: &str, api_key: &str, credential_root: Option<&Path>) -> String {
    if !api_key.contains("${") {
        return api_key.to_string();
    }
    let Some(root) = credential_root else {
        tracing::warn!(
            provider = provider_id,
            "api_key 含凭据引用但目录无凭据根（内嵌表），按原值保留"
        );
        return api_key.to_string();
    };
    match crate::credentials::CredentialStore::new(root).resolve_template(api_key) {
        Ok(resolved) => resolved,
        Err(crate::credentials::TemplateError::Missing { name }) => {
            tracing::warn!(
                provider = provider_id,
                name,
                "providers.json api_key 引用的凭据未配置，解析为空（鉴权将失败）"
            );
            String::new()
        }
        Err(err @ crate::credentials::TemplateError::Syntax(_)) => {
            tracing::warn!(provider = provider_id, error = %err, "providers.json api_key 模板语法非法，按原值保留");
            api_key.to_string()
        }
    }
}

#[cfg(test)]
#[path = "provider_catalog_tests.rs"]
mod tests;
