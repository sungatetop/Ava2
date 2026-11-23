//! editor.rs 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::{Editor, char_boundary};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn typing_inserts_at_cursor() {
    let mut e = Editor::new();
    e.handle_key(key(KeyCode::Char('a')));
    e.handle_key(key(KeyCode::Char('b')));
    e.handle_key(key(KeyCode::Left));
    e.handle_key(key(KeyCode::Char('c')));
    assert_eq!(e.line(), "> acb");
}

#[test]
fn backspace_removes_char_before_cursor() {
    let mut e = Editor::new();
    e.handle_key(key(KeyCode::Char('x')));
    e.handle_key(key(KeyCode::Backspace));
    assert_eq!(e.line(), "> ");
    // 空缓冲 Backspace 无操作。
    e.handle_key(key(KeyCode::Backspace));
    assert_eq!(e.line(), "> ");
}

#[test]
fn delete_removes_char_at_cursor() {
    let mut e = Editor::new();
    e.handle_key(key(KeyCode::Char('a')));
    e.handle_key(key(KeyCode::Char('b')));
    e.handle_key(key(KeyCode::Home));
    e.handle_key(key(KeyCode::Delete));
    assert_eq!(e.line(), "> b");
}

#[test]
fn enter_submits_and_clears() {
    let mut e = Editor::new();
    e.handle_key(key(KeyCode::Char('h')));
    e.handle_key(key(KeyCode::Char('i')));
    let line = e.handle_key(key(KeyCode::Enter));
    assert_eq!(line.as_deref(), Some("hi"));
    assert_eq!(e.line(), "> ");
    // 空行提交返回 None。
    assert_eq!(e.handle_key(key(KeyCode::Enter)), None);
}

#[test]
fn history_navigation_up_down() {
    let mut e = Editor::new();
    e.handle_key(key(KeyCode::Char('1')));
    e.handle_key(key(KeyCode::Enter));
    e.handle_key(key(KeyCode::Char('2')));
    e.handle_key(key(KeyCode::Enter));

    // Up：定位最新一条。
    e.handle_key(key(KeyCode::Up));
    assert_eq!(e.line(), "> 2");
    // 再 Up：前一条。
    e.handle_key(key(KeyCode::Up));
    assert_eq!(e.line(), "> 1");
    // Down：回到最新一条。
    e.handle_key(key(KeyCode::Down));
    assert_eq!(e.line(), "> 2");
    // 再 Down：越过末尾，恢复草稿（空）。
    e.handle_key(key(KeyCode::Down));
    assert_eq!(e.line(), "> ");
}

#[test]
fn history_preserves_draft() {
    let mut e = Editor::new();
    e.handle_key(key(KeyCode::Char('a')));
    e.handle_key(key(KeyCode::Enter));
    e.handle_key(key(KeyCode::Char('d')));
    e.handle_key(key(KeyCode::Char('r')));
    e.handle_key(key(KeyCode::Char('a')));
    e.handle_key(key(KeyCode::Char('f')));
    e.handle_key(key(KeyCode::Char('t')));
    e.handle_key(key(KeyCode::Up));
    assert_eq!(e.line(), "> a");
    e.handle_key(key(KeyCode::Down));
    assert_eq!(e.line(), "> draft");
}

#[test]
fn cjk_cursor_math() {
    let mut e = Editor::new();
    e.handle_key(key(KeyCode::Char('你')));
    e.handle_key(key(KeyCode::Char('好')));
    // "你好" 各占 2 显示列 → 光标在末尾 = "> " (2) + 4。
    assert_eq!(e.cursor_col(), 6);
    e.handle_key(key(KeyCode::Left));
    // 光标移到 "好" 之前（仅 prefix="你" 宽 2）→ 列 2+2=4。
    assert_eq!(e.cursor_col(), 4);
    e.handle_key(key(KeyCode::Backspace));
    assert_eq!(e.line(), "> 好");
    // Backspace 删除"你" → cursor 跟着前移到行首 → 列 2。
    assert_eq!(e.cursor_col(), 2);
}

#[test]
fn ctrl_combinations_not_inserted() {
    let mut e = Editor::new();
    e.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert_eq!(e.line(), "> ");
    e.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL));
    assert_eq!(e.line(), "> ");
}

#[test]
fn char_boundary_walks_char_indices() {
    assert_eq!(char_boundary("你好", 0), 0);
    assert_eq!(char_boundary("你好", 1), 3);
    assert_eq!(char_boundary("你好", 2), 6);
    assert_eq!(char_boundary("", 0), 0);
}

#[test]
fn submit_trims_whitespace() {
    let mut e = Editor::new();
    e.handle_key(key(KeyCode::Char(' ')));
    e.handle_key(key(KeyCode::Char('h')));
    e.handle_key(key(KeyCode::Char('i')));
    e.handle_key(key(KeyCode::Char(' ')));
    let line = e.handle_key(key(KeyCode::Enter));
    assert_eq!(line.as_deref(), Some("hi"));
}
