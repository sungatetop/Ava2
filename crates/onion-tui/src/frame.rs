//! frame.rs：全屏帧组装（终端渲染链路的前端一半）。
//!
//! 一帧 = 内容段（banner → transcript 投影行 → 一次性反馈行 → overlay 行 →
//! Thinking 状态行 → 命令提示）按滚动窗口切片 + 固定段（输入框圆角边框三行
//! → footer 行）。
//!
//! 输入框即「响应区 / 输入区」的视觉划分（对齐参考稿 TRAE CLI）：框上是滚动
//! 会话流，框内是输入行，框下是状态 footer。边框颜色提示状态：idle dim /
//! 轮在飞 Yellow（醒目）。
//!
//! 视口语义（回看历史）：内容段默认跟随底部（尾窗口）；`scroll_top` 冻结顶
//! 非 None 时窗口锚定内容段（新内容不推移窗口），PgUp/PgDn/滚轮意图在本帧
//! 结算，触底自动解除冻结。输入框与 footer 恒驻屏底（回看时仍可输入）。
//!
//! 组装是纯函数（无 IO、无全局状态）：给定投影与状态 → 帧行 + 光标位置 +
//! 滚动结算值，便于语义快照测试。

use crate::renderer::{Line, Style};
use crate::transcript::Node;
use crate::view_state::ViewState;
use crossterm::style::Color;

/// 一帧的内容：行序列 + 光标位置 + 滚动结算值。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub lines: Vec<Line>,
    /// 输入行索引（光标宿主行；位于边框三行的中间行）。
    pub prompt_row: Option<usize>,
    /// 光标列（相对终端绝对列，由宿主按帧首行偏移换算）。
    pub cursor_col: Option<u16>,
    /// 结算后的视口冻结顶（内容段行索引；None = 跟随底部）。宿主回写为下一帧状态。
    pub scroll_top: Option<usize>,
}

/// 滚动步长。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollStep {
    /// 整页（当前内容窗口高度）。
    Page,
    /// 固定行数（鼠标滚轮一格）。
    Lines(u16),
}

/// 滚动意图（PgUp/滚轮产出；`assemble` 结算后经 `Frame::scroll_top` 回传宿主）。
///
/// 无「直接回底」意图：PgDn 推进到触底即自动解除冻结（见 assemble 夹取）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollIntent {
    Up(ScrollStep),
    Down(ScrollStep),
}

/// 帧组装输入（宿主状态快照）。
pub struct FrameInput<'a> {
    /// transcript 投影行（append-origin 组件树）。
    pub transcript: &'a [Node],
    /// 一次性反馈行（命令分发结果 / turn 报告）。
    pub feedback: &'a [String],
    /// overlay 行（/model 选择器 / 表单 / 问题面板，携带样式）。
    pub overlay: &'a [Line],
    /// prompt 行（编辑器输入，`> ` 前缀 + 缓冲）。
    pub prompt: String,
    /// 编辑器光标列（`> ` 前缀口径；组装时换算为边框内绝对列）。
    pub cursor_col: u16,
    /// footer 行（idle/running 双态，可带 model 名 accent 高亮）。
    pub footer: Line,
    /// 视图级开关（推理显隐 / 工具卡片展开，§7.3）。
    pub view: ViewState,
    /// 轮在飞（流式输出中）：最后一个非空 assistant 行尾跟块光标 `▌`。
    pub streaming: bool,
    /// Thinking 状态行数据（elapsed 秒，流式字符数）：轮进行中显示于输入框上方。
    pub thinking: Option<(u64, usize)>,
    /// 启动横幅（banner 三段式，帧顶展示，随滚动移出视口）。
    pub banner: Vec<Line>,
    /// 命令提示行（缓冲以 `/` 开头时的补全菜单，叠加在输入框上方）。
    pub hints: Vec<Line>,
    /// 终端宽度（输入框满宽依据）。
    pub width: u16,
    /// 终端高度（行）——内容段视口裁剪依据。
    pub height: u16,
    /// 视口冻结顶（内容段行索引；None = 跟随底部）。
    pub scroll_top: Option<usize>,
    /// 本帧滚动意图（PgUp/PgDn/滚轮；结算值经 `Frame::scroll_top` 回传宿主）。
    pub scroll_intent: Option<ScrollIntent>,
}

/// 空闲空缓冲时的输入框占位文案（对齐参考稿「Ask anything...」）。
const PLACEHOLDER: &str = "Ask anything...";

