//! transcript.rs 语义终端快照测试（固定事件序列 → 投影 → 组件树 → 渲染帧断言）。
//!
//! append-origin 语义的回归锚点：同一事件序列经 resume 全量重放与运行中增量
//! 两条路径必须产出完全一致的组件树（确定性 fold）。渲染帧为纯文本快照。

use crate::transcript::Transcript;
use onion::session::SessionEvent;
use onion::types::{AgentMessage, ContentBlock, ToolCall, ToolResult, Usage};

/// 构造含完整生命周期的事件序列。
fn sample_events() -> Vec<SessionEvent> {
    let call = ToolCall {
        id: "c1".to_string(),
        name: "read_file".to_string(),
        input: serde_json::json!({"path": "a.txt"}),
    };
    let result = ToolResult::text(&call, "hello");
    vec![
        SessionEvent::SessionStarted {
            session_id: "s-abc".to_string(),
            model: "test-model".to_string(),
        },
        SessionEvent::TurnStart { turn: 1 },
        SessionEvent::StepStart { turn: 1, step: 1 },
        SessionEvent::AssistantMessage {
            turn: 1,
            step: 1,
            message: AgentMessage::assistant(vec![ContentBlock::Text {
                text: "working".to_string(),
            }]),
            usage: Usage {
                input_tokens: 10,
                output_tokens: 5,
                cache_read_tokens: 20,
                cache_write_tokens: 2,
            },
        },
        // call/result 各只被消费一次，无需克隆。
        SessionEvent::ToolCall {
            turn: 1,
            step: 1,
            call,
        },
        SessionEvent::ToolResult {
            turn: 1,
            step: 1,
            result,
        },
        SessionEvent::StepEnd { turn: 1, step: 1 },
        SessionEvent::TurnEnd { turn: 1, reason: "end_turn".to_string() },
    ]
}

/// resume 全量重放路径（事件序 → 清空重放）。
fn replay_frame(events: &[SessionEvent]) -> Vec<String> {
    let mut t = Transcript::new();
    t.replay(events);
    render_frame(&t)
}

#[test]
fn system_prompt_event_renders_nothing() {
    // system prompt 落盘事件：正文不渲染（界面清爽），投影不产生任何节点。
    let events = vec![
        SessionEvent::SessionStarted { session_id: "s".into(), model: "m".into() },
        SessionEvent::SystemMessage { turn: 0, step: 0, message: "huge system prompt...".into() },
    ];
    let mut t = Transcript::new();
    t.replay(&events);
    let texts: Vec<String> = t.nodes().iter().map(|n| format!("{n:?}")).collect();
    assert_eq!(t.nodes().len(), 1, "仅会话头，system prompt 不入投影：{texts:?}");
    assert!(matches!(t.nodes()[0], crate::transcript::Node::Header { .. }));
}

#[test]
fn injected_messages_collapse_to_note() {
    // 带 source 标记的注入：折叠为一行注记，不展开正文。
    let mut runtime_ctx = AgentMessage::user("Current runtime context. huge snapshot...".to_string());
    runtime_ctx.source = Some(onion::types::MessageSource::RuntimeContext);
    // 无标记但文本启发式命中（历史日志兼容）。
    let legacy = AgentMessage::user("<system-reminder>workspace rules...</system-reminder>".to_string());
    // 普通用户消息：不折叠。
    let plain = AgentMessage::user("你好".to_string());
    let events = vec![
        SessionEvent::UserMessage { turn: 1, step: 1, message: runtime_ctx },
        SessionEvent::UserMessage { turn: 1, step: 1, message: legacy },
        SessionEvent::UserMessage { turn: 1, step: 1, message: plain },
    ];
    let mut t = Transcript::new();
    t.replay(&events);
    assert_eq!(t.nodes().len(), 3);
    assert_eq!(
        t.nodes()[0],
        crate::transcript::Node::InjectionNote { kind: "runtime-context".into(), chars: "Current runtime context. huge snapshot...".chars().count() }
    );
    assert_eq!(
        t.nodes()[1],
        crate::transcript::Node::InjectionNote { kind: "system-reminder".into(), chars: "<system-reminder>workspace rules...</system-reminder>".chars().count() }
    );
    // 正文不出现在第三条（普通消息保留全文节点）。
    assert!(matches!(t.nodes()[2], crate::transcript::Node::UserMessage { .. }));
}

/// 运行中增量路径（事件逐条 apply）。
fn incremental_frame(events: &[SessionEvent]) -> Vec<String> {
    let mut t = Transcript::new();
    for e in events {
        t.apply(e);
    }
    render_frame(&t)
}

fn render_frame(t: &Transcript) -> Vec<String> {
    let lines = crate::components::render_nodes(t.nodes(), crate::view_state::ViewState::default(), false);
    // 快照呈现终端实际显示的文本：与 format_line 同点净化（display_text）。
    lines
        .iter()
        .map(|l| crate::terminal::display_text(&l.text))
        .collect()
}

#[test]
fn replay_and_incremental_produce_identical_frames() {
    let events = sample_events();
    let a = replay_frame(&events);
    let b = incremental_frame(&events);
    assert_eq!(a, b, "resume 全量重放与运行中增量必须产出同一组件树");
}

