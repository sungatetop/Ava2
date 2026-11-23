//! settings.json 写回通道（方案 4：模型选择用户级持久化）。
//!
//! /model 切换后持久化用户级 model 目标（model + base_url + effort），启动时
//! 读回覆盖 env 缺省。写回失败仅 warn、不影响本会话：
//! 写回失败不阻塞 /model 切换，仅在启动读回时以 env/内建缺省兜底。
//!
//! 磁盘形态 `<root>/settings.json`：
//! ```json
//! { "model": { "model": "Qwen3.8-27B", "base_url": "...", "effort": null } }
//! ```

use std::path::Path;

use serde::{Deserialize, Serialize};

/// settings.json 磁盘形态。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<PersistedModelTarget>,
}

/// 持久化的模型目标（model + provider 路由 + 可选 effort）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedModelTarget {
    pub model: String,
    pub base_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
}

impl SettingsFile {
    /// 读取 `<root>/settings.json`；文件不存在 / 解析失败 → 缺省空表（不阻塞启动）。
    pub fn load(root: &Path) -> Self {
        let path = crate::paths::settings_file(root);
        match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// 读取持久化的模型目标（None = 无写回记录，调用方回退 env/内建）。
    pub fn model_target(&self) -> Option<PersistedModelTarget> {
        self.model.clone()
    }

    /// 写回模型目标（model + base_url + effort）。写失败仅返回 Err，调用方
    /// 决定是否 warn（失败不影响本会话）。
    pub fn save_model_target(root: &Path, target: &PersistedModelTarget) -> std::io::Result<()> {
        let file = SettingsFile { model: Some(target.clone()) };
        let path = crate::paths::settings_file(root);
        let content = serde_json::to_string_pretty(&file)
            .map_err(std::io::Error::other)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, content)
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
