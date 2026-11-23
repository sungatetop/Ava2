//! footer 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;

#[test]
fn idle_renders_ratio_and_model() {
    let t = Transcript::new();
    let f = Footer::Idle;
    let line = f.render(&t, "test-model", 1000, None);
    assert!(line.contains("context"), "missing ratio label: {line}");
    assert!(line.contains("test-model"), "missing model: {line}");
    assert!(!line.contains(" steps"), "无轮历史不显示轮次：{line}");
}

#[test]
fn idle_appends_turn_stats_when_reported() {
    let t = Transcript::new();
    let f = Footer::Idle;
    let line = f.render(&t, "m1", 1000, Some((3, 7)));
    assert!(line.contains("t3 · 7 steps"), "missing turn stats: {line}");
    assert!(line.ends_with(" · m1"));
}

#[test]
fn running_renders_elapsed_and_escape_hint() {
    let t = Transcript::new();
    let f = Footer::Running { started_at: Instant::now() };
    let line = f.render(&t, "test-model", 1000, Some((1, 1)));
    assert!(line.contains("running"), "missing running label: {line}");
    assert!(line.contains("Esc"), "missing escape hint: {line}");
}

#[test]
fn estimate_counts_text_and_thinking() {
    let mut t = Transcript::new();
    let msg = onion::types::AgentMessage::user("12345678"); // 8 chars → 2 tokens
    t.apply(&onion::session::SessionEvent::UserMessage {
        turn: 1,
        step: 1,
        message: msg,
    });
    let estimated = estimate_transcript(&t);
    assert_eq!(estimated, 2);
}