/// 组装一帧：内容段（滚动窗口切片）→ 输入框 → footer。
///
/// 叠加序：会话内容在上（响应区），交互区（输入框 + footer）在下；
/// 输入框与 footer 恒驻屏底，不随回看滚动移出。
pub fn assemble(input: FrameInput<'_>) -> Frame {
    // —— 内容段：banner → transcript → feedback → overlay → Thinking → hints ——
    let mut content = input.banner;
    content.extend(crate::components::render_nodes(input.transcript, input.view, input.streaming));
    for line in input.feedback {
        content.push(Line::plain(line.clone()));
    }
    for line in input.overlay {
        content.push(line.clone());
    }
    // Thinking 状态行（轮进行中，dim；对齐参考稿「· Thinking... (14s ↓ 441 B)」）。
    if let Some((secs, chars)) = input.thinking {
        let detail = if chars > 0 {
            format!(" · {chars} ch")
        } else {
            String::new()
        };
        content.push(Line::styled(
            format!("· Thinking… ({secs}s{detail})"),
            Style::new().dim(),
        ));
    }
    content.extend(input.hints);

    // —— 固定段：输入框（满宽圆角边框三行）+ footer，恒驻屏底 ——
    // 右侧留 1 列防换行；颜色提示状态——idle dim / 轮在飞 Yellow；空闲空缓冲
    // 显示 placeholder。
    let w_total = (input.width as usize).max(8).saturating_sub(1);
    let inner = w_total.saturating_sub(4);
    let border = if input.streaming {
        Style::new().color(Color::Yellow)
    } else {
        Style::new().dim()
    };
    let buffer = input
        .prompt
        .strip_prefix("> ")
        .unwrap_or(input.prompt.as_str());
    let body = if buffer.is_empty() && !input.streaming {
        PLACEHOLDER.to_string()
    } else {
        truncate_width(buffer, inner)
    };
    let pad = " ".repeat(inner.saturating_sub(display_width(&body)));
    // 光标列（0 基）：输入行缓冲（无 `> ` 前缀）从第 2 列（`│ `）开始显示；
    // 编辑器 `cursor_col` 的 `> ` 前缀 2 列恰与 `│ ` 的 2 列抵消，故直接复用，
    // 无需再加偏移；clamp 到框内正文右缘。
    let cursor_col = input.cursor_col.min(w_total as u16 - 2);
    let pinned = vec![
        Line::styled(format!("╭{}╮", "─".repeat(w_total - 2)), border),
        Line::styled(format!("│ {body}{pad} │"), border),
        Line::styled(format!("╰{}╯", "─".repeat(w_total - 2)), border),
        input.footer,
    ];

    // —— 视口：内容段窗口切片（冻结顶结算）——
    // 底部留 1 行防自然滚动错位；输入框 + footer 占据其余行，内容段分享剩余。
    let viewport_h = (input.height as usize).saturating_sub(1).max(1);
    let content_h = viewport_h.saturating_sub(pinned.len()).max(1);
    let max_top = content.len().saturating_sub(content_h);
    let step = |s: ScrollStep| match s {
        ScrollStep::Page => content_h,
        ScrollStep::Lines(n) => (n as usize).max(1),
    };
    let scroll_top = match input.scroll_intent {
        Some(ScrollIntent::Up(step_size)) => Some(match input.scroll_top {
            Some(top) => top.saturating_sub(step(step_size)),
            None => max_top.saturating_sub(step(step_size)),
        }),
        Some(ScrollIntent::Down(step_size)) => {
            input.scroll_top.map(|top| (top + step(step_size)).min(max_top))
        }
        None => input.scroll_top,
    };
    // 夹取 + 触底自动解除冻结：内容收缩时对齐窗口；已到底恢复跟随（新内容
    // 不被过期冻结顶挡住，如清屏后 / 新轮开轮）。
    let scroll_top = scroll_top.map(|top| top.min(max_top)).filter(|&top| top < max_top);
    let window = match scroll_top {
        Some(top) => content[top..top + content_h].to_vec(),
        None => {
            let start = content.len().saturating_sub(content_h);
            content[start..].to_vec()
        }
    };

    let mut lines = window;
    let prompt_row = lines.len() + 1;
    lines.extend(pinned);
    Frame {
        lines,
        prompt_row: Some(prompt_row),
        cursor_col: Some(cursor_col),
        scroll_top,
    }
}

/// 字符串显示宽度（CJK 宽字符计 2，复用编辑器宽度表）。
pub(crate) fn display_width(text: &str) -> usize {
    text.chars().map(crate::editor::char_width).sum()
}

/// 按显示宽度截断（超宽输入不换行，保边框完整）。
fn truncate_width(text: &str, max: usize) -> String {
    let mut width = 0;
    let mut out = String::new();
    for ch in text.chars() {
        let ch_width = crate::editor::char_width(ch);
        if width + ch_width > max {
            break;
        }
        width += ch_width;
        out.push(ch);
    }
    out
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod tests;
