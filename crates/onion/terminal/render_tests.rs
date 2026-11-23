//! `render` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use crate::terminal::render::{
    format_close, format_list, format_open, format_read, format_send, format_signal,
    truncate_for_render,
};
use crate::terminal::seam::{
    PtySignal, TerminalSessionId, TerminalSessionView, TerminalStatus, WaitReason,
};

#[test]
fn truncate_keeps_utf8_boundary() {
    let text = "你好世界你好世界"; // 每个中文字符 3 字节
    let out = truncate_for_render(text, 4);
    // 截断到 4 个字符 + 标记；不跨码元边界。
    assert!(out.starts_with("你好"));
    assert!(out.ends_with("[output truncated]"));
}

#[test]
fn truncate_short_text_unchanged() {
    assert_eq!(truncate_for_render("hello", 100), "hello");
}

#[test]
fn open_renders_banner_and_motd() {
    let out = format_open("pty-1", "main", "shell", "welcome\n");
    assert_eq!(out, "started terminal session pty-1 (main) [type: shell]\nwelcome");
}

#[test]
fn send_renders_wait_and_session() {
    let out = format_send("$ echo hi\nhi", WaitReason::StdinRead, &TerminalStatus::Running, 1000);
    assert_eq!(out, "$ echo hi\nhi\n[wait: stdin_read]\n[session: running]");
}

#[test]
fn send_renders_exit_status() {
    let out = format_send(
        "done",
        WaitReason::SessionExit,
        &TerminalStatus::Exited { exit_code: 1, signal: None },
        1000,
    );
    assert_eq!(out, "done\n[wait: session_exit]\n[session: exited code=1]");
}

#[test]
fn read_renders_pagination() {
    let lines = vec!["a".to_string(), "b".to_string()];
    let out = format_read(&lines, 10, 12, 1000);
    assert_eq!(out, "a\nb\n[lines: 11-12 of 12]");
}

#[test]
fn signal_renders_delivery() {
    let out = format_signal(PtySignal::Sigint.as_str(), "42");
    assert_eq!(out, "delivered SIGINT to foreground process group 42");
}

#[test]
fn close_renders_confirmation() {
    assert_eq!(format_close("pty-1"), "closed terminal session pty-1");
}

#[test]
fn list_empty() {
    assert_eq!(format_list(&[], 1000), "(no terminal sessions)");
}

#[test]
fn list_renders_sessions() {
    let views = vec![TerminalSessionView {
        id: TerminalSessionId("pty-1".to_string()),
        name: "main".to_string(),
        type_: "shell".to_string(),
        status: TerminalStatus::Running,
        motd: String::new(),
        lines: 3,
    }];
    let out = format_list(&views, 1000);
    assert!(out.contains("pty-1 (main) [type: shell] [lines: 3] — running"));
}
