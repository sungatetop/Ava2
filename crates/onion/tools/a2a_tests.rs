//! `a2a` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//!
//! 用例覆盖：定义 schema、风险分级、输入校验（url / message / contextId）、
//! GET/POST 线格式构造、委派链接落盘（`tasks/links.jsonl`）与 owner 未接线
//! fail-closed。网络路径走本地回环 TCP 桩，不打真实对端。

use std::path::PathBuf;

use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::ToolPort;
use super::{A2aApproveTool, A2aCallTool, A2aSendTool, build_provide_input_rpc, build_send_rpc, read_links};
use crate::approval::ToolRisk;
use crate::types::{ContentBlock, ToolCall};

/// 会话落盘容器临时目录（测试结束清理）。
fn temp_sessions_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("onion_a2a_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp sessions dir");
    dir
}

/// 构造已接线 owner 的 `a2a_send`（instanceId 固定 `inst-a`，显示名 `Alice`，会话固定 `sess-1`）。
fn send_tool(token: Option<String>, sessions_dir: PathBuf) -> A2aSendTool {
    let tool = A2aSendTool::new(token, sessions_dir, "inst-a".to_string(), "Alice".to_string());
    tool.wire_owner("sess-1");
    tool
}

/// 定义与风险。
#[test]
fn call_definition_and_risk() {
    let tool = A2aCallTool::new();
    let definition = tool.definition();
    assert_eq!(definition.name, "a2a_call");
    assert_eq!(tool.risk(), ToolRisk::ReadOnly);
    assert_eq!(tool.classify_risk(&tool_call("{}", json!({}))), ToolRisk::ReadOnly);
}

/// 定义与风险。
#[test]
fn send_definition_and_risk() {
    let dir = temp_sessions_dir("definition");
    let tool = send_tool(/*token*/ None, dir.clone());
    let definition = tool.definition();
    assert_eq!(definition.name, "a2a_send");
    assert_eq!(tool.risk(), ToolRisk::WorkspaceWrite);
    assert_eq!(tool.classify_risk(&tool_call("{}", json!({}))), ToolRisk::WorkspaceWrite);
    let _ = std::fs::remove_dir_all(&dir);
}

/// `a2a_call` 输入校验：空 url / 非 http(s)。
#[tokio::test]
async fn call_rejects_bad_url() {
    let tool = A2aCallTool::new();
    let cancel = CancellationToken::new();
    for bad in ["", "   ", "ftp://x", "not-a-url"] {
        let result = tool
            .execute(tool_call("c1", json!({ "url": bad })), cancel.clone())
            .await;
        assert!(result.is_err(), "expected error for {bad:?}");
    }
}

