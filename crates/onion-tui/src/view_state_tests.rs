//! view_state 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;

#[test]
fn defaults_hide_thinking_and_collapse_tools() {
    let view = ViewState::default();
    assert!(!view.show_thinking);
    assert!(!view.expand_tools);
}

#[test]
fn toggles_flip_and_report_new_state() {
    let mut view = ViewState::default();
    assert!(view.toggle_thinking(), "首次切到显隐");
    assert!(view.show_thinking);
    assert!(!view.toggle_thinking(), "再次切回隐藏");
    assert!(!view.show_thinking);

    assert!(view.toggle_tools());
    assert!(view.expand_tools);
    assert!(!view.toggle_tools());
    assert!(!view.expand_tools);
}

#[test]
fn toggles_are_independent() {
    let mut view = ViewState::default();
    view.toggle_tools();
    // 工具展开不应影响推理显隐。
    assert!(view.expand_tools);
    assert!(!view.show_thinking);
}
