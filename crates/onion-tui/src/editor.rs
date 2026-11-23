//! editor.rs：输入编辑器（idle=send / running=steer）。
//!
//! idle 时 Enter 开新轮（Runtime::run），running 时 Enter 经
//! Inbox::followup 排队成轮。斜杠命令经 commands 分发，不进模型上下文。
//! 单行编辑器：缓冲 + 光标（字符索引）+ 历史（Up/Down 导航）。

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// 输入编辑器状态。
#[derive(Default)]
pub struct Editor {
    buffer: String,
    /// 光标位置（字符索引，非字节——CJK 安全）。
    cursor: usize,
    history: Vec<String>,
    /// 历史导航位置：None = 不在导航态（编辑草稿）。
    history_pos: Option<usize>,
    /// 进入历史导航前的草稿（Down 回到底时恢复）。
    stash: Option<String>,
}

impl Editor {
    pub fn new() -> Self {
        Self::default()
    }

    /// 按键处理：返回 Some(line) = 提交一行（Enter）。
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<String> {
        match key.code {
            KeyCode::Enter => return self.submit(),
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    let byte = char_boundary(&self.buffer, self.cursor - 1);
                    self.buffer.remove(byte);
                    self.cursor -= 1;
                }
            }
            KeyCode::Delete => {
                if self.cursor < self.buffer.chars().count() {
                    let byte = char_boundary(&self.buffer, self.cursor);
                    self.buffer.remove(byte);
                }
            }
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right => {
                self.cursor = (self.cursor + 1).min(self.buffer.chars().count());
            }
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.buffer.chars().count(),
            KeyCode::Up => self.history_prev(),
            KeyCode::Down => self.history_next(),
            KeyCode::Char(c)
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                // Ctrl 组合键不进 buffer（Ctrl+C/Ctrl+D 由主循环拦截）。
                let byte = char_boundary(&self.buffer, self.cursor);
                self.buffer.insert(byte, c);
                self.cursor += 1;
            }
            _ => {}
        }
        None
    }

    /// 提交：非空行入历史并清空缓冲。
    fn submit(&mut self) -> Option<String> {
        let line = self.buffer.trim().to_string();
        if line.is_empty() {
            return None;
        }
        self.history.push(line.clone());
        self.buffer.clear();
        self.cursor = 0;
        self.history_pos = None;
        self.stash = None;
        Some(line)
    }

    /// Up：进入/前移历史导航。
    fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let pos = match self.history_pos {
            None => {
                // 首次 Up：暂存草稿，定位到最新一条。
                self.stash = Some(self.buffer.clone());
                self.history.len() - 1
            }
            Some(p) => p.saturating_sub(1),
        };
        self.history_pos = Some(pos);
        self.buffer = self.history[pos].clone();
        self.cursor = self.buffer.chars().count();
    }

    /// Down：后移历史导航；越过末尾恢复草稿。
    fn history_next(&mut self) {
        let Some(pos) = self.history_pos else { return };
        if pos + 1 >= self.history.len() {
            // 越过末尾：恢复草稿，退出导航态。
            self.buffer = self.stash.take().unwrap_or_default();
            self.history_pos = None;
        } else {
            self.history_pos = Some(pos + 1);
            self.buffer = self.history[pos + 1].clone();
        }
        self.cursor = self.buffer.chars().count();
    }

    /// 渲染行：prompt + buffer（光标位置由宿主定位）。
    pub fn line(&self) -> String {
        format!("> {}", self.buffer)
    }

    /// 光标列偏移（prompt 宽度 + buffer 内偏移，按**显示宽度**累计）。
    ///
    /// CJK 宽字符占 2 列：按字符数算会让光标落在字中间（半格错位）。
    pub fn cursor_col(&self) -> u16 {
        let prefix_width: usize = self.buffer[..char_boundary(&self.buffer, self.cursor)]
            .chars()
            .map(char_width)
            .sum();
        (2 + prefix_width) as u16
    }
}

/// 单字符显示宽度（East Asian Wide/Fullwidth 近似表，覆盖教学场景常用区间）。
pub fn char_width(c: char) -> usize {
    let cp = c as u32;
    let wide = matches!(cp,
        0x1100..=0x115F        // Hangul Jamo
        | 0x2E80..=0x303E      // CJK 部首..CJK 符号标点
        | 0x3041..=0x33FF      // 假名..CJK 兼容
        | 0x3400..=0x4DBF      // CJK 扩展 A
        | 0x4E00..=0x9FFF      // CJK 统一表意
        | 0xA000..=0xA4CF      // 彝文
        | 0xAC00..=0xD7A3      // 谚文音节
        | 0xF900..=0xFAFF      // CJK 兼容表意
        | 0xFE30..=0xFE4F      // CJK 兼容形式
        | 0xFF00..=0xFF60      // 全角形式
        | 0xFFE0..=0xFFE6      // 全角符号
        | 0x1F300..=0x1FAFF    // Emoji（近似 2 列）
        | 0x20000..=0x2FFFD    // CJK 扩展 B+
    );
    if wide { 2 } else { 1 }
}

/// 第 n 个字符的字节边界（字符索引 → 字节索引）。
fn char_boundary(s: &str, n: usize) -> usize {
    s.char_indices().nth(n).map(|(b, _)| b).unwrap_or(s.len())
}

#[cfg(test)]
#[path = "editor_tests.rs"]
mod tests;
