//! `/v1` RESTful API 端到端冒烟：启动真实 `onion-server` 子进程 + 真实 HTTP 调用。
//!
//! 覆盖：鉴权（公开探活 vs 受保护 REST）、运维与元信息、会话生命周期、
//! 能力枚举、记忆资料库、错误模型、SSE 首帧快照。会话消息轮次需要真实 LLM，
//! 不在此文件内（由人工冒烟脚本覆盖）。

// 集成测试的装配辅助函数不在 `#[test]` 体内，`allow-expect-in-tests` 不覆盖，
// 这里按测试文件粒度放行（失败即断言失败）。
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use reqwest::StatusCode;
use serde_json::{json, Value};

/// 测试用访问令牌。
const TOKEN: &str = "test-onion-token";

/// 子进程守卫：退出时 kill 并清理临时数据目录。
struct Server {
    child: Child,
    base: String,
    root: std::path::PathBuf,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// 选一个空闲端口（bind 后立即释放）。
fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind ephemeral port");
    listener.local_addr().expect("local addr").port()
}

/// 启动服务并等待 `/healthz` 就绪。
///
/// `label` 隔离各测试的临时数据根：同一测试进程内多个 `#[tokio::test]`
/// 并发运行，若共享 `ONION_HOME`，先结束测试的 `Server::drop` 会
/// `remove_dir_all` 掉仍在运行一方刚落盘的会话日志，导致其后的
/// `require_session` 冷恢复 `locate` 失败而误报 404。
async fn spawn_server(label: &str) -> Server {
    let root = std::env::temp_dir().join(format!("onion_rest_api_{label}_{}", std::process::id()));
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).expect("create temp workspace");
    let port = free_port();
    let child = Command::new(env!("CARGO_BIN_EXE_onion-server"))
        .arg("--port")
        .arg(port.to_string())
        .env("ONION_HOME", &root)
        .env("ONION_SERVER_WORKSPACE", &workspace)
        .env("ONION_SERVER_TOKEN", TOKEN)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn onion-server");
    let base = format!("http://127.0.0.1:{port}");
    let client = reqwest::Client::new();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Ok(response) = client.get(format!("{base}/healthz")).send().await {
            if response.status() == StatusCode::OK {
                break;
            }
        }
        assert!(Instant::now() < deadline, "onion-server 未在 30s 内就绪");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Server { child, base, root }
}

/// 带令牌的请求构造。
fn authed(client: &reqwest::Client, server: &Server, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
    client
        .request(method, format!("{}{path}", server.base))
        .header("X-Onion-Token", TOKEN)
}

/// 读取 JSON body 的 `error.code`。
fn error_code(body: &Value) -> &str {
    body.get("error")
        .and_then(|error| error.get("code"))
        .and_then(Value::as_str)
        .unwrap_or_default()
}

