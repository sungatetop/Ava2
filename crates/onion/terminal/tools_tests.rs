//! `tools` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use crate::tools::ToolPort;
use crate::approval::ToolRisk;
use crate::terminal::seam::TerminalService;
use crate::terminal::tools::{
    TerminalCloseTool, TerminalListTool, TerminalOpenTool, TerminalReadTool, TerminalSendTool,
    TerminalSignalTool, TerminalToolSet,
};

#[test]
fn six_tools_registered_with_expected_names() {
    let service = std::sync::Arc::new(TerminalService::new());
    let set = TerminalToolSet::new(service);
    let tools: Vec<Box<dyn ToolPort>> = vec![
        Box::new(TerminalOpenTool::new(set.clone())),
        Box::new(TerminalSendTool::new(set.clone())),
        Box::new(TerminalReadTool::new(set.clone())),
        Box::new(TerminalSignalTool::new(set.clone())),
        Box::new(TerminalCloseTool::new(set.clone())),
        Box::new(TerminalListTool::new(set)),
    ];
    let names: Vec<String> = tools.iter().map(|t| t.definition().name).collect();
    assert_eq!(
        names,
        vec![
            "terminal_open",
            "terminal_send",
            "terminal_read",
            "terminal_signal",
            "terminal_close",
            "terminal_list",
        ]
    );
}

#[test]
fn write_tools_are_escalating_and_work_mode() {
    let service = std::sync::Arc::new(TerminalService::new());
    let set = TerminalToolSet::new(service);
    let open = TerminalOpenTool::new(set.clone());
    let send = TerminalSendTool::new(set.clone());
    let signal = TerminalSignalTool::new(set.clone());
    let close = TerminalCloseTool::new(set);
    assert_eq!(open.risk(), ToolRisk::Escalating);
    assert_eq!(send.risk(), ToolRisk::Escalating);
    assert_eq!(signal.risk(), ToolRisk::Escalating);
    assert_eq!(close.risk(), ToolRisk::Escalating);
    assert_eq!(open.modes(), Some(vec!["work".to_string()]));
}

#[test]
fn read_tools_are_readonly() {
    let service = std::sync::Arc::new(TerminalService::new());
    let set = TerminalToolSet::new(service);
    let read = TerminalReadTool::new(set.clone());
    let list = TerminalListTool::new(set);
    assert_eq!(read.risk(), ToolRisk::ReadOnly);
    assert_eq!(list.risk(), ToolRisk::ReadOnly);
}

#[test]
fn send_schema_has_signal_enum_whitelist() {
    let service = std::sync::Arc::new(TerminalService::new());
    let set = TerminalToolSet::new(service);
    let signal = TerminalSignalTool::new(set);
    let def = signal.definition();
    let props = def.parameters["properties"].as_object().unwrap();
    let sig_enum = props["signal"]["enum"].as_array().unwrap();    let values: Vec<&str> = sig_enum.iter().map(|v| v.as_str().unwrap()).collect();
    assert_eq!(values, vec!["SIGINT", "SIGTERM", "SIGKILL", "SIGTSTP", "SIGHUP"]);
}

#[test]
fn open_schema_requires_type() {
    let service = std::sync::Arc::new(TerminalService::new());
    let set = TerminalToolSet::new(service);
    let open = TerminalOpenTool::new(set);
    let def = open.definition();
    let required = def.parameters["required"].as_array().unwrap();
    assert!(required.iter().any(|v| v.as_str() == Some("type")));
}

#[test]
fn background_send_returns_job_id_when_wired() {
    let service = std::sync::Arc::new(TerminalService::new());
    let registry = std::sync::Arc::new(onion_jobs::JobRegistry::new());
    let set = TerminalToolSet::new(service).with_background_enabled(true);
    set.wire_background(registry);
    set.wire_owner("sess-1");
    let send = TerminalSendTool::new(set);
    let call = crate::types::ToolCall {
        id: "call-1".to_string(),
        name: "terminal_send".to_string(),
        input: serde_json::json!({
            "sessionId": "pty-1",
            "text": "echo hi",
            "run_in_background": true
        }),
    };
    // owner 已接线、registry 已接线：spawn 成功返回 job id（PTY 会话未打开，
    // 后台任务在 spawn_blocking 中因 no session 落地 Failed，但不影响返回）。
    let result = send.execute(call, tokio_util::sync::CancellationToken::new());
    let blocks = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(result)
        .expect("background send should succeed");
    assert_eq!(blocks.len(), 1);
    let text = match &blocks[0] {
        crate::types::ContentBlock::Text { text } => text,
        _ => panic!("expected text block"),
    };
    assert!(text.starts_with("started background job "), "got: {text}");
}

#[test]
fn background_send_disabled_rejected() {
    let service = std::sync::Arc::new(TerminalService::new());
    let set = TerminalToolSet::new(service).with_background_enabled(false);
    set.wire_owner("sess-1");
    let send = TerminalSendTool::new(set);
    let call = crate::types::ToolCall {
        id: "call-1".to_string(),
        name: "terminal_send".to_string(),
        input: serde_json::json!({
            "sessionId": "pty-1",
            "text": "echo hi",
            "run_in_background": true
        }),
    };
    let result = send.execute(call, tokio_util::sync::CancellationToken::new());
    let err = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(result)
        .expect_err("should be rejected");
    assert!(err.contains("disabled"), "got: {err}");
}
