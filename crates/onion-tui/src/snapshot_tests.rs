//! 语义终端快照测试（设计文档 §14）。
//!
//! 固定 SessionEvent 序列 → Transcript.replay（append-origin 投影）→
//! components::render_nodes（组件树）→ 文本帧断言。整帧快照：任何投影/组件
//! 改动都会破坏帧，强制显式更新预期。

use onion::session::SessionEvent;
use onion::types::{AgentMessage, ContentBlock, Usage};

fn assistant(text: &str, usage: Usage) -> SessionEvent {
    SessionEvent::AssistantMessage {
        turn: 1,
        step: 1,
        message: AgentMessage::assistant(vec![ContentBlock::Text { text: text.to_string() }]),
        usage,
    }
}

#[test]
fn full_frame_replay_projection() {
    let events = vec![
        SessionEvent::SessionStarted {
            session_id: "sess-1".to_string(),
            model: "Qwen3.8-27B".to_string(),
        },
        SessionEvent::UserMessage {
            turn: 1,
            step: 1,
            message: AgentMessage::user("列出 workspace 变更"),
        },
        assistant(
            "已检查 workspace。",
            Usage {
                input_tokens: 100,
                output_tokens: 20,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
            },
        ),
        SessionEvent::ToolCall {
            turn: 1,
            step: 1,
            call: onion::types::ToolCall {
                id: "call-1".to_string(),
                name: "workspace_changed".to_string(),
                input: serde_json::json!({}),
            },
        },
        SessionEvent::ToolResult {
            turn: 1,
            step: 1,
            result: onion::types::ToolResult::text(
                &onion::types::ToolCall {
                    id: "call-1".to_string(),
                    name: "workspace_changed".to_string(),
                    input: serde_json::json!({}),
                },
                "+2 files",
            ),
        },
        SessionEvent::TurnEnd {
            turn: 1,
            reason: "end_turn".to_string(),
        },
    ];

    let mut t = crate::transcript::Transcript::new();
    t.replay(&events);
    // 本测试断言工具输出渲染路径，用展开视图（默认折叠不渲染输出，§7.3）。
    let expanded = crate::view_state::ViewState { show_thinking: true, expand_tools: true };
    let frame = crate::components::render_nodes(t.nodes(), expanded, false);

    let rendered: Vec<String> = frame.iter().map(|l| l.text.clone()).collect();
    assert!(rendered.iter().any(|l| l.contains("sess-1")), "会话头缺 session id");
    assert!(rendered.iter().any(|l| l.contains("Qwen3.8-27B")), "会话头缺 model");
    assert!(rendered.iter().any(|l| l.contains("列出 workspace 变更")), "缺用户消息");
    assert!(rendered.iter().any(|l| l.contains("已检查 workspace。")), "缺 assistant 文本");
    assert!(rendered.iter().any(|l| l.contains("in 100 out 20")), "缺 token 总计行");
    assert!(rendered.iter().any(|l| l.contains("workspace_changed")), "缺工具卡片标题");
    assert!(rendered.iter().any(|l| l.contains("+2 files")), "缺工具输出");
    // turn 分隔渲染为圆点标题（对齐 opencode 分组行）。
    assert!(rendered.iter().any(|l| l.contains("● turn 1 (end_turn)")), "缺 turn 分隔行");
}

#[test]
fn compaction_card_frame() {
    let events = vec![
        SessionEvent::CompactionStart {
            turn: 2,
            compaction_id: "c1".to_string(),
            trigger: "auto".to_string(),
        },
        SessionEvent::CompactionSummary {
            turn: 2,
            compaction_id: "c1".to_string(),
            summary: "此前对话已压缩".to_string(),
            shadowed_count: 5,
            shadowed_tokens: 800,
            usage: Usage {
                input_tokens: 0,
                output_tokens: 0,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
            },
        },
    ];

    let mut t = crate::transcript::Transcript::new();
    t.replay(&events);
    let frame = crate::components::render_nodes(t.nodes(), crate::view_state::ViewState::default(), false);
    let rendered: Vec<String> = frame.iter().map(|l| l.text.clone()).collect();

    assert!(rendered.iter().any(|l| l.contains("auto")), "缺 trigger");
    assert!(rendered.iter().any(|l| l.contains("此前对话已压缩")), "缺摘要");
    assert!(rendered.iter().any(|l| l.contains("800")), "缺节省 tokens");
}

