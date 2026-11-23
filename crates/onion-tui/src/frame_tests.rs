//! frame.rs 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;
use onion::session::SessionEvent;
use onion::types::AgentMessage;

fn sample_nodes() -> Vec<Node> {
    let mut t = crate::transcript::Transcript::new();
    t.replay(&[
        SessionEvent::SessionStarted {
            session_id: "s1".to_string(),
            model: "m1".to_string(),
        },
        SessionEvent::UserMessage {
            turn: 1,
            step: 1,
            message: AgentMessage::user("hi"),
        },
    ]);
    t.nodes().to_vec()
}

fn input<'a>(nodes: &'a [Node], prompt: &str, cursor: u16) -> FrameInput<'a> {
    FrameInput {
        transcript: nodes,
        feedback: &[],
        overlay: &[],
        prompt: prompt.to_string(),
        cursor_col: cursor,
        footer: Line::plain("f"),
        view: ViewState::default(),
        banner: Vec::new(),
        hints: Vec::new(),
        thinking: None,
        width: 40,
        streaming: false,
        // 高度取大值：内容段不被视口裁剪（裁剪行为由滚动专测覆盖）。
        height: 100,
        scroll_top: None,
        scroll_intent: None,
    }
}

#[test]
fn frame_order_is_transcript_feedback_overlay_box_footer() {
    let nodes = sample_nodes();
    let feedback = vec!["（ok）".to_string()];
    let overlay = vec![crate::renderer::Line::plain("overlay-line")];
    let frame = assemble(FrameInput {
        feedback: &feedback,
        overlay: &overlay,
        prompt: "> typed".to_string(),
        ..input(&nodes, "> typed", 7)
    });
    let texts: Vec<&str> = frame.lines.iter().map(|l| l.text.as_str()).collect();
    let idx = |needle: &str| texts.iter().position(|t| t.contains(needle)).expect("missing line");
    assert!(idx("onion session") < idx("（ok）"));
    assert!(idx("（ok）") < idx("overlay-line"));
    assert!(idx("overlay-line") < idx("typed"));
    assert!(idx("╰") < idx("f"), "footer 在边框底之下");
}

#[test]
fn input_box_has_rounded_border_and_status_colors() {
    let frame = assemble(input(&[], "> hi", 4));
    // 边框三行：满宽 39 列（width-1），输入行内容居中含缓冲。
    assert_eq!(frame.lines[0].text, format!("╭{}╮", "─".repeat(37)));
    assert!(frame.lines[1].text.starts_with("│ hi "));
    assert!(frame.lines[1].text.ends_with(" │"));
    assert_eq!(frame.lines[2].text, format!("╰{}╯", "─".repeat(37)));
    // 颜色状态：idle dim / busy 黄。
    assert!(frame.lines[0].style.dim);
    let busy = assemble(FrameInput { streaming: true, ..input(&[], "> hi", 4) });
    assert_eq!(busy.lines[0].style.color, Some(Color::Yellow));
    assert!(!busy.lines[0].style.dim);
}

#[test]
fn idle_empty_buffer_shows_placeholder_but_busy_does_not() {
    let idle = assemble(input(&[], "> ", 2));
    assert!(idle.lines[1].text.contains(PLACEHOLDER));
    // 光标停在 "> " 之后（框内缓冲从列 2 起，空缓冲 = 列 2）。
    assert_eq!(idle.cursor_col, Some(2));
    assert_eq!(idle.prompt_row, Some(1));

    let busy = assemble(FrameInput { streaming: true, ..input(&[], "> ", 2) });
    assert!(!busy.lines[1].text.contains(PLACEHOLDER), "busy 不显示占位符");
}

#[test]
fn cursor_shifts_into_box_and_clamps_to_border() {
    // editor 列 4（"> ab"）→ 框内缓冲从列 2 起，"ab" 后 = 列 4。
    let frame = assemble(input(&[], "> ab", 4));
    assert_eq!(frame.cursor_col, Some(4));
    // 超长输入光标 clamp 到右边框内。
    let long = format!("> {}", "x".repeat(60));
    let clamped = assemble(input(&[], &long, 62));
    assert_eq!(clamped.cursor_col, Some(37));
    // 超宽内容被截断，右边框完整。
    assert!(clamped.lines[1].text.ends_with(" │"));
}

