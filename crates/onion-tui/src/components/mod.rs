//! components.rs：基础渲染组件（Node → Line 的确定性映射）。
//!
//! Phase A 最小集：会话头 / turn 分隔 / 用户消息 / assistant 块 / 工具卡片 /
//! 压缩卡 / token 总计。所有组件是纯函数：`(Node, width) → Vec<Line>`，
//! 同输入恒同输出——append-origin 语义不依赖终端尺寸。

use crate::renderer::Line;
use crate::transcript::Node;
use crate::view_state::ViewState;
use onion::todo::types::TodoStatus;
use onion::types::ContentBlock;

#[cfg(test)]
#[path = "components_tests.rs"]
mod components_tests;

/// 会话头。
pub fn render_header(session_id: &str, model: &str) -> Vec<Line> {
    vec![
        Line::plain("onion session"),
        Line::styled(
            format!("  id: {session_id}"),
            crate::renderer::Style::new().dim(),
        ),
        Line::styled(
            format!("  model: {model}"),
            crate::renderer::Style::new().dim(),
        ),
    ]
}

/// 用户消息。
pub fn render_user_message(text: &str) -> Vec<Line> {
    vec![Line::plain(format!("▸ user: {text}"))]
}

/// assistant 块：文本 + 推理 + token 总计。
///
/// 文本按 `\n` 拆多行（行是绝对行定位的渲染单元，嵌入换行会把后续行错位）。
pub fn render_assistant_block(text: &str, thinking: &str, usage: Option<&onion::types::Usage>) -> Vec<Line> {
    let mut lines = Vec::new();
    for line in split_lines(thinking) {
        lines.push(Line::styled(line, crate::renderer::Style::new().dim()));
    }
    for line in split_lines(text) {
        // 正文显式白色（SGR 37，主题感知）：不依赖终端默认前景，长会话里
        // 与 dim 的推理/工具行形成稳定对比。
        lines.push(Line::styled(
            line,
            crate::renderer::Style::new().color(crossterm::style::Color::White),
        ));
    }
    if let Some(usage) = usage {
        lines.push(Line::styled(
            format!(
                "  tokens: in {} out {}",
                usage.input_tokens, usage.output_tokens
            ),
            crate::renderer::Style::new().dim(),
        ));
    }
    lines
}

/// 按行拆分（保留空行；末尾换行不产生多余空行）。
fn split_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    text.trim_end_matches('\n').split('\n').map(str::to_string).collect()
}

/// 工具卡片：标题行 + 展开输出（缩进）/ 折叠统计行 + 状态。
///
/// 呈现（对齐 opencode 工具行）：
/// - 折叠：`└ Done (ctrl+o to expand)`（输出折叠，Ctrl+E 展开）
/// - 展开：输出行 3 格缩进（对齐 opencode 折叠子树缩进）
pub fn render_tool_card(
    name: &str,
    input_summary: &str,
    output: Option<&str>,
    is_error: bool,
) -> Vec<Line> {
    let mut lines = vec![Line::plain(format!("▸ {name}({input_summary})"))];
    match output {
        Some(output) => {
            for line in output.lines().take(3) {
                lines.push(Line::styled(
                    format!("   {line}"),
                    crate::renderer::Style::new().dim(),
                ));
            }
        }
        None if is_error => {
            lines.push(Line::styled(
                "└ Failed (ctrl+o to expand)".to_string(),
                crate::renderer::Style::new().dim(),
            ));
        }
        None => {
            lines.push(Line::styled(
                "└ Done (ctrl+o to expand)".to_string(),
                crate::renderer::Style::new().dim(),
            ));
        }
    }
    if is_error {
        lines.push(Line::styled(
            "error".to_string(),
            crate::renderer::Style::new().reverse().color(crossterm::style::Color::Red),
        ));
    }
    lines
}

/// 压缩卡。
pub fn render_compaction_card(
    trigger: &str,
    summary: Option<&str>,
    shadowed_count: Option<usize>,
    shadowed_tokens: Option<u64>,
    error: Option<&str>,
) -> Vec<Line> {
    let mut lines = vec![Line::plain(format!("⇄ compaction ({trigger})"))];
    if let Some(summary) = summary {
        lines.push(Line::styled(summary.to_string(), crate::renderer::Style::new().dim()));
    }
    if let (Some(c), Some(t)) = (shadowed_count, shadowed_tokens) {
        lines.push(Line::styled(
            format!("  shadowed {c} messages, {t} tokens"),
            crate::renderer::Style::new().dim(),
        ));
    }
    if let Some(error) = error {
        lines.push(Line::styled(
            error.to_string(),
            crate::renderer::Style::new().reverse().color(crossterm::style::Color::Red),
        ));
    }
    lines
}

/// assistant 尝试失败。
pub fn render_assistant_attempt(error: &str, cancelled: bool) -> Vec<Line> {
    let tag = if cancelled { "cancelled" } else { "failed" };
    vec![Line::styled(
        format!("assistant {tag}: {error}"),
        crate::renderer::Style::new().reverse(),
    )]
}

/// goal 相位条。
pub fn render_goal_bar(objective: &str, phase: &str, max_rounds: u32) -> Vec<Line> {
    vec![Line::plain(format!("◇ goal [{phase}] {objective} (max {max_rounds} rounds)"))]
}