#[tokio::test]
async fn rest_api_smoke() {
    let server = spawn_server("rest").await;
    let client = reqwest::Client::new();

    // ---- 鉴权：公开探活免令牌，REST 受保护。----
    let healthz = client
        .get(format!("{}/healthz", server.base))
        .send()
        .await
        .expect("healthz");
    assert_eq!(healthz.status(), StatusCode::OK);

    let denied = client
        .get(format!("{}/v1/health", server.base))
        .send()
        .await
        .expect("unauthenticated health");
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(&denied.json().await.expect("error body")), "unauthorized");

    let bearer = client
        .get(format!("{}/v1/health", server.base))
        .header("Authorization", format!("Bearer {TOKEN}"))
        .send()
        .await
        .expect("bearer health");
    assert_eq!(bearer.status(), StatusCode::OK);

    // ---- 运维与元信息。----
    let health: Value = authed(&client, &server, reqwest::Method::GET, "/v1/health")
        .send()
        .await
        .expect("v1 health")
        .json()
        .await
        .expect("health json");
    assert_eq!(health["status"], "ok");
    assert!(!health["model"].as_str().unwrap_or_default().is_empty());
    assert!(health["uptimeMs"].as_u64().is_some());

    let meta: Value = authed(&client, &server, reqwest::Method::GET, "/v1/meta")
        .send()
        .await
        .expect("meta")
        .json()
        .await
        .expect("meta json");
    assert_eq!(meta["protocol"], "onion-rest/v1");
    assert_eq!(meta["interactiveDefault"], true);
    assert!(!meta["features"].as_array().unwrap_or(&Vec::new()).is_empty());

    let config: Value = authed(&client, &server, reqwest::Method::GET, "/v1/config")
        .send()
        .await
        .expect("config")
        .json()
        .await
        .expect("config json");
    assert!(config.get("apiKey").is_none(), "config 必须脱敏");

    let models: Value = authed(&client, &server, reqwest::Method::GET, "/v1/models")
        .send()
        .await
        .expect("models")
        .json()
        .await
        .expect("models json");
    let providers = models["providers"].as_array().expect("providers");
    assert!(!providers.is_empty());
    assert!(!models["defaultModel"].as_str().unwrap_or_default().is_empty());
    for provider in providers {
        assert!(provider.get("apiKey").is_none(), "models 必须脱敏");
    }

    let agents: Vec<Value> = authed(&client, &server, reqwest::Method::GET, "/v1/agents")
        .send()
        .await
        .expect("agents")
        .json()
        .await
        .expect("agents json");
    assert!(!agents.is_empty());

    // ---- 会话生命周期。----
    let created = authed(&client, &server, reqwest::Method::POST, "/v1/sessions")
        .json(&json!({ "id": "rest-smoke" }))
        .send()
        .await
        .expect("create session");
    assert_eq!(created.status(), StatusCode::CREATED);
    let session: Value = created.json().await.expect("session json");
    assert_eq!(session["id"], "rest-smoke");
    assert_eq!(session["interactive"], true);
    assert_eq!(session["busy"], false);

    let detail: Value = authed(&client, &server, reqwest::Method::GET, "/v1/sessions/rest-smoke")
        .send()
        .await
        .expect("get session")
        .json()
        .await
        .expect("session detail json");
    assert_eq!(detail["id"], "rest-smoke");
    assert!(detail["plan"]["active"].as_bool().is_some());

    let list: Value = authed(&client, &server, reqwest::Method::GET, "/v1/sessions?live=true")
        .send()
        .await
        .expect("list sessions")
        .json()
        .await
        .expect("list json");
    assert!(list["sessions"]
        .as_array()
        .expect("sessions")
        .iter()
        .any(|item| item["id"] == "rest-smoke" && item["live"] == true));

    // ---- SSE 首帧：连接即下发 snapshot。----
    let mut stream = authed(&client, &server, reqwest::Method::GET, "/v1/sessions/rest-smoke/events")
        .send()
        .await
        .expect("sse connect");
    assert_eq!(stream.status(), StatusCode::OK);
    let first = tokio::time::timeout(Duration::from_secs(10), stream.chunk())
        .await
        .expect("sse first frame timeout")
        .expect("sse chunk")
        .expect("sse bytes");
    let first_text = String::from_utf8_lossy(&first).to_string();
    assert!(
        first_text.contains("event: snapshot") && first_text.contains("\"type\":\"snapshot\""),
        "首帧应为 snapshot：{first_text}"
    );
    drop(stream);

    // ---- 能力枚举（会话级）。----
    let tools: Value = authed(&client, &server, reqwest::Method::GET, "/v1/tools?session=rest-smoke")
        .send()
        .await
        .expect("tools")
        .json()
        .await
        .expect("tools json");
    assert!(!tools["tools"].as_array().expect("tools array").is_empty());
    assert!(tools["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .any(|tool| tool["name"] == "read"));

    let missing_session = authed(&client, &server, reqwest::Method::GET, "/v1/tools")
        .send()
        .await
        .expect("tools without session");
    assert_eq!(missing_session.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        error_code(&missing_session.json().await.expect("error body")),
        "session_required"
    );

    for path in ["/v1/skills?session=rest-smoke", "/v1/mcp/servers?session=rest-smoke", "/v1/schedules?session=rest-smoke"] {
        let response = authed(&client, &server, reqwest::Method::GET, path)
            .send()
            .await
            .expect("capability read");
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        assert_eq!(status, StatusCode::OK, "{path} body={body}");
    }

    // ---- 记忆资料库。----
    let memory: Value = authed(&client, &server, reqwest::Method::GET, "/v1/memory")
        .send()
        .await
        .expect("memory list")
        .json()
        .await
        .expect("memory json");
    assert!(memory["memories"].as_array().is_some());

    let missing_memory = authed(&client, &server, reqwest::Method::GET, "/v1/memory/missing.md")
        .send()
        .await
        .expect("memory get");
    assert_eq!(missing_memory.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        error_code(&missing_memory.json().await.expect("error body")),
        "memory_not_found"
    );

    // ---- 错误模型。----
    let unknown_session = authed(&client, &server, reqwest::Method::GET, "/v1/sessions/nope")
        .send()
        .await
        .expect("unknown session");
    assert_eq!(unknown_session.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        error_code(&unknown_session.json().await.expect("error body")),
        "session_not_found"
    );

    let bad_decision = authed(
        &client,
        &server,
        reqwest::Method::POST,
        "/v1/sessions/rest-smoke/approvals/req-nope",
    )
    .json(&json!({ "decision": "bogus" }))
    .send()
    .await
    .expect("bad decision");
    assert_eq!(bad_decision.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        error_code(&bad_decision.json().await.expect("error body")),
        "invalid_decision"
    );

    let unknown_request = authed(
        &client,
        &server,
        reqwest::Method::POST,
        "/v1/sessions/rest-smoke/approvals/req-nope",
    )
    .json(&json!({ "decision": "approve" }))
    .send()
    .await
    .expect("unknown request");
    assert_eq!(unknown_request.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        error_code(&unknown_request.json().await.expect("error body")),
        "request_not_found"
    );

    // ---- 审批回执三值：`approve_for_session` 为合法 decision（未知 request 仍 404）。----
    let session_decision = authed(
        &client,
        &server,
        reqwest::Method::POST,
        "/v1/sessions/rest-smoke/approvals/req-nope",
    )
    .json(&json!({ "decision": "approve_for_session" }))
    .send()
    .await
    .expect("approve_for_session");
    assert_eq!(session_decision.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        error_code(&session_decision.json().await.expect("error body")),
        "request_not_found"
    );

    // ---- 删除会话。----
    let deleted = authed(&client, &server, reqwest::Method::DELETE, "/v1/sessions/rest-smoke")
        .send()
        .await
        .expect("delete session");
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);

    let gone = authed(&client, &server, reqwest::Method::GET, "/v1/sessions/rest-smoke")
        .send()
        .await
        .expect("session after delete");
    assert_eq!(gone.status(), StatusCode::NOT_FOUND);
}