#[test]
fn thinking_line_sits_above_box_with_dim_style() {
    let nodes = sample_nodes();
    let frame = assemble(FrameInput { thinking: Some((12, 441)), ..input(&nodes, "> ", 2) });
    let texts: Vec<&str> = frame.lines.iter().map(|l| l.text.as_str()).collect();
    let thinking = texts.iter().position(|t| t.contains("Thinking… (12s · 441 ch)")).expect("Thinking 行");
    assert!(frame.lines[thinking].style.dim);
    // 无 assistant 输出时省略字符数。
    let fresh = assemble(FrameInput { thinking: Some((3, 0)), ..input(&nodes, "> ", 2) });
    assert!(fresh.lines.iter().any(|l| l.text == "· Thinking… (3s)"));
    let none = assemble(input(&nodes, "> ", 2));
    assert!(!none.lines.iter().any(|l| l.text.contains("Thinking")));
}

#[test]
fn banner_sits_above_transcript_in_frame() {
    let nodes = sample_nodes();
    let frame = assemble(FrameInput {
        banner: crate::banner::banner_lines("C:\\w", "m1"),
        ..input(&nodes, "> ", 2)
    });
    // 横幅在最前，其后才是 transcript 投影（onion session 头）。
    let texts: Vec<&str> = frame.lines.iter().map(|l| l.text.as_str()).collect();
    let banner_start = texts.iter().position(|t| t.starts_with(">> ONION TUI")).expect("banner 身份行");
    let header = texts.iter().position(|t| t.contains("onion session")).expect("会话头");
    assert!(banner_start < header);
    assert!(texts[0].starts_with("--"), "横幅首行应为虚线");
}

#[test]
fn hints_sit_above_box_and_cursor_tracks_input_row() {
    let nodes = sample_nodes();
    let hints = vec![
        Line::plain("/new  新建会话"),
        Line::plain("/resume [id]  恢复历史会话"),
    ];
    let frame = assemble(FrameInput { hints, ..input(&nodes, "> /", 3) });
    // 顺序：hints → 边框顶 → 输入行（光标宿主）→ 边框底 → footer。
    let texts: Vec<&str> = frame.lines.iter().map(|l| l.text.as_str()).collect();
    let hint = texts.iter().position(|t| t.starts_with("/new")).unwrap();
    assert_eq!(texts[hint + 2], format!("╭{}╮", "─".repeat(37)));
    assert!(texts[hint + 3].starts_with("│ / "));
    assert_eq!(frame.prompt_row, Some(hint + 3));
    assert_eq!(frame.cursor_col, Some(3));
}

#[test]
fn streaming_appends_block_cursor_to_last_assistant_line() {
    // 流式：最后一个非空 assistant 行尾跟 ▌；turn 收尾（streaming=false）后消失。
    let mut t = crate::transcript::Transcript::new();
    t.replay(&[
        SessionEvent::SessionStarted { session_id: "s".into(), model: "m".into() },
        SessionEvent::AssistantMessage {
            turn: 1,
            step: 1,
            message: AgentMessage::assistant(vec![onion::types::ContentBlock::Text {
                text: "你好".to_string(),
            }]),
            usage: onion::types::Usage::default(),
        },
    ]);
    let nodes = t.nodes().to_vec();
    let frame = assemble(FrameInput { streaming: true, ..input(&nodes, "> ", 2) });
    let streamed = frame.lines.iter().any(|l| l.text.contains("你好▌"));
    assert!(streamed, "流式行应带块光标：{:?}", frame.lines.iter().map(|l| &l.text).collect::<Vec<_>>());

    let frame_done = assemble(input(&nodes, "> ", 2));
    assert!(!frame_done.lines.iter().any(|l| l.text.contains('▌')));
}

// —— 视口滚动（回看历史）——
// height=24：viewport 23 行，pinned 4 行（边框三行 + footer），内容段窗口 19 行。

#[test]
fn follow_shows_tail_and_page_up_freezes_window() {
    let feedback: Vec<String> = (0..200).map(|i| format!("fb{i}")).collect();
    // 跟随：窗口 = 内容段尾部（fb181..fb199），输入框 + footer 恒驻。
    let follow = assemble(FrameInput {
        height: 24,
        feedback: &feedback,
        ..input(&[], "> ", 2)
    });
    assert_eq!(follow.lines.len(), 23);
    assert!(follow.lines[0].text.contains("fb181"), "跟随窗口应从尾部起：{}", follow.lines[0].text);
    assert_eq!(follow.scroll_top, None);
    assert!(follow.lines[19].text.starts_with('╭'), "输入框恒驻屏底");
    assert!(follow.lines.last().unwrap().text == "f");

    // PgUp 整页：冻结顶 = 181 - 19 = 162 → 窗口 fb162..fb180。
    let frozen = assemble(FrameInput {
        height: 24,
        feedback: &feedback,
        scroll_intent: Some(ScrollIntent::Up(ScrollStep::Page)),
        ..input(&[], "> ", 2)
    });
    assert_eq!(frozen.scroll_top, Some(162));
    assert!(frozen.lines[0].text.contains("fb162"));
    assert!(frozen.lines[19].text.starts_with('╭'), "回看时输入框仍驻屏底");
    assert_eq!(frozen.prompt_row, Some(20), "光标仍宿主输入行");
}