/// `a2a_send` 输入校验：空 url / 空 message。
#[tokio::test]
async fn send_rejects_bad_input() {
    let dir = temp_sessions_dir("bad_input");
    let tool = send_tool(/*token*/ None, dir.clone());
    let cancel = CancellationToken::new();
    for input in [
        json!({ "url": "", "message": "hi" }),
        json!({ "url": "http://127.0.0.1:1", "message": "" }),
        json!({ "url": "http://127.0.0.1:1" }),
    ] {
        let result = tool.execute(tool_call("c1", input.clone()), cancel.clone()).await;
        assert!(result.is_err(), "expected error for {input:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// owner 未接线：fail-closed（不发请求，直接报错）。
#[tokio::test]
async fn send_fails_closed_without_owner() {
    let dir = temp_sessions_dir("no_owner");
    let tool = A2aSendTool::new(/*token*/ None, dir.clone(), "inst-a".to_string(), "Alice".to_string());
    let result = tool
        .execute(
            tool_call("c1", json!({ "url": "http://127.0.0.1:1", "message": "hi" })),
            CancellationToken::new(),
        )
        .await;
    assert!(result.is_err(), "expected fail-closed without owner");
    let _ = std::fs::remove_dir_all(&dir);
}

/// `a2a_call` 真实请求打到本进程不可达端口：错误合成（只验证 reqwest 错误路径）。
#[tokio::test]
async fn call_unreachable_url_yields_error() {
    let tool = A2aCallTool::new();
    let cancel = CancellationToken::new();
    let result = tool
        .execute(tool_call("c1", json!({ "url": "http://127.0.0.1:1" })), cancel)
        .await;
    assert!(result.is_err());
}

/// `message/send` 线格式：message 必须嵌在 `params.message` 下（A2A `MessageSendParams`）。
#[test]
fn send_rpc_embeds_message_under_params() {
    let rpc = build_send_rpc("m-1", "hello", /*context_id*/ None, /*report_to*/ None, /*sender*/ None);
    assert_eq!(rpc["method"], json!("message/send"));
    assert_eq!(rpc["params"]["message"]["messageId"], json!("m-1"));
    assert_eq!(rpc["params"]["message"]["role"], json!("user"));
    assert_eq!(rpc["params"]["message"]["kind"], json!("message"));
    assert_eq!(rpc["params"]["message"]["parts"][0]["text"], json!("hello"));
    assert!(rpc["params"]["message"].get("contextId").is_none());
    assert!(rpc["params"]["message"].get("metadata").is_none());
    // 回归防线：message 本体不得直接铺在 params 上。
    assert!(rpc["params"].get("messageId").is_none());
}

/// `message/send` 线格式：显式 contextId 落在 message 内。
#[test]
fn send_rpc_carries_context_id() {
    let rpc = build_send_rpc("m-2", "hello", Some("ctx-9"), /*report_to*/ None, /*sender*/ None);
    assert_eq!(rpc["params"]["message"]["contextId"], json!("ctx-9"));
}

/// `message/send` 线格式：`reportTo` 落在 message.metadata 下。
#[test]
fn send_rpc_carries_report_to() {
    let report_to = json!({ "instanceId": "inst-a", "sessionId": "sess-1" });
    let rpc = build_send_rpc("m-3", "hello", /*context_id*/ None, Some(&report_to), /*sender*/ None);
    assert_eq!(rpc["params"]["message"]["metadata"]["reportTo"], report_to);
}

/// `message/send` 线格式：`sender` 落在 message.metadata 下，且与 `reportTo` 共存。
#[test]
fn send_rpc_carries_sender() {
    let report_to = json!({ "instanceId": "inst-a", "sessionId": "sess-1" });
    let sender = json!({ "instanceId": "inst-a", "name": "Alice", "sessionId": "sess-1" });
    let rpc = build_send_rpc("m-4", "hello", /*context_id*/ None, Some(&report_to), Some(&sender));
    assert_eq!(rpc["params"]["message"]["metadata"]["sender"], sender);
    assert_eq!(rpc["params"]["message"]["metadata"]["reportTo"], report_to);
}

/// `a2a_send` 取消：cancel 后 execute 立即失败。
#[tokio::test]
async fn send_cancelled_yields_error() {
    let dir = temp_sessions_dir("cancelled");
    let tool = send_tool(/*token*/ None, dir.clone());
    let cancel = CancellationToken::new();
    cancel.cancel();
    let result = tool
        .execute(
            tool_call("c1", json!({ "url": "http://127.0.0.1:9801", "message": "hi" })),
            cancel,
        )
        .await;
    assert!(result.is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

/// `a2a_call` 取消：cancel 后 execute 立即失败。
#[tokio::test]
async fn call_cancelled_yields_error() {
    let tool = A2aCallTool::new();
    let cancel = CancellationToken::new();
    cancel.cancel();
    let result = tool
        .execute(tool_call("c1", json!({ "url": "http://127.0.0.1:9801" })), cancel)
        .await;
    assert!(result.is_err());
}

/// 本地回环 HTTP 桩：读满请求头、回 200 + 成功 Task JSON，返回原始请求文本与工具结果。
async fn capture_request(
    token: Option<String>,
    sessions_dir: PathBuf,
) -> (String, Result<Vec<ContentBlock>, String>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept");
        let mut raw = Vec::new();
        let mut buf = [0u8; 1024];
        loop {
            let read = socket.read(&mut buf).await.expect("read");
            if read == 0 {
                break;
            }
            raw.extend_from_slice(&buf[..read]);
            if raw.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        let body = r#"{"jsonrpc":"2.0","id":"m-1","result":{"id":"task-1","contextId":"ctx-1","status":{"state":"completed"}}}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        let _ = socket.write_all(response.as_bytes()).await;
        String::from_utf8_lossy(&raw).into_owned()
    });
    let tool = send_tool(token, sessions_dir);
    let result = tool
        .execute(
            tool_call("c1", json!({ "url": format!("http://{addr}"), "message": "hi" })),
            CancellationToken::new(),
        )
        .await;
    (server.await.expect("server"), result)
}

/// mesh 令牌：配置后 `a2a_send` 随请求带 `X-Onion-Token`。
#[tokio::test]
async fn send_attaches_mesh_token_header() {
    let dir = temp_sessions_dir("mesh_token");
    let (raw, result) = capture_request(Some("mesh-1".to_string()), dir.clone()).await;
    assert!(raw.to_ascii_lowercase().contains("x-onion-token: mesh-1"), "headers: {raw}");
    assert!(result.is_ok(), "expected ok");
    let _ = std::fs::remove_dir_all(&dir);
}

/// 未配置 mesh 令牌：不带任何鉴权头（匿名 `/a2a` 面）。
#[tokio::test]
async fn send_omits_token_header_when_unset() {
    let dir = temp_sessions_dir("no_token");
    let (raw, result) = capture_request(None, dir.clone()).await;
    assert!(!raw.to_ascii_lowercase().contains("x-onion-token"), "headers: {raw}");
    assert!(result.is_ok(), "expected ok");
    let _ = std::fs::remove_dir_all(&dir);
}

/// 成功发送后追加一行委派链接（含 reportTo 自动填充）。
#[tokio::test]
async fn send_appends_delegation_link() {
    let dir = temp_sessions_dir("links");
    let (_raw, result) = capture_request(None, dir.clone()).await;
    assert!(result.is_ok(), "expected ok");

    let links = read_links(&dir, "sess-1");
    assert_eq!(links.len(), 1, "expected one delegation link");
    let link = &links[0];
    assert_eq!(link.remote_task_id, "task-1");
    assert_eq!(link.remote_context_id, "ctx-1");
    assert_eq!(link.remote_state, "completed");
    assert!(link.peer_url.starts_with("http://127.0.0.1:"));
    assert_eq!(
        link.report_to,
        Some(json!({ "instanceId": "inst-a", "sessionId": "sess-1" }))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// 构造 ToolCall。
fn tool_call(name: &str, input: serde_json::Value) -> ToolCall {
    ToolCall {
        id: format!("call_{name}"),
        name: name.to_string(),
        input,
    }
}

/// `a2a_approve` 定义与风险。
#[test]
fn approve_definition_and_risk() {
    let tool = A2aApproveTool::new(/*token*/ None);
    let definition = tool.definition();
    assert_eq!(definition.name, "a2a_approve");
    assert_eq!(tool.risk(), ToolRisk::WorkspaceWrite);
    // 回填需等对端轮次收敛 → 阻断型，不参与统一 tool_timeout。
    assert!(tool.blocking());
    assert_eq!(
        tool.classify_risk(&tool_call("{}", json!({}))),
        ToolRisk::WorkspaceWrite
    );
}

/// `tasks/provideInput` 线格式：plan-review 裁决。
#[test]
fn provide_input_rpc_carries_decision() {
    let rpc = build_provide_input_rpc("task-1", "req-1", Some("approve"), /*answers*/ None);
    assert_eq!(rpc["method"], json!("tasks/provideInput"));
    assert_eq!(rpc["id"], json!("req-1"));
    assert_eq!(rpc["params"]["taskId"], json!("task-1"));
    assert_eq!(rpc["params"]["requestId"], json!("req-1"));
    assert_eq!(rpc["params"]["decision"], json!("approve"));
    assert!(rpc["params"].get("answers").is_none());
}

/// `tasks/provideInput` 线格式：ask-user 答复。
#[test]
fn provide_input_rpc_carries_answers() {
    let answers = json!([{ "id": "q1", "selected": ["a"], "custom": null }]);
    let rpc = build_provide_input_rpc("task-2", "req-2", /*decision*/ None, Some(&answers));
    assert_eq!(rpc["params"]["answers"], answers);
    assert!(rpc["params"].get("decision").is_none());
}

/// `a2a_approve` 输入校验：缺 url / taskId / requestId，或裁决与答复双缺。
#[tokio::test]
async fn approve_rejects_bad_input() {
    let tool = A2aApproveTool::new(/*token*/ None);
    let cancel = CancellationToken::new();
    for input in [
        json!({ "taskId": "t1", "requestId": "r1", "decision": "approve" }),
        json!({ "url": "http://127.0.0.1:1", "requestId": "r1", "decision": "approve" }),
        json!({ "url": "http://127.0.0.1:1", "taskId": "t1", "decision": "approve" }),
        json!({ "url": "http://127.0.0.1:1", "taskId": "t1", "requestId": "r1" }),
    ] {
        let result = tool
            .execute(tool_call("c1", input.clone()), cancel.clone())
            .await;
        assert!(result.is_err(), "expected error for {input:?}");
    }
}

/// `a2a_approve` 取消：cancel 后 execute 立即失败。
#[tokio::test]
async fn approve_cancelled_yields_error() {
    let tool = A2aApproveTool::new(/*token*/ None);
    let cancel = CancellationToken::new();
    cancel.cancel();
    let result = tool
        .execute(
            tool_call(
                "c1",
                json!({ "url": "http://127.0.0.1:9801", "taskId": "t1", "requestId": "r1", "decision": "deny" }),
            ),
            cancel,
        )
        .await;
    assert!(result.is_err());
}
