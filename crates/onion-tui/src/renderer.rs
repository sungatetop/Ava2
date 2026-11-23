//! renderer.rs：渲染后端（crossterm 生产后端 + 内存后端供快照测试）。
//!
//! 组件树 → 文本帧：append-origin 组件按序渲染，统一通过 `Line` 抽象
//! （文本 + 样式），样式层用 16 色 ANSI + reverse video（亮/暗主题
//! 由宿主终端 remap，不做 TUI 主题设置）。
//!
//! 生产后端采用**行级 diff 渲染**（对齐 ratatui buffer-diff 的行粒度版）：
//! 缓存上一帧的屏幕行，新帧只重写变化行、不清屏——流式 delta 高频重绘不再
//! 闪烁；仅终端 resize 时全清重画。viewport 取终端尾部窗口（底部留一行防
//! 滚动），窗口滑动时退化为整窗重写（无清屏，同样不闪）。

use crossterm::style::Color;
use std::io::{self, Write};

/// 样式（16 色 + dim + reverse）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub color: Option<Color>,
    pub dim: bool,
    pub reverse: bool,
}

impl Style {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub fn dim(mut self, ) -> Self {
        self.dim = true;
        self
    }

    pub fn reverse(mut self) -> Self {
        self.reverse = true;
        self
    }
}

/// 行内高亮段（字节索引，`String::find` 产物保证 char 边界）：基样式之外
/// 叠加一段前景色（如 footer 里的 model 名红色）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccentSpan {
    pub start: usize,
    pub end: usize,
    pub color: Color,
}

/// 一行渲染单元（文本 + 样式 + 可选行内高亮段）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub text: String,
    pub style: Style,
    pub accent: Option<AccentSpan>,
}

impl Line {
    pub fn plain(text: impl Into<String>) -> Self {
        Self { text: text.into(), style: Style::new(), accent: None }
    }

    pub fn styled(text: impl Into<String>, style: Style) -> Self {
        Self { text: text.into(), style, accent: None }
    }

    /// 行内高亮段（`start..end` 为 `text` 的字节索引）。
    pub fn accent(mut self, start: usize, end: usize, color: Color) -> Self {
        self.accent = Some(AccentSpan { start, end, color });
        self
    }
}

/// 渲染后端：crossterm 行级 diff（生产）。快照测试走 compose/assemble 纯函数
/// 路径（Frame 结构断言），不依赖具体后端。
///
/// crossterm 后端：行级 diff 渲染（缓存上帧 → 只写变化行 → 定位光标）。
///
/// color:false（`NO_COLOR` 存在即关样式）时跳过 SGR 生成，纯文本渲染；
/// displayText 净化照常——只有 TUI 产生 ANSI 序列（设计文档 §12）。
pub struct CrosstermBackend {
    stdout: io::Stdout,
    color: bool,
    /// 上一帧的屏幕行（含样式序列，无换行）——diff 基准。
    last_frame: Vec<String>,
    /// 上一帧终端尺寸（行，列）——变化即 resize，全清重画。
    last_size: (u16, u16),
}

impl CrosstermBackend {
    pub fn new() -> Self {
        let size = terminal_size();
        Self { stdout: io::stdout(), color: true, last_frame: Vec::new(), last_size: size }
    }

    /// color:false：关样式（纯文本渲染，转义与净化照常）。
    pub fn without_color(mut self) -> Self {
        self.color = false;
        self
    }

    /// 按环境解析色开关：`NO_COLOR` 存在（非空）即关。
    pub fn color_from_env() -> bool {
        std::env::var("NO_COLOR").map(|v| v.is_empty()).unwrap_or(true)
    }

    /// 渲染一帧：viewport 裁剪 → 行级 diff → 只写变化行 → 光标定位。
    ///
    /// `cursor` 为（逻辑行号，列）：逻辑行被窗口裁掉时不定位光标（prompt 恒在
    /// 尾部窗口内，正常不发生）。draw 期间隐藏光标（不随输出游走），帧尾定位
    /// 后显示（ratatui 同款）。
    pub fn render_frame(
        &mut self,
        lines: &[Line],
        cursor: Option<(usize, u16)>,
    ) -> io::Result<()> {
        // resize：全清重画（缓存作废）。清屏只发生在 resize，常态无闪烁源。
        let size = terminal_size();
        if size != self.last_size {
            use crossterm::cursor::MoveTo;
            use crossterm::terminal::{Clear, ClearType};
            let _ = crossterm::execute!(self.stdout, MoveTo(0, 0), Clear(ClearType::All));
            self.last_frame.clear();
            self.last_size = size;
        }

        // viewport：终端尾部窗口，底部留一行防自然滚动错位。
        let viewport_h = (size.1 as usize).saturating_sub(1).max(1);
        let start = lines.len().saturating_sub(viewport_h);
        let window: Vec<String> = lines[start..]
            .iter()
            .map(|line| format_line_text(line, self.color))
            .collect();

        let mut out = String::from("\x1b[?25l"); // Hide（draw 期间光标不随输出游走）
        let plan = diff_rows(&self.last_frame, &window);
        for offset in 0..window.len().saturating_sub(plan.rewrite_from) {
            let row = plan.rewrite_from + offset;
            let text = &window[row];
            // ANSI 光标定位是 1 基行号：0 基逻辑行 row → 终端第 row+1 行。
            // viewport 高度已按 size-1 裁剪，写到末行不触发自然滚动。
            // 行尾 \x1b[0m 重置 SGR：样式不跨行残留（否则 plain 行继承上一行
            // 的 dim/Yellow 等，正文被染成暗色/杂色）。
            out.push_str(&format!("\x1b[{};1H{text}\x1b[0m\x1b[K", row + 1)); // MoveTo + 写行 + 重置 + 清行尾残留
        }
        if let Some(from) = plan.clear_tail_from {
            out.push_str(&format!("\x1b[{};1H\x1b[0m\x1b[0J", from + 1)); // MoveTo + 重置 + 清到屏尾
        }
        if let Some((logical_row, col)) = cursor {
            if logical_row >= start {
                let screen_row = logical_row - start;
                out.push_str(&format!("\x1b[{};{}H\x1b[?25h", screen_row + 1, col + 1)); // MoveTo + Show
            }
        } else {
            out.push_str("\x1b[?25h"); // Show
        }
        self.last_frame = window;
        self.stdout.write_all(out.as_bytes())?;
        self.stdout.flush()
    }
}