/// WS 订阅端到端：建会话 → WS 订阅 → DELETE 触发 session_closed → Close 帧。
/// 握手鉴权与 REST 同源（X-Onion-Token / Bearer），无法在 `tokio-tungstenite`
/// 客户端便捷地注入自定义头并断言 401（拒绝发生在 HTTP upgrade 阶段），
/// 鉴权矩阵由 SSE 端点在 rest_api_smoke 中覆盖——两条路由挂在同一个
/// `require_token` 中间件之后。
#[tokio::test]
async fn ws_session_events_smoke() {
    let server = spawn_server("ws").await;
    let client = reqwest::Client::new();

    let created = authed(&client, &server, reqwest::Method::POST, "/v1/sessions")
        .json(&json!({ "id": "ws-smoke" }))
        .send()
        .await
        .expect("create session");
    assert_eq!(created.status(), StatusCode::CREATED);

    // ---- WS 握手：令牌经自定义头注入（与 REST 同一 `require_token` 中间件）。----
    let ws_url = format!("{}/v1/sessions/ws-smoke/ws", server.base.replace("http", "ws"));
    let mut request = tokio_tungstenite::tungstenite::client::IntoClientRequest::into_client_request(
        ws_url.as_str(),
    )
    .expect("ws request");
    request.headers_mut().insert(
        "X-Onion-Token",
        TOKEN.parse().expect("header value"),
    );
    let (mut ws, _) = tokio_tungstenite::connect_async(request)
        .await
        .expect("ws handshake");

    // ---- 首帧：snapshot（与 SSE 对齐）。----
    let first = tokio::time::timeout(Duration::from_secs(10), ws.next())
        .await
        .expect("ws first frame timeout")
        .expect("ws stream ok")
        .expect("ws frame");
    let first_text = match first {
        tokio_tungstenite::tungstenite::Message::Text(text) => text.to_string(),
        other => panic!("首帧应为 Text(snapshot)，实际 {other:?}"),
    };
    assert!(
        first_text.contains("\"type\":\"snapshot\""),
        "首帧应为 snapshot：{first_text}"
    );

    // ---- DELETE 会话 → 收到 session_closed 帧 → Close 帧断开。----
    let deleted = authed(&client, &server, reqwest::Method::DELETE, "/v1/sessions/ws-smoke")
        .send()
        .await
        .expect("delete session");
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);

    let closed = tokio::time::timeout(Duration::from_secs(10), ws.next())
        .await
        .expect("ws closed frame timeout")
        .expect("ws stream ok")
        .expect("ws frame");
    assert!(
        matches!(closed, tokio_tungstenite::tungstenite::Message::Text(ref text) if text.contains("\"type\":\"session_closed\"")),
        "应推送 session_closed 帧，实际 {closed:?}"
    );

    // ---- Close 帧随后到达（服务端主动断开）。----
    // `runtime.end()` 会先推 `session_closed`，再推 `StateChanged` live 事件，
    // 最后 broadcast 通道关闭才有 Close——中间的 live 事件逐一跳过。
    let close = loop {
        let frame = tokio::time::timeout(Duration::from_secs(10), ws.next())
            .await
            .expect("ws close frame timeout");
        match frame {
            Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_))) => break Some(()),
            Some(Ok(_)) => continue,
            _ => break None,
        }
    };
    assert!(close.is_some(), "session_closed 后应收到 Close 帧");
}

