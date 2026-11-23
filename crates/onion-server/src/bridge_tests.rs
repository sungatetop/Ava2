//! `bridge` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//!
//! 用例覆盖：`reportTo` → 对端 URL 路由（命中 / peers 表未命中 / 缺字段）、
//! 确认请求 `message/send` 线格式（contextId 指向裁决会话、`confirmRequest`
//! 元数据与正文回填指引）。仅纯函数，不发真实请求。

use serde_json::json;

use super::{Bridge, Route, build_confirm_rpc};
use crate::rest::dto::PeerInfo;
use crate::session::PendingKind;

/// 双实例 peers 表（A :9800 / B :9801，A 的 url 带尾斜杠验证归一化）。
fn bridge() -> Bridge {
    Bridge::new(
        vec![
            PeerInfo {
                name: "inst-a".to_string(),
                url: "http://127.0.0.1:9800/".to_string(),
            },
            PeerInfo {
                name: "inst-b".to_string(),
                url: "http://127.0.0.1:9801".to_string(),
            },
        ],
        "http://127.0.0.1:9801".to_string(),
        Some("mesh-1".to_string()),
    )
}

/// `reportTo` 命中 peers 表：URL 去尾斜杠 + 携带裁决会话 id。
#[test]
fn route_resolves_peer_url() {
    let bridge = bridge();
    let report_to = json!({ "instanceId": "inst-a", "sessionId": "sess-a" });
    assert_eq!(
        bridge.route(Some(&report_to)),
        Some(Route {
            peer_url: "http://127.0.0.1:9800".to_string(),
            session_id: "sess-a".to_string(),
        })
    );
}

/// 对端不在 peers 表 / 缺字段 / 无 `reportTo`：一律不推送（无兜底）。
#[test]
fn route_requires_known_peer() {
    let bridge = bridge();
    for report_to in [
        json!({ "instanceId": "inst-x", "sessionId": "sess-a" }),
        json!({ "instanceId": "inst-a" }),
        json!({ "sessionId": "sess-a" }),
        json!({}),
    ] {
        assert_eq!(bridge.route(Some(&report_to)), None, "{report_to:?}");
    }
    assert_eq!(bridge.route(/*report_to*/ None), None);
}

/// 确认请求线格式：plan-review 指向裁决会话并携带回填元数据。
#[test]
fn confirm_rpc_wire_format_plan_review() {
    let route = Route {
        peer_url: "http://127.0.0.1:9800".to_string(),
        session_id: "sess-a".to_string(),
    };
    let kind = PendingKind::PlanReview {
        plan: "# Plan\nstep 1".to_string(),
    };
    let rpc = build_confirm_rpc(&route, "http://127.0.0.1:9801", "task-9", "req-9", &kind);
    assert_eq!(rpc["method"], json!("message/send"));
    assert_eq!(rpc["id"], json!("req-9"));
    assert_eq!(rpc["params"]["message"]["role"], json!("user"));
    assert_eq!(rpc["params"]["message"]["contextId"], json!("sess-a"));
    let meta = &rpc["params"]["message"]["metadata"]["confirmRequest"];
    assert_eq!(meta["requestId"], json!("req-9"));
    assert_eq!(meta["item"], json!("plan_review"));
    assert_eq!(meta["taskId"], json!("task-9"));
    assert_eq!(meta["peerUrl"], json!("http://127.0.0.1:9801"));
    assert_eq!(meta["payload"]["plan"], json!("# Plan\nstep 1"));
    let text = rpc["params"]["message"]["parts"][0]["text"]
        .as_str()
        .expect("text part");
    assert!(text.contains("req-9"), "text: {text}");
    assert!(text.contains("a2a_approve"), "text: {text}");
}

/// 确认请求线格式：ask-user 携带问题载荷（`item = question`）。
#[test]
fn confirm_rpc_wire_format_question() {
    let route = Route {
        peer_url: "http://127.0.0.1:9800".to_string(),
        session_id: "sess-a".to_string(),
    };
    let kind = PendingKind::Question {
        call_id: "call-7".to_string(),
        questions: Vec::new(),
    };
    let rpc = build_confirm_rpc(&route, "http://127.0.0.1:9801", "task-10", "req-10", &kind);
    let meta = &rpc["params"]["message"]["metadata"]["confirmRequest"];
    assert_eq!(meta["item"], json!("question"));
    assert_eq!(meta["payload"]["callId"], json!("call-7"));
    let text = rpc["params"]["message"]["parts"][0]["text"]
        .as_str()
        .expect("text part");
    assert!(text.contains("callId=call-7"), "text: {text}");
    assert!(text.contains("answers"), "text: {text}");
}