impl Default for CrosstermBackend {
    fn default() -> Self {
        Self::new()
    }
}

/// 终端尺寸（size 失败回退 80x24）。
fn terminal_size() -> (u16, u16) {
    crossterm::terminal::size().unwrap_or((80, 24))
}

/// 行级 diff 计划：从首个变化行起整段重写 + 清尾部多余行。
///
/// 流式场景（尾部追加/变化）通常只有少量行需要重写；窗口整体滑动时退化为
/// 全窗重写（无清屏，不闪）。
#[derive(Debug, PartialEq, Eq)]
pub struct FrameDiff {
    /// 从该屏幕行起重写，直到新帧末行。
    pub rewrite_from: usize,
    /// 旧帧比新帧多出的行：从该屏幕行起清到屏尾（None = 无需清除）。
    pub clear_tail_from: Option<usize>,
}

/// 计算两帧屏幕行的 diff（纯函数，供快照测试）。
pub fn diff_rows(prev: &[String], next: &[String]) -> FrameDiff {
    let common = prev.iter().zip(next.iter()).take_while(|(a, b)| a == b).count();
    FrameDiff {
        rewrite_from: common,
        clear_tail_from: if next.len() < prev.len() { Some(next.len()) } else { None },
    }
}

/// 16 色前景 SGR 序列（format_line_text 基样式与 accent 段共用）。
fn push_fg(out: &mut String, color: Color) {
    let code = match color {
        Color::Black => "30",
        Color::DarkGrey => "90",
        Color::Red => "31",
        Color::DarkRed => "91",
        Color::Green => "32",
        Color::DarkGreen => "92",
        Color::Yellow => "33",
        Color::DarkYellow => "93",
        Color::Blue => "34",
        Color::DarkBlue => "94",
        Color::Magenta => "35",
        Color::DarkMagenta => "95",
        Color::Cyan => "36",
        Color::DarkCyan => "96",
        Color::White => "37",
        Color::Grey => "97",
        _ => "0",
    };
    out.push_str("\x1b[");
    out.push_str(code);
    out.push('m');
}

/// 单行渲染组装（纯函数，便于快照测试）：净化 + SGR 前缀 + 文本（无换行）。
pub(crate) fn format_line_text(line: &Line, color: bool) -> String {
    // displayText 净化：模型输出/会话数据/工具呈现进终端前，C0/C1 转可见转义。
    let text = crate::terminal::display_text(&line.text);
    let mut out = String::new();
    if color && (line.style.color.is_some() || line.style.dim || line.style.reverse) {
        out.push_str("\x1b[0m");
        if let Some(color) = line.style.color {
            // 16 色前景。
            push_fg(&mut out, color);
        }
        if line.style.dim {
            out.push_str("\x1b[2m");
        }
        if line.style.reverse {
            out.push_str("\x1b[7m");
        }
    }
    // 行内高亮段：accent 段换前景色 + 重置后重发基样式（段外保持基样式）。
    // 净化不改变文本长度（C0/C1 → 可见转义是等宽替换？否——转义可能变长），
    // 因此 accent 索引按净化后文本重新查找不可行；约定 accent 只用于宿主
    // 自建行（footer），模型文本不携带 accent，索引直接按净化后字符串切分。
    match line.accent {
        Some(span) if span.end <= text.len() && text.is_char_boundary(span.end) => {
            let base = out.clone();
            out.clear();
            let (head, mid, tail) = (
                &text[..span.start],
                &text[span.start..span.end],
                &text[span.end..],
            );
            out.push_str(&base);
            out.push_str(head);
            out.push_str("\x1b[0m");
            push_fg(&mut out, span.color);
            out.push_str(mid);
            out.push_str("\x1b[0m");
            out.push_str(&base);
            out.push_str(tail);
        }
        _ => out.push_str(&text),
    }
    out
}

#[cfg(test)]
#[path = "renderer_tests.rs"]
mod renderer_tests;
