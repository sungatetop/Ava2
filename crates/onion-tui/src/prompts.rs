//! prompts.rs：问题面板（设计文档 §9）。
//!
//! 位置 = 宽幅左下键盘面板（有待审批时替换/叠加输入编辑器）；批次进度
//! 「1/3」式（多请求排队时）；编号选项 + 对齐描述；控制提示按当前选项数渲染。
//!
//! 入参是**展示切片**（工具名 / 输入 JSON / plan 文本 / 问题列表），不含
//! oneshot 答复句柄——句柄由 `Commands` 持有（答复走 `/approve` `/deny`
//! `/answer`），展示切片由主循环持有，两者生命周期解耦。

use onion::ask_user::types::AskUserQuestion;

use crate::editor::char_width;
use crate::renderer::{Line, Style};

/// 显示宽度（CJK 双宽，对齐 editor::cursor_col 口径）。
fn display_width(text: &str) -> usize {
    text.chars().map(char_width).sum()
}

/// 卡片框线（对齐 opencode dialog 框架 + Trae CLI 审批卡）：
/// 圆角边框 + 反色标题行 + 内容行，整体染边框色。
fn box_card(title: &str, body: Vec<String>, border: crossterm::style::Color) -> Vec<Line> {
    let mut all = vec![title.to_string()];
    all.extend(body);
    let width = all.iter().map(|l| display_width(l)).max().unwrap_or(0) + 3; // │ + 左右各 1 格 padding
    let mut lines = Vec::new();
    lines.push(Line::styled(
        format!("╭{}╮", "─".repeat(width)),
        Style::new().color(border),
    ));
    for (idx, line) in all.iter().enumerate() {
        let pad = width.saturating_sub(display_width(line) + 3);
        // 标题行反色（卡片头），内容行原样。
        let style = if idx == 0 {
            Style::new().color(border).reverse()
        } else {
            Style::new().color(border)
        };
        lines.push(Line::styled(
            format!("│ {line}{}│", " ".repeat(pad)),
            style,
        ));
    }
    lines.push(Line::styled(
        format!("╰{}╯", "─".repeat(width)),
        Style::new().color(border),
    ));
    lines
}

/// 在飞问题面板状态（同一时刻至多一个在飞请求 + 批次进度）。
pub struct PromptPanel {
    /// 批次进度：`current` = 已呈现序号（1 起），`total` = 批次总数。
    pub batch: Option<(usize, usize)>,
}

impl PromptPanel {
    pub fn new() -> Self {
        Self { batch: None }
    }

    /// 渲染工具审批面板行（红框卡：标题 + 输入摘要 + 编号选项）。
    pub fn render_approval(&self, name: &str, input: &serde_json::Value) -> Vec<Line> {
        let title = match self.batch {
            Some((current, total)) => format!("审批（{current}/{total}）：{name}"),
            None => format!("审批：{name}"),
        };
        box_card(
            &title,
            vec![summarize_input(input), "1. 允许  2. 拒绝（/approve /deny）".to_string()],
            crossterm::style::Color::Red,
        )
    }

    /// 渲染 plan review 面板行（红框卡：标题 + plan 全文 + 编号选项）。
    pub fn render_plan_review(&self, plan: &str) -> Vec<Line> {
        let title = match self.batch {
            Some((current, total)) => format!("计划审批（{current}/{total}）"),
            None => "计划审批".to_string(),
        };
        let mut body: Vec<String> = plan.lines().map(str::to_string).collect();
        body.push("1. 批准  2. 继续规划（/approve /deny）".to_string());
        box_card(&title, body, crossterm::style::Color::Red)
    }

    /// 渲染 ask_user 问询行（红框卡：问题清单 + `/answer` 答复提示）。
    pub fn render_ask_user(&self, questions: &[AskUserQuestion]) -> Vec<Line> {
        let mut body = vec![format!(
            "共 {} 问（/answer <id> <选项编号或文本>）",
            questions.len()
        )];
        for question in questions {
            body.push(format!("  [{}] {}", question.id, question.question));
            if let Some(options) = &question.options {
                for (index, option) in options.iter().enumerate() {
                    body.push(format!(
                        "    {}. {}{}",
                        index + 1,
                        option.label,
                        option
                            .description
                            .as_deref()
                            .map(|d| format!("（{d}）"))
                            .unwrap_or_default()
                    ));
                }
            }
        }
        box_card("待答复", body, crossterm::style::Color::Red)
    }
}

impl Default for PromptPanel {
    fn default() -> Self {
        Self::new()
    }
}

/// 工具输入摘要（head/tail 截断）。
fn summarize_input(input: &serde_json::Value) -> String {
    let rendered = serde_json::to_string(input).unwrap_or_default();
    const MAX: usize = 120;
    if rendered.chars().count() <= MAX {
        rendered
    } else {
        let head: String = rendered.chars().take(MAX / 2).collect();
        let tail: String = rendered.chars().rev().take(MAX / 2).collect();
        let tail: String = tail.chars().rev().collect();
        format!("{head}…{tail}")
    }
}

#[cfg(test)]
#[path = "prompts_tests.rs"]
mod tests;
