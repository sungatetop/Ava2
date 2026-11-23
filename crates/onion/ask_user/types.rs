//! ask_user 领域类型。
//!
//! 完整问题项含 `id` / `question` / `detail` / `header` /
//! `options` / `multiSelect` / `intent`。onion 取 legacy blocking 模式子集，
//! 保留 `id` / `question` / `header` / `options` / `multi_select`（`detail` /
//! `intent` 是 timed 模式 / 能力 UI 扩展，教学子集不引入）。

use serde::{Deserialize, Serialize};

/// 一个可选项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AskUserOption {
    /// 面向用户的选项标签。
    pub label: String,
    /// 可选的一句话权衡/影响说明。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// 一个问题。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AskUserQuestion {
    /// 调用方提供的稳定问题 id（答复中回显）。
    pub id: String,
    /// 向用户提出的具体问题。
    pub question: String,
    /// 可选短标题（如 "Confirm" / "Choose Mode"）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<String>,
    /// 可选选项列表（UI 渲染为菜单）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<AskUserOption>>,
    /// 是否允许多选（缺省单选）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multi_select: Option<bool>,
}

/// 单个问题的答复。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AskUserAnswerItem {
    /// 所答问题 id。
    pub id: String,
    /// 选中的选项标签（多选可多个）。
    pub selected: Vec<String>,
    /// 可选自由文本 "Other" 答复。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom: Option<String>,
}

/// 人工答复。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AskUserAnswer {
    pub answers: Vec<AskUserAnswerItem>,
}
