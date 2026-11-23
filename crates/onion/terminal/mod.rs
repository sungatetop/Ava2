//! Terminal 能力 seam：注册表服务 +
//! `terminal_*` 6 件套。
//!
//! 三层结构：
//! - `seam.rs`：owner 隔离的会话注册表服务（id 铸造 / 名称预留 / backend 注册 /
//!   spawn / read / send / signal / kill / list / owner 生命周期清理）+ 类型与错误码；
//! - `render.rs`：纯渲染（spawn MOTD / send viewport / read 分页 / list / signal /
//!   close + UTF-8 截断），无 I/O；
//! - `tools.rs`：`terminal_*` 6 件工具（open / send / read / signal / close / list）。
//!
//! 核心语义：
//! - owner 隔离：会话只对发起它的 agent 可见/可操作（`FOREIGN_SESSION` 拒绝）；
//!   owner 生命周期结束自动清理其全部会话；
//! - waitReason 四值（stdin_read / inferred_idle / timeout / session_exit）——模型面
//!   必须看到，用于判断前台命令是否真结束；
//! - signal 白名单（SIGINT/SIGTERM/SIGKILL/SIGTSTP/SIGHUP）；shell-targeted SIGKILL
//!   拒绝，用 `terminal_close`；
//! - `terminal_send` 的 `run_in_background` 后台化（复用 onion-jobs）。
//!
//! 后端（`pbackend.rs`，S14b）为可注册 provider 的空 seam：onion 用 portable-pty 拉起
//! 真 shell，缺省注册单一 `type="shell"` 后端。

mod pbackend;
mod render;
mod seam;
pub mod tools;

pub use pbackend::{is_ready_marker, PtyBackend, CONTROLLED_PROMPT};
pub use render::{
    format_close, format_list, format_open, format_read, format_send, format_signal,
    truncate_for_render,
};
pub use seam::{
    PtySignal, TerminalError, TerminalErrorCode, TerminalSessionId, TerminalService,
    TerminalStatus, WaitReason, MAX_SCROLLBACK_LINES,
};
pub use tools::TERMINAL_PROMPT_TEXT;

pub use tools::TerminalToolSet;