#[test]
fn frame_contains_session_header() {
    let frame = replay_frame(&sample_events());
    assert!(frame.iter().any(|l| l.contains("onion session")));
    assert!(frame.iter().any(|l| l.contains("s-abc")));
    assert!(frame.iter().any(|l| l.contains("test-model")));
}

#[test]
fn frame_contains_user_message_and_assistant_block() {
    let events = vec![
        SessionEvent::UserMessage {
            turn: 1,
            step: 1,
            message: AgentMessage::user("hello world"),
        },
        SessionEvent::AssistantMessage {
            turn: 1,
            step: 2,
            message: AgentMessage::assistant(vec![ContentBlock::Text {
                text: "answer text".to_string(),
            }]),
            usage: Usage {
                input_tokens: 100,
                output_tokens: 50,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
            },
        },
    ];
    let frame = replay_frame(&events);
    assert!(frame.iter().any(|l| l.contains("hello world")));
    assert!(frame.iter().any(|l| l.contains("answer text")));
    assert!(frame.iter().any(|l| l.contains("in 100 out 50")));
}

#[test]
fn frame_contains_tool_card() {
    let mut t = Transcript::new();
    t.replay(&sample_events());
    // 工具输出默认折叠（§7.3）；展开视图下断言输出渲染路径。
    let expanded = crate::view_state::ViewState { show_thinking: true, expand_tools: true };
    let frame: Vec<String> = crate::components::render_nodes(t.nodes(), expanded, false)
        .iter()
        .map(|l| crate::terminal::display_text(&l.text))
        .collect();
    assert!(frame.iter().any(|l| l.contains("read_file")));
    assert!(frame.iter().any(|l| l.contains("path=a.txt")));
    assert!(frame.iter().any(|l| l.contains("hello")));
}

#[test]
fn turn_separator_node_renders_as_dot_title() {
    // turn 分隔节点渲染为圆点标题（对齐 opencode 分组行）；节点保留
    // （durable 投影 + token 估算）。
    let frame = replay_frame(&sample_events());
    assert!(frame.iter().any(|l| l.contains("● turn 1")));
    let t = Transcript::new();
    let mut t = t;
    t.replay(&sample_events());
    assert!(t.nodes().iter().any(
        |n| matches!(n, crate::transcript::Node::TurnSeparator { turn: 1, .. })
    ));
}

#[test]
fn frame_contains_compaction_card() {
    let events = vec![
        SessionEvent::CompactionStart {
            turn: 1,
            compaction_id: "k1".to_string(),
            trigger: "manual".to_string(),
        },
        SessionEvent::CompactionSummary {
            turn: 1,
            compaction_id: "k1".to_string(),
            summary: "summary text".to_string(),
            shadowed_count: 5,
            shadowed_tokens: 1000,
            usage: Usage {
                input_tokens: 0,
                output_tokens: 0,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
            },
        },
        SessionEvent::CompactionEnd {
            turn: 1,
            compaction_id: "k1".to_string(),
            error: None,
        },
    ];
    let frame = replay_frame(&events);
    assert!(frame.iter().any(|l| l.contains("compaction")));
    assert!(frame.iter().any(|l| l.contains("summary text")));
}

#[test]
fn frame_contains_goal_bar_and_plan_badge() {
    let events = vec![
        SessionEvent::PlanMode { active: true },
        SessionEvent::PlanWanted { active: false },
    ];
    let frame = replay_frame(&events);
    assert!(frame.iter().any(|l| l.contains("plan: on")));
    assert!(frame.iter().any(|l| l.contains("wanted: off")));
}

#[test]
fn frame_contains_todo_list() {
    let todos = vec![
        onion::todo::types::TodoItem {
            content: "task a".to_string(),
            status: onion::todo::types::TodoStatus::Completed,
        },
        onion::todo::types::TodoItem {
            content: "task b".to_string(),
            status: onion::todo::types::TodoStatus::InProgress,
        },
        onion::todo::types::TodoItem {
            content: "task c".to_string(),
            status: onion::todo::types::TodoStatus::Pending,
        },
    ];
    let events = vec![SessionEvent::TodoWrite { todos }];
    let frame = replay_frame(&events);
    assert!(frame.iter().any(|l| l.contains("task a")));
    assert!(frame.iter().any(|l| l.contains("task b")));
    assert!(frame.iter().any(|l| l.contains("task c")));
}

#[test]
fn frame_sanitizes_embedded_ansi_in_model_output() {
    // 模型输出携带 ANSI 序列 → displayText 净化（转可见转义）。
    let events = vec![SessionEvent::AssistantMessage {
        turn: 1,
        step: 1,
        message: AgentMessage::assistant(vec![ContentBlock::Text {
            text: "a\u{1b}[31mb".to_string(),
        }]),
        usage: Usage {
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        },
    }];
    let frame = replay_frame(&events);
    assert!(
        frame.iter().any(|l| l.contains("\\x1b[31m")),
        "模型输出中的 ANSI 序列必须被净化为可见转义: {frame:?}"
    );
}

#[test]
fn frame_is_deterministic_across_replays() {
    // 确定性 fold：同一序列重放两次，帧必须逐字节一致。
    let events = sample_events();
    assert_eq!(replay_frame(&events), replay_frame(&events));
}