/// plan 徽标。
pub fn render_plan_badge(active: bool, wanted: Option<bool>) -> Vec<Line> {
    let mut lines = vec![Line::plain(format!("plan: {}", if active { "on" } else { "off" }))];
    if let Some(wanted) = wanted {
        lines.push(Line::styled(
            format!("wanted: {}", if wanted { "on" } else { "off" }),
            crate::renderer::Style::new().dim(),
        ));
    }
    lines
}

/// hook 行。
pub fn render_hook_line(point: &str, handler_id: &str, decision: Option<&str>) -> Vec<Line> {
    let mut text = format!("hook {point} ({handler_id})");
    if let Some(decision) = decision {
        text.push_str(&format!(" → {decision}"));
    }
    vec![Line::styled(text, crate::renderer::Style::new().dim())]
}

/// workspace 事件行。
pub fn render_workspace_line(summary: &str) -> Vec<Line> {
    vec![Line::styled(summary.to_string(), crate::renderer::Style::new().dim())]
}

/// 组件树 → 行序列（主渲染入口）。
///
/// `view` 是视图级开关（§7.3 terminal-only controls）：推理显隐与工具卡片
/// 展开/折叠都只影响投影，不回写 durable。`streaming` 为真时在最后一个
/// 非空 assistant 文本行尾追加块光标 `▌`（对齐 pi-tui/codex 的流式打字机
/// 视觉，diff 渲染下随 delta 平滑推进）。
pub fn render_nodes(nodes: &[Node], view: ViewState, streaming: bool) -> Vec<Line> {
    // 流式块光标的落点：最后一个非空 assistant 块（反向找，None = 无）。
    let stream_at = if streaming {
        nodes.iter().rposition(|n| {
            matches!(n, Node::AssistantBlock { text, .. } if !text.is_empty())
        })
    } else {
        None
    };
    let mut lines = Vec::new();
    for (idx, node) in nodes.iter().enumerate() {
        match node {
            Node::Header { session_id, model } => {
                lines.extend(render_header(session_id, model));
            }
            // turn 分隔：`● turn N (reason)` 圆点标题（对齐 opencode 分组行）。
            // 轮次统计仍在 footer（t{n} · {m} steps）；节点保留（durable 投影
            // + token 估算）。
            Node::TurnSeparator { turn, reason } => {
                let title = match reason {
                    Some(reason) => format!("● turn {turn} ({reason})"),
                    None => format!("● turn {turn}"),
                };
                lines.push(Line::styled(
                    title,
                    crate::renderer::Style::new()
                        .color(crossterm::style::Color::Blue)
                        .dim(),
                ));
            }
            Node::UserMessage { message } => {
                let text = message
                    .blocks
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<String>();
                lines.extend(render_user_message(&text));
            }
            Node::InjectionNote { kind, chars } => {
                lines.push(Line::styled(
                    format!("· [{kind}] 注入 · {chars} 字符"),
                    crate::renderer::Style::new().dim(),
                ));
            }
            Node::AssistantBlock { text, thinking, usage } => {
                // 流式块光标：仅落点块追加（turn 收尾后 streaming=false 自动消失）。
                let marker = if Some(idx) == stream_at { "▌" } else { "" };
                lines.extend(render_assistant_block(
                    &format!("{text}{marker}"),
                    if view.show_thinking { thinking } else { "" },
                    usage.as_ref(),
                ));
            }
            Node::ToolCard { name, input_summary, output, is_error } => {
                lines.extend(render_tool_card(
                    name,
                    input_summary,
                    if view.expand_tools { output.as_deref() } else { None },
                    *is_error,
                ));
            }
            Node::AssistantAttempt { error, cancelled } => {
                lines.extend(render_assistant_attempt(error, *cancelled));
            }
            Node::CompactionCard { trigger, summary, shadowed_count, shadowed_tokens, error } => {
                lines.extend(render_compaction_card(trigger, summary.as_deref(), *shadowed_count, *shadowed_tokens, error.as_deref()));
            }
            Node::GoalBar { objective, phase, max_rounds } => {
                lines.extend(render_goal_bar(objective, phase, *max_rounds));
            }
            Node::PlanBadge { active, wanted } => {
                lines.extend(render_plan_badge(*active, *wanted));
            }
            Node::HookLine { point, handler_id, decision } => {
                lines.extend(render_hook_line(point, handler_id, decision.as_deref()));
            }
            Node::WorkspaceLine { summary } => {
                lines.extend(render_workspace_line(summary));
            }
            Node::TodoList { todos } => {
                // todo 整表快照：状态 marker（对齐 opencode：[✓]/[•]/[ ]）+ 文本
                // （pending 灰 / in_progress 亮 / completed 划线）。
                for todo in todos {
                    let (mark, style) = match todo.status {
                        TodoStatus::Completed => ("✓", crate::renderer::Style::new().dim()),
                        TodoStatus::InProgress => ("•", crate::renderer::Style::new()),
                        TodoStatus::Pending => (" ", crate::renderer::Style::new().dim()),
                    };
                    lines.push(Line::styled(format!("[{mark}] {}", todo.content), style));
                }
            }
        }
    }
    lines
}