#[test]
fn streaming_pending_then_settled_tool_card() {
    // append-origin：ToolCall 先建 pending 卡，ToolResult 落定该卡。
    let call = SessionEvent::ToolCall {
        turn: 1,
        step: 1,
        call: onion::types::ToolCall {
            id: "c1".to_string(),
            name: "read_file".to_string(),
            input: serde_json::json!({ "path": "a.rs" }),
        },
    };
    let result = SessionEvent::ToolResult {
        turn: 1,
        step: 1,
        result: onion::types::ToolResult::text(
            &onion::types::ToolCall {
                id: "c1".to_string(),
                name: "read_file".to_string(),
                input: serde_json::json!({ "path": "a.rs" }),
            },
            "fn main() {}",
        ),
    };

    let mut t = crate::transcript::Transcript::new();
    t.apply(&call);
    let pending: Vec<String> = crate::components::render_nodes(t.nodes(), crate::view_state::ViewState::default(), false)
        .iter()
        .map(|l| l.text.clone())
        .collect();
    assert!(pending.iter().any(|l| l.contains("read_file") && l.contains("path=a.rs")), "pending 卡缺 input 摘要");
    assert!(!pending.iter().any(|l| l.contains("fn main()")), "pending 卡不应含输出");

    t.apply(&result);
    // settled 卡断言输出渲染路径 → 展开视图（默认折叠不渲染输出，§7.3）。
    let expanded = crate::view_state::ViewState { show_thinking: true, expand_tools: true };
    let settled: Vec<String> = crate::components::render_nodes(t.nodes(), expanded, false)
        .iter()
        .map(|l| l.text.clone())
        .collect();
    assert!(settled.iter().any(|l| l.contains("fn main()")), "settled 卡缺输出");
}

#[test]
fn default_view_collapses_tool_output_and_thinking() {
    // §7.3 默认态：推理隐藏、工具输出折叠——投影仍在（Ctrl+R/Ctrl+E 可切回），
    // durable 不动。标题行与正文（assistant 文本）不受影响。
    let events = vec![
        SessionEvent::AssistantMessage {
            turn: 1,
            step: 1,
            message: AgentMessage::assistant(vec![
                ContentBlock::Thinking { thinking: "inner monologue".to_string() },
                ContentBlock::Text { text: "visible answer".to_string() },
            ]),
            usage: Usage { input_tokens: 1, output_tokens: 1, cache_read_tokens: 0, cache_write_tokens: 0 },
        },
        SessionEvent::ToolCall {
            turn: 1,
            step: 1,
            call: onion::types::ToolCall {
                id: "c1".to_string(),
                name: "bash".to_string(),
                input: serde_json::json!({ "command": "ls" }),
            },
        },
        SessionEvent::ToolResult {
            turn: 1,
            step: 1,
            result: onion::types::ToolResult::text(
                &onion::types::ToolCall { id: "c1".to_string(), name: "bash".to_string(), input: serde_json::json!({}) },
                "secret-output",
            ),
        },
    ];
    let mut t = crate::transcript::Transcript::new();
    t.replay(&events);

    let default_frame: Vec<String> =
        crate::components::render_nodes(t.nodes(), crate::view_state::ViewState::default(), false)
            .iter()
            .map(|l| l.text.clone())
            .collect();
    assert!(!default_frame.iter().any(|l| l.contains("inner monologue")), "默认应隐藏推理");
    assert!(!default_frame.iter().any(|l| l.contains("secret-output")), "默认应折叠工具输出");
    assert!(default_frame.iter().any(|l| l.contains("visible answer")), "正文不受折叠影响");
    assert!(default_frame.iter().any(|l| l.contains("bash")), "卡片标题不受折叠影响");

    let expanded = crate::view_state::ViewState { show_thinking: true, expand_tools: true };
    let expanded_frame: Vec<String> =
        crate::components::render_nodes(t.nodes(), expanded, false)
            .iter()
            .map(|l| l.text.clone())
            .collect();
    assert!(expanded_frame.iter().any(|l| l.contains("inner monologue")), "展开后应显示推理");
    assert!(expanded_frame.iter().any(|l| l.contains("secret-output")), "展开后应显示工具输出");
}

#[test]
fn replay_equals_incremental_application() {
    // 两条路径同一份确定性 fold：全量重放帧 == 逐条 apply 帧。
    let events = vec![
        SessionEvent::SessionStarted {
            session_id: "s".to_string(),
            model: "m".to_string(),
        },
        SessionEvent::UserMessage {
            turn: 1,
            step: 1,
            message: AgentMessage::user("hi"),
        },
        SessionEvent::TurnEnd {
            turn: 1,
            reason: "end_turn".to_string(),
        },
    ];

    let mut replayed = crate::transcript::Transcript::new();
    replayed.replay(&events);

    let mut incremental = crate::transcript::Transcript::new();
    for e in &events {
        incremental.apply(e);
    }

    assert_eq!(replayed.nodes(), incremental.nodes());
}
