//! provider_form.rs：/provider 表单 overlay（新增供应商 / 新增模型）。
//!
//! 对齐 model_picker 的 overlay 状态机风格：打开 → 逐字段编辑 → Enter 提交 /
//! Esc 取消。表单不直接落盘：提交后由 main 消费 `FormSubmit` 回调写回
//! providers.json（onion `ProviderCatalog::save`），并刷新目录快照。
//!
//! 两种表单：
//! - 供应商（Provider）：id / name / base_url / api_key（api_key 可留空）。
//! - 模型（Model）：provider_id / id / name / description（后两者可留空）。

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::renderer::Line;

/// 表单类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormKind {
    Provider,
    Model,
}

/// 已提交的表单结果（main 消费写回 providers.json）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormResult {
    /// 新增/替换供应商（id, name, base_url, api_key, models——逗号分隔模型 id）。
    AddProvider(String, String, String, String, String),
    /// 向供应商追加/替换模型（provider_id, id, name, description）。
    AddModel(String, String, String, String),
}

/// 表单状态：打开中（当前编辑字段索引 + 各字段缓冲）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderForm {
    pub kind: FormKind,
    /// 字段缓冲（索引对齐 `fields()` 标签）。
    pub values: Vec<String>,
    /// 当前正在编辑的字段索引。
    pub active: usize,
    /// 表单已打开。
    pub open: bool,
}

impl ProviderForm {
    /// 关闭态（默认，无表单打开）。
    pub fn closed() -> Self {
        Self {
            kind: FormKind::Provider,
            values: Vec::new(),
            active: 0,
            open: false,
        }
    }

    /// 打开供应商表单。
    pub fn open_provider() -> Self {
        Self {
            kind: FormKind::Provider,
            values: vec![String::new(); 5],
            active: 0,
            open: true,
        }
    }

    /// 打开模型表单（预填 provider_id 为当前高亮/默认供应商）。
    pub fn open_model(provider_id: String) -> Self {
        let mut values = vec![String::new(); 4];
        values[0] = provider_id;
        Self {
            kind: FormKind::Model,
            values,
            active: 1,
            open: true,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// 字段标签（渲染用）。
    pub fn fields(&self) -> &'static [&'static str] {
        match self.kind {
            FormKind::Provider => &["id", "name", "base_url", "api_key", "models"],
            FormKind::Model => &["provider_id", "id", "name", "description"],
        }
    }

    /// 按键处理：返回 Some(result) = 提交成功；None = 继续编辑/取消。
    /// Esc 关闭（返回 None，open 置 false）；Tab/Enter 下移字段；
    /// Shift+Tab 上移；最后一个字段 Enter 提交。
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<FormResult> {
        if !self.open {
            return None;
        }
        match key.code {
            KeyCode::Esc => {
                self.open = false;
                None
            }
            KeyCode::BackTab => {
                self.active = self.active.saturating_sub(1);
                None
            }
            KeyCode::Tab | KeyCode::Enter => {
                let last = self.values.len() - 1;
                if self.active == last {
                    // 最后一个字段 Enter：提交。
                    let result = match self.kind {
                        FormKind::Provider => FormResult::AddProvider(
                            self.values[0].trim().to_string(),
                            self.values[1].trim().to_string(),
                            self.values[2].trim().to_string(),
                            self.values[3].trim().to_string(),
                            self.values[4].trim().to_string(),
                        ),
                        FormKind::Model => FormResult::AddModel(
                            self.values[0].trim().to_string(),
                            self.values[1].trim().to_string(),
                            self.values[2].trim().to_string(),
                            self.values[3].trim().to_string(),
                        ),
                    };
                    self.open = false;
                    Some(result)
                } else {
                    self.active += 1;
                    None
                }
            }
            KeyCode::Backspace => {
                self.values[self.active].pop();
                None
            }
            KeyCode::Char(c) => {
                // Ctrl 组合键不进编辑（Ctrl+C 由主循环拦截）。
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
                    self.values[self.active].push(c);
                }
                None
            }
            _ => None,
        }
    }

    /// 渲染 overlay 行集（关闭返回空）。
    pub fn render(&self) -> Vec<Line> {
        if !self.open {
            return Vec::new();
        }
        let title = match self.kind {
            FormKind::Provider => "── /provider 新增供应商 ─────────────────────────",
            FormKind::Model => "── /provider model 新增模型 ──────────────────────",
        };
        let mut lines = vec![Line::plain(title)];
        let fields = self.fields();
        let last = fields.len() - 1;
        for (i, label) in fields.iter().enumerate() {
            let marker = if i == self.active { ">" } else { " " };
            let value = &self.values[i];
            let tail = if i == last { "  ← Enter 提交" } else { "" };
            lines.push(Line::plain(format!("{marker} {label}: {value}{tail}")));
        }
        lines.push(Line::plain(
            "  Tab/Enter 下一字段 · Shift+Tab 上一字段 · 末字段 Enter 提交 · Esc 取消".to_string(),
        ));
        lines
    }
}

#[cfg(test)]
#[path = "provider_form_tests.rs"]
mod tests;