#[test]
fn scroll_walks_pages_and_bottom_resumes_follow() {
    let feedback: Vec<String> = (0..200).map(|i| format!("fb{i}")).collect();
    // 冻结顶 10 再上翻一页 → 饱和到 0 顶。
    let top = assemble(FrameInput {
        height: 24,
        feedback: &feedback,
        scroll_top: Some(10),
        scroll_intent: Some(ScrollIntent::Up(ScrollStep::Page)),
        ..input(&[], "> ", 2)
    });
    assert_eq!(top.scroll_top, Some(0));
    // PgDn 到底：结算触底 → 自动解除冻结（None = 恢复跟随）。
    let bottom = assemble(FrameInput {
        height: 24,
        feedback: &feedback,
        scroll_top: Some(170),
        scroll_intent: Some(ScrollIntent::Down(ScrollStep::Page)),
        ..input(&[], "> ", 2)
    });
    assert_eq!(bottom.scroll_top, None);
    // PgDn 未到底：冻结顶推进但保持冻结。
    let mid = assemble(FrameInput {
        height: 24,
        feedback: &feedback,
        scroll_top: Some(100),
        scroll_intent: Some(ScrollIntent::Down(ScrollStep::Page)),
        ..input(&[], "> ", 2)
    });
    assert_eq!(mid.scroll_top, Some(119));
    // 滚轮 3 行。
    let wheel = assemble(FrameInput {
        height: 24,
        feedback: &feedback,
        scroll_top: Some(10),
        scroll_intent: Some(ScrollIntent::Up(ScrollStep::Lines(3))),
        ..input(&[], "> ", 2)
    });
    assert_eq!(wheel.scroll_top, Some(7));
}

#[test]
fn frozen_window_stays_put_while_content_grows() {
    // 冻结顶锚定内容：流式追加不推移窗口（回看不被新内容冲走）。
    let fb200: Vec<String> = (0..200).map(|i| format!("fb{i}")).collect();
    let fb210: Vec<String> = (0..210).map(|i| format!("fb{i}")).collect();
    let frozen = assemble(FrameInput {
        height: 24,
        feedback: &fb200,
        scroll_top: Some(10),
        ..input(&[], "> ", 2)
    });
    let grown = assemble(FrameInput {
        height: 24,
        feedback: &fb210,
        scroll_top: Some(10),
        ..input(&[], "> ", 2)
    });
    assert!(frozen.lines[0].text.contains("fb10"));
    assert!(grown.lines[0].text.contains("fb10"), "冻结窗口不应随内容增长移动");
    // 跟随态对照：窗口随内容增长下移。
    let follow_grown = assemble(FrameInput {
        height: 24,
        feedback: &fb210,
        ..input(&[], "> ", 2)
    });
    assert!(follow_grown.lines[0].text.contains("fb191"));
}

#[test]
fn content_shrink_clamps_frozen_top_and_unfreezes_at_bottom() {
    let fb200: Vec<String> = (0..200).map(|i| format!("fb{i}")).collect();
    let fb30: Vec<String> = (0..30).map(|i| format!("fb{i}")).collect();
    // 冻结顶 100 后内容收缩到 30 行（如清屏）：夹取到 max_top=11 = 触底 → 解冻跟随。
    let shrunk = assemble(FrameInput {
        height: 24,
        feedback: &fb200,
        scroll_top: Some(100),
        ..input(&[], "> ", 2)
    });
    assert_eq!(shrunk.scroll_top, Some(100));
    let clamped = assemble(FrameInput {
        height: 24,
        feedback: &fb30,
        scroll_top: Some(100),
        ..input(&[], "> ", 2)
    });
    assert_eq!(clamped.scroll_top, None);
    // 冻结顶仍在窗口内（未触底）：保持冻结。
    let mid = assemble(FrameInput {
        height: 24,
        feedback: &fb30,
        scroll_top: Some(5),
        ..input(&[], "> ", 2)
    });
    assert_eq!(mid.scroll_top, Some(5));
    // 内容不足一屏：强制跟随。
    let tiny = assemble(FrameInput {
        height: 24,
        feedback: &fb30[..5],
        scroll_top: Some(0),
        ..input(&[], "> ", 2)
    });
    assert_eq!(tiny.scroll_top, None);
}
