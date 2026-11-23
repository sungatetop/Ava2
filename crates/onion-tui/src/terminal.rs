//! terminal.rs：终端所有权。
//!
//! - TTY fail-loud：stdin/stdout 必须都是 TTY，任一不是则报错退出，不静默降级；
//! - alternate screen + raw mode：TUI 拥有整个终端画面，restore 契约兜底；
//! - displayText() 净化：C0/C1 控制符转可见十六进制转义，只有 TUI 产生 ANSI 序列。

use std::io::{self, Stdout, Write};

/// TTY gate：stdin 与 stdout 必须都是 TTY。
/// 失败即退出（不静默降级为行式行为）——管道/脚本场景留给 onion-cli。
pub fn require_tty() -> io::Result<()> {
    use crossterm::tty::IsTty;
    let stdin_tty = io::stdin().is_tty();
    let stdout_tty = io::stdout().is_tty();
    if !stdin_tty {
        return Err(io::Error::new(io::ErrorKind::Unsupported, "stdin is not a TTY (管道/脚本场景请用 onion-cli)"));
    }
    if !stdout_tty {
        return Err(io::Error::new(io::ErrorKind::Unsupported, "stdout is not a TTY (管道/脚本场景请用 onion-cli)"));
    }
    Ok(())
}

/// displayText 净化：C0/C1 控制符（除换行）转可见十六进制转义。
/// 只有 TUI 产生 ANSI 序列；模型输出/工具输出/会话数据经本函数净化后进渲染层。
pub fn display_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\n' | '\r' | '\t' => out.push(c),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\x{:02x}", c as u32));
            }
            c if (c as u32) >= 0x80 && (c as u32) < 0xa0 => {
                out.push_str(&format!("\\x{:02x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

/// 终端会话：alternate screen + raw mode 的所有权封装。
/// Drop 时 restore（panic 时由 panic hook 兜底）。
pub struct Terminal {
    stdout: Stdout,
}

impl Terminal {
    /// 进入 alternate screen + raw mode + 鼠标捕获 + 隐藏光标。
    ///
    /// 鼠标捕获：滚轮事件进应用（alternate screen 无 scrollback，不捕获则
    /// 滚轮无处可去）；文本选择需 Shift+拖拽（终端标准约定）。
    pub fn enter() -> io::Result<Self> {
        use crossterm::cursor::Hide;
        use crossterm::event::EnableMouseCapture;
        use crossterm::terminal::{EnterAlternateScreen, enable_raw_mode};
        enable_raw_mode()?;
        // crossterm 的 execute!(stdout, ...) 一次写多命令（减少闪烁）。
        crossterm::execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture, Hide)?;
        Ok(Self { stdout: io::stdout() })
    }

    /// restore：leave alternate screen + disable raw mode + 关鼠标捕获 + show cursor。
    pub fn restore(&mut self) -> io::Result<()> {
        use crossterm::cursor::Show;
        use crossterm::event::DisableMouseCapture;
        use crossterm::terminal::{LeaveAlternateScreen, disable_raw_mode};
        disable_raw_mode()?;
        crossterm::execute!(self.stdout, LeaveAlternateScreen, DisableMouseCapture, Show)?;
        self.stdout.flush()
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

/// panic hook：restore 终端再 panic，防终端僵死。
pub fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore_now();
        default(info);
    }));
}

/// 即时 restore（供 panic hook / 退出路径调用）。
pub fn restore_now() -> io::Result<()> {
    use crossterm::cursor::Show;
    use crossterm::event::DisableMouseCapture;
    use crossterm::terminal::{LeaveAlternateScreen, disable_raw_mode};
    disable_raw_mode()?;
    crossterm::execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture, Show)?;
    io::stdout().flush()
}

#[cfg(test)]
#[path = "terminal_tests.rs"]
mod terminal_tests;