/// 会话控制面：运行期审批预设 / 模型切换（读面走 SessionInfo 字段）。
#[tokio::test]
async fn rest_session_controls() {
    let server = spawn_server("controls").await;
    let client = reqwest::Client::new();

    let created = authed(&client, &server, reqwest::Method::POST, "/v1/sessions")
        .json(&json!({ "id": "controls-smoke" }))
        .send()
        .await
        .expect("create session");
    assert_eq!(created.status(), StatusCode::CREATED);

    // ---- 读面：SessionInfo 暴露 model / approvalPreset。----
    let detail: Value = authed(&client, &server, reqwest::Method::GET, "/v1/sessions/controls-smoke")
        .send()
        .await
        .expect("get session")
        .json()
        .await
        .expect("session detail json");
    assert!(
        detail["model"].as_str().is_some_and(|model| !model.is_empty()),
        "SessionInfo.model 应为非空模型 id：{detail}"
    );
    assert_eq!(detail["approvalPreset"], "workspace-write");

    // ---- 审批预设切换：合法值写回 + 非法值 400。----
    let switched = authed(&client, &server, reqwest::Method::PUT, "/v1/sessions/controls-smoke/approval")
        .json(&json!({ "preset": "auto" }))
        .send()
        .await
        .expect("set approval");
    assert_eq!(switched.status(), StatusCode::OK);
    let body: Value = switched.json().await.expect("approval json");
    assert_eq!(body["approvalPreset"], "auto");

    let rejected = authed(&client, &server, reqwest::Method::PUT, "/v1/sessions/controls-smoke/approval")
        .json(&json!({ "preset": "custom" }))
        .send()
        .await
        .expect("set approval rejected");
    assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        error_code(&rejected.json().await.expect("error body")),
        "unknown_preset"
    );

    // ---- 模型切换：目录内模型写回 + 目录外 400。----
    let models: Value = authed(&client, &server, reqwest::Method::GET, "/v1/models")
        .send()
        .await
        .expect("models")
        .json()
        .await
        .expect("models json");
    let model_id = models["providers"]
        .as_array()
        .and_then(|providers| {
            providers.iter().find_map(|provider| {
                provider["models"]
                    .as_array()?
                    .first()
                    .and_then(|model| model["id"].as_str())
                    .map(str::to_string)
            })
        })
        .expect("catalog 至少有一个可路由模型");
    let switched = authed(&client, &server, reqwest::Method::PUT, "/v1/sessions/controls-smoke/model")
        .json(&json!({ "model": model_id }))
        .send()
        .await
        .expect("set model");
    assert_eq!(switched.status(), StatusCode::OK);
    let body: Value = switched.json().await.expect("model json");
    assert_eq!(body["model"], Value::String(model_id));

    let rejected = authed(&client, &server, reqwest::Method::PUT, "/v1/sessions/controls-smoke/model")
        .json(&json!({ "model": "no-such-model" }))
        .send()
        .await
        .expect("set model rejected");
    assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        error_code(&rejected.json().await.expect("error body")),
        "unknown_model"
    );
}
