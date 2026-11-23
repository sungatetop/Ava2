//! model_picker.rs：/model 过滤选择器 overlay（设计文档 §11）。
//!
//! 过滤框 + Escape 双段（过滤非空先清空过滤，
//! 再次 Escape 才取消选择器）+ Shift+Tab effort 循环（onion 目录暂无条目级
//! effort 元数据 → 占位提示不可用，P2 接入）。
//!
//! 数据源：onion `ProviderCatalog`（缺口已补齐：provider_catalog.rs /
//! model_target.rs / settings.rs 接线闭环）——`from_catalog` 将目录条目
//! 转换为选择器 entries；选中语义 = `cmd_model` 带参直切路径
//! （swap_current + 写回 settings + ModelSelection durable 事件）。

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::renderer::Line;

/// 目录条目（P2 决策阻塞下的最小形态：provider/model 标签 + 描述）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelEntry {
    /// provider/model 标签（如 `local/Qwen3.8-27B`）。
    pub label: String,
    /// 模型名（如 `Qwen3.8-27B`）。
    pub model: String,
    /// 描述（如 base_url）。
    pub description: String,
}

/// 选择器状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickerState {
    /// 已关闭。
    Closed,
    /// 打开中（filter = 过滤子串，selected = 过滤后列表高亮行索引）。
    Open { filter: String, selected: usize },
}

/// /model 过滤选择器 overlay。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelPicker {
    /// 目录（打开时快照，不随目录变更而失效）。
    catalog: Vec<ModelEntry>,
    state: PickerState,
}

impl ModelPicker {
    /// onion 目录 → 选择器 entries（label = `provider/model`，desc = 条目描述或 base_url）。
    pub fn from_catalog(catalog: &onion::provider_catalog::ProviderCatalog) -> Self {
        let mut catalog = catalog.clone();
        catalog.providers.retain(|p| !p.models.is_empty());
        Self::new(
            catalog.providers
                .iter()
                .flat_map(|p| {
                    p.models.iter().map(move |m| ModelEntry {
                        label: format!("{}/{}", p.id, m.id),
                        model: m.id.clone(),
                        description: m
                            .description
                            .clone()
                            .unwrap_or_else(|| p.base_url.clone()),
                    })
                })
                .collect(),
        )
    }

    /// 从目录构建（closed 态）。
    pub fn new(catalog: Vec<ModelEntry>) -> Self {
        Self { catalog, state: PickerState::Closed }
    }

    pub fn state(&self) -> &PickerState {
        &self.state
    }

    /// /model：打开选择器（过滤清空，高亮 = 当前 model 行）。
    pub fn open(&mut self, current: &str) {
        let selected = self
            .catalog
            .iter()
            .position(|e| e.model == current)
            .unwrap_or(0);
        self.state = PickerState::Open { filter: String::new(), selected };
    }

    /// 过滤后可见条目（大小写不敏感子串匹配标签/模型名/描述）。
    pub fn visible(&self) -> Vec<ModelEntry> {
        let PickerState::Open { filter, .. } = &self.state else {
            return Vec::new();
        };
        Self::filtered(&self.catalog, filter)
    }

    /// 大小写不敏感子串过滤（标签/模型名/描述）。
    fn filtered(catalog: &[ModelEntry], filter: &str) -> Vec<ModelEntry> {
        let needle = filter.to_lowercase();
        catalog
            .iter()
            .filter(|e| {
                needle.is_empty()
                    || e.label.to_lowercase().contains(&needle)
                    || e.model.to_lowercase().contains(&needle)
                    || e.description.to_lowercase().contains(&needle)
            })
            .cloned()
            .collect()
    }

    /// 按键处理（open 态）：返回 Some(model) = 确认选择。
    /// Escape 双段：过滤非空先清空，再次 Escape 关闭（返回 None）。
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<String> {
        let PickerState::Open { filter, selected } = &mut self.state else {
            return None;
        };
        match key.code {
            KeyCode::Esc => {
                if !filter.is_empty() {
                    filter.clear();
                    *selected = 0;
                    return None;
                }
                self.state = PickerState::Closed;
                None
            }
            KeyCode::Enter => {
                // 确认选择：先取可见条目（filtered 只借局部快照，不借 self），
                // 再关状态并返回选中 model。
                let visible = Self::filtered(&self.catalog, filter);
                let model = visible.get(*selected).map(|e| e.model.clone());
                self.state = PickerState::Closed;
                model
            }
            KeyCode::Up => {
                *selected = selected.saturating_sub(1);
                None
            }
            KeyCode::Down => {
                *selected += 1;
                None
            }
            KeyCode::Backspace => {
                filter.pop();
                *selected = 0;
                None
            }
            KeyCode::Char(c) => {
                // Ctrl 组合键不进 filter（Ctrl+C 由主循环拦截）。
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
                    filter.push(c);
                    *selected = 0;
                }
                None
            }
            _ => None,
        }
    }

    /// 渲染 overlay 行集（open 态；closed 返回空）。
    pub fn render(&self) -> Vec<Line> {
        let PickerState::Open { filter, selected } = &self.state else {
            return Vec::new();
        };
        let visible = self.visible();
        let mut lines = vec![
            Line::plain("── /model 选择器 ─────────────────────────"),
            Line::plain(format!("过滤: {filter}")),
        ];
        if visible.is_empty() {
            lines.push(Line::plain("  （无匹配条目）".to_string()));
        } else {
            let selected = (*selected).min(visible.len() - 1);
            for (i, entry) in visible.iter().enumerate() {
                let marker = if i == selected { ">" } else { " " };
                lines.push(Line::plain(format!("{marker} {} — {}", entry.label, entry.description)));
            }
        }
        lines.push(Line::plain(
            "  Enter 确认 · Esc 取消/清过滤 · ↑↓ 选择".to_string(),
        ));
        lines
    }
}

#[cfg(test)]
#[path = "model_picker_tests.rs"]
mod tests;
