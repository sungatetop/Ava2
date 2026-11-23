//! banner.rs：TUI 启动横幅（对齐参考稿 TRAE CLI banner 的三段式）。
//!
//! 结构：虚线 → ANSI Shadow 风格 `ONION` 大字（绿色）→ 虚线 → `>>` 两列
//! 信息区（身份 + model / cwd + 恢复提示）→ 通栏快捷键行 → 空行。横幅随
//! transcript 增长自然滚出视口（append-origin：每帧重放于帧顶，diff 渲染
//! 后仅首次落屏，无闪烁代价）。

use crate::renderer::{Line, Style};
use crossterm::style::Color;

/// 虚线长度（对齐参考稿通栏分隔线宽度）。
const RULE_LEN: usize = 62;

/// `ONION` 大字（ANSI Shadow 风格，O-N-I-O-N 五块 × 45 列，行尾空白已裁剪）。
const ART: [&str; 6] = [
    " ██████╗  ███╗   ██╗ ██╗  ██████╗  ███╗   ██╗",
    "██╔═══██╗ ████╗  ██║ ██║ ██╔═══██╗ ████╗  ██║",
    "██║   ██║ ██╔██╗ ██║ ██║ ██║   ██║ ██╔██╗ ██║",
    "██║   ██║ ██║╚██╗██║ ██║ ██║   ██║ ██║╚██╗██║",
    "╚██████╔╝ ██║ ╚████║ ██║ ╚██████╔╝ ██║ ╚████║",
    " ╚═════╝  ╚═╝  ╚═══╝ ╚═╝  ╚═════╝  ╚═╝  ╚═══╝",
];

/// 信息区左列宽度（`>>` 两列布局的列对齐位）。
const LEFT_COL: usize = 38;

/// 组装启动横幅：cwd / model 为启动时刻快照（当前 model 以 footer 为准）。
pub fn banner_lines(cwd: &str, model: &str) -> Vec<Line> {
    let rule = Line::styled(
        "-".repeat(RULE_LEN),
        Style::new().dim(),
    );
    let mut lines = vec![rule.clone()];
    for row in ART {
        lines.push(Line::styled(row.to_string(), Style::new().color(Color::DarkGreen)));
    }
    lines.push(rule);
    lines.push(Line::plain(format!(
        "{:<LEFT_COL$}>> Model: {model}",
        ">> ONION TUI"
    )));
    lines.push(Line::plain(format!(
        "{:<LEFT_COL$}>> /resume <id> 恢复会话",
        format!(">> cwd: {cwd}")
    )));
    lines.push(Line::plain(
        ">> Esc 退出 · Ctrl+M 切换模型 · Ctrl+R 推理 · Ctrl+E 工具 · Ctrl+U 清屏",
    ));
    lines.push(Line::plain(String::new()));
    lines
}

#[cfg(test)]
#[path = "banner_tests.rs"]
mod tests;
