//! main 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）：按键路由 + 帧光标。

use super::*;
use crate::model_picker::ModelEntry;
use crossterm::event::KeyEventKind;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent { code, modifiers: KeyModifiers::NONE, kind: KeyEventKind::Press, state: crossterm::event::KeyEventState::NONE }
}

fn ctrl(code: KeyCode) -> KeyEvent {
    KeyEvent { code, modifiers: KeyModifiers::CONTROL, kind: KeyEventKind::Press, state: crossterm::event::KeyEventState::NONE }
}

/// 空目录选择器 + 空编辑器（Ui 的其余字段与路由无关）。
fn ui() -> Ui {
    Ui { picker: ModelPicker::new(Vec::new()), editor: Editor::new(), panel: PromptPanel::new(), active: None, form: ProviderForm::closed(), catalog: onion::provider_catalog::ProviderCatalog::builtin("x"), model: String::new(), cwd: String::new(), turn_stats: None, view: Default::default(), scroll_top: None, scroll_intent: None }
}

#[test]
fn plain_characters_reach_editor_and_enter_submits_line() {
    let mut ui = ui();
    for c in "hello".chars() {
        assert_eq!(route_key(key(KeyCode::Char(c)), &mut ui, false), Routed::Redraw);
    }
    assert_eq!(ui.editor.line(), "> hello");
    assert_eq!(
        route_key(key(KeyCode::Enter), &mut ui, false),
        Routed::Submit("hello".to_string())
    );
    // 提交后缓冲清空（下一轮输入从空行起步）。
    assert_eq!(ui.editor.line(), "> ");
}

#[test]
fn enter_on_empty_buffer_submits_nothing() {
    let mut ui = ui();
    assert_eq!(route_key(key(KeyCode::Enter), &mut ui, false), Routed::Redraw);
}

#[test]
fn esc_and_ctrl_c_quit_when_idle_but_cancel_when_busy() {
    let mut ui = ui();
    route_key(key(KeyCode::Char('x')), &mut ui, false);
    // 空闲：Esc / Ctrl+C = 退出。
    assert_eq!(route_key(key(KeyCode::Esc), &mut ui, false), Routed::Quit);
    assert_eq!(route_key(ctrl(KeyCode::Char('c')), &mut ui, false), Routed::Quit);
    // 在飞：Esc / Ctrl+C = 取消活动轮（不退出）。
    assert_eq!(route_key(key(KeyCode::Esc), &mut ui, true), Routed::Cancel);
    assert_eq!(route_key(ctrl(KeyCode::Char('c')), &mut ui, true), Routed::Cancel);
}

#[test]
fn ctrl_m_opens_picker_and_swallows_following_keys() {
    let mut ui = ui();
    assert_eq!(route_key(ctrl(KeyCode::Char('m')), &mut ui, false), Routed::Redraw);
    assert!(matches!(ui.picker.state(), PickerState::Open { .. }));
    // overlay 打开后字符进过滤框，不再进编辑器。
    route_key(key(KeyCode::Char('q')), &mut ui, false);
    assert_eq!(ui.editor.line(), "> ");
}

#[test]
fn picker_enter_over_empty_catalog_yields_nothing() {
    let mut ui = ui();
    route_key(ctrl(KeyCode::Char('m')), &mut ui, false);
    // 目录为空 → 无可确认条目 → 仅重绘，不产出 /model 行。
    assert_eq!(route_key(key(KeyCode::Enter), &mut ui, false), Routed::Redraw);
    assert!(matches!(ui.picker.state(), PickerState::Closed));
}

#[test]
fn picker_enter_confirms_selected_model() {
    let mut ui = Ui {
        picker: ModelPicker::new(vec![ModelEntry {
            label: "local/Qwen3.8-27B".to_string(),
            model: "Qwen3.8-27B".to_string(),
            description: "http://localhost:8080/v1".to_string(),
        }]),
        editor: Editor::new(),
        panel: PromptPanel::new(),
        active: None,
        form: ProviderForm::closed(),
        catalog: onion::provider_catalog::ProviderCatalog::builtin("x"),
        model: "Qwen3.8-27B".to_string(),
        cwd: String::new(),
        turn_stats: None,
        view: Default::default(),
        scroll_top: None,
        scroll_intent: None,
    };
    route_key(ctrl(KeyCode::Char('m')), &mut ui, false);
    assert_eq!(
        route_key(key(KeyCode::Enter), &mut ui, false),
        Routed::Picked("Qwen3.8-27B".to_string())
    );
}

#[test]
fn arrow_keys_edit_cursor_without_submitting() {
    let mut ui = ui();
    for c in "abc".chars() {
        route_key(key(KeyCode::Char(c)), &mut ui, false);
    }
    route_key(key(KeyCode::Left), &mut ui, false);
    // 光标在 'c' 前（偏移 2）→ 列 = "> " (2) + 2。
    assert_eq!(ui.editor.cursor_col(), 4);
    route_key(key(KeyCode::Char('X')), &mut ui, false);
    assert_eq!(ui.editor.line(), "> abXc");
    route_key(key(KeyCode::Backspace), &mut ui, false);
    assert_eq!(ui.editor.line(), "> abc");
}

#[test]
fn up_key_recalls_last_submitted_line() {
    let mut ui = ui();
    for c in "first".chars() {
        route_key(key(KeyCode::Char(c)), &mut ui, false);
    }
    route_key(key(KeyCode::Enter), &mut ui, false);
    assert_eq!(
        route_key(key(KeyCode::Up), &mut ui, false),
        Routed::Redraw
    );
    assert_eq!(ui.editor.line(), "> first");
    assert_eq!(
        route_key(key(KeyCode::Enter), &mut ui, false),
        Routed::Submit("first".to_string())
    );
}

#[test]
fn frame_places_cursor_on_prompt_row_with_editor_column() {
    let mut ui = ui();
    for c in "hi".chars() {
        route_key(key(KeyCode::Char(c)), &mut ui, false);
    }
    ui.model = "Qwen3.8-27B".to_string();
    let frame = compose(
        &Transcript::new(),
        &FeedbackRing::new(),
        &ui,
        &TurnView { state: Footer::Idle, streaming: false, thinking: None },
        128_000,
        80,
        24,
    );
    // 帧尾：边框三行（输入行在中间）+ footer；光标列 = 缓冲从列 2 起 + "hi" 2 字符。
    assert!(frame.lines[frame.lines.len() - 3].text.starts_with("│ hi "));
    assert!(frame.lines[frame.lines.len() - 2].text.starts_with('╰'));
    // footer：右对齐后仍以 context 开头的判定失效，改为 ends_with（前缀是右侧留白）。
    assert!(frame.lines[frame.lines.len() - 1].text.ends_with("context 0.0% · Qwen3.8-27B"));
    assert_eq!(frame.cursor_col, Some(4));
}

#[test]
fn running_footer_replaces_idle_line_while_turn_in_flight() {
    let ui = ui();
    let idle = compose(
        &Transcript::new(),
        &FeedbackRing::new(),
        &ui,
        &TurnView { state: Footer::Idle, streaming: false, thinking: None },
        128_000,
        80,
        24,
    );
    let running = compose(
        &Transcript::new(),
        &FeedbackRing::new(),
        &ui,
        &TurnView {
            state: Footer::Running { started_at: std::time::Instant::now() },
            streaming: false,
            thinking: None,
        },
        128_000,
        80,
        24,
    );
    // 右对齐后判断内容存在性而非前缀。
    assert!(idle.lines.last().is_some_and(|l| l.text.contains("context ")));
    assert!(running.lines.last().is_some_and(|l| l.text.contains("running ")));
}

#[test]
fn feedback_ring_keeps_only_last_eight_lines() {
    let mut ring = FeedbackRing::new();
    ring.extend((0..12).map(|i| format!("line{i}")));
    let lines: Vec<&String> = ring.lines().collect();
    assert_eq!(lines.len(), 8);
    assert_eq!(lines[0], "line4");
    assert_eq!(lines[7], "line11");
}

#[test]
fn switch_target_maps_to_next_session_and_end_reason() {
    // /new → 下次装配不指定会话 id（新建空会话），durable 原因独立标注。
    assert_eq!(next_session_of(&Switch::New), None);
    assert_eq!(switch_reason(&Switch::New), "switch new");
    // /resume <id> → 下次装配回放该会话日志。
    assert_eq!(
        next_session_of(&Switch::Resume("s-42".to_string())),
        Some("s-42".to_string())
    );
    assert_eq!(switch_reason(&Switch::Resume("s-42".to_string())), "switch resume");
}

#[test]
fn approval_panel_overlay_sits_above_prompt() {
    let mut ui = ui();
    ui.active = Some(Panel::Approval {
        name: "Bash".to_string(),
        input: serde_json::json!({ "command": "ls" }),
    });
    let frame = compose(
        &Transcript::new(),
        &FeedbackRing::new(),
        &ui,
        &TurnView { state: Footer::Idle, streaming: false, thinking: None },
        128_000,
        80,
        24,
    );
    let texts: Vec<&str> = frame.lines.iter().map(|l| l.text.as_str()).collect();
    let approval = texts.iter().position(|t| t.contains("审批：Bash")).expect("审批面板行");
    let prompt = texts.iter().skip(approval + 1).position(|t| t.starts_with("│ ")).expect("输入框行") + approval + 1;
    assert!(approval < prompt, "面板应叠加在输入框之上：{texts:?}");
}

#[test]
fn ctrl_r_and_ctrl_e_toggle_view_state() {
    let mut ui = ui();
    // 默认：推理隐藏、工具折叠。
    assert!(!ui.view.show_thinking);
    assert!(!ui.view.expand_tools);
    assert_eq!(route_key(ctrl(KeyCode::Char('r')), &mut ui, false), Routed::Redraw);
    assert!(ui.view.show_thinking);
    assert_eq!(route_key(ctrl(KeyCode::Char('e')), &mut ui, false), Routed::Redraw);
    assert!(ui.view.expand_tools);
    // 再按一次切回。
    route_key(ctrl(KeyCode::Char('r')), &mut ui, false);
    route_key(ctrl(KeyCode::Char('e')), &mut ui, false);
    assert!(!ui.view.show_thinking);
    assert!(!ui.view.expand_tools);
}

#[test]
fn ctrl_u_requests_view_level_transcript_clear() {
    let mut ui = ui();
    assert_eq!(route_key(ctrl(KeyCode::Char('u')), &mut ui, false), Routed::ClearTranscript);
    // Ctrl+L 仅强制重绘。
    assert_eq!(route_key(ctrl(KeyCode::Char('l')), &mut ui, false), Routed::Redraw);
}

#[test]
fn page_up_down_produce_scroll_intents_without_touching_editor() {
    let mut ui = ui();
    route_key(key(KeyCode::Char('x')), &mut ui, false);
    assert_eq!(route_key(key(KeyCode::PageUp), &mut ui, false), Routed::Redraw);
    assert_eq!(ui.scroll_intent, Some(ScrollIntent::Up(ScrollStep::Page)));
    assert_eq!(ui.editor.line(), "> x", "PgUp 不进编辑器");
    assert_eq!(route_key(key(KeyCode::PageDown), &mut ui, false), Routed::Redraw);
    assert_eq!(ui.scroll_intent, Some(ScrollIntent::Down(ScrollStep::Page)));
}
