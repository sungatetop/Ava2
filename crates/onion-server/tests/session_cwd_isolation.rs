//! 会话级工作目录端到端验证。
//!
//! 覆盖：同一实例内两个不同 `cwd` 的会话各自独立（`root` / `workingDir` /
//! 落盘容器互不相同）、`cwd` 校验（相对路径 / 不存在目录一律 400）、
//! 重启后按会话 id 跨工作目录恢复。

// 集成测试的装配辅助函数不在 `#[test]` 体内，`allow-expect-in-tests` 不覆盖，
// 这里按测试文件粒度放行（失败即断言失败）。
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use reqwest::StatusCode;
use serde_json::{json, Value};

const TOKEN: &str = "test-onion-token";

/// 子进程守卫：退出时 kill 并清理临时数据目录。
struct Server {
    child: Child,
    base: String,
    root: std::path::PathBuf,
    /// 为 true 时退出不清理数据目录（模拟重启，保留落盘会话）。
    keep_data: bool,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if !self.keep_data {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
}

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind ephemeral port");
    listener.local_addr().expect("local addr").port()
}

/// 启动服务并等待 `/healthz` 就绪（`port` 复用可模拟重启）。
async fn spawn_server(root: std::path::PathBuf, port: u16) -> Server {
    spawn_server_keeping(root, port, false).await
}

/// 同 [`spawn_server`]，`keep_data = true` 时退出不清理数据目录。
async fn spawn_server_keeping(root: std::path::PathBuf, port: u16, keep_data: bool) -> Server {
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).expect("create temp workspace");
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
    Server { child, base, root, keep_data }
}

fn authed(
    client: &reqwest::Client,
    server: &Server,
    method: reqwest::Method,
    path: &str,
) -> reqwest::RequestBuilder {
    client
        .request(method, format!("{}{path}", server.base))
        .header("X-Onion-Token", TOKEN)
}

/// 同一实例内两个不同 `cwd` 的会话：根、有效目录、落盘容器互不相同。
#[tokio::test]
async fn sessions_with_distinct_cwd_are_isolated() {
    let root = std::env::temp_dir().join(format!("onion_cwd_iso_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let ws_a = root.join("ws-a");
    let ws_b = root.join("ws-b");
    std::fs::create_dir_all(&ws_a).expect("ws-a");
    std::fs::create_dir_all(&ws_b).expect("ws-b");
    let server = spawn_server(root.clone(), free_port()).await;
    let client = reqwest::Client::new();

    let mut roots = Vec::new();
    for (id, cwd) in [("iso-a", &ws_a), ("iso-b", &ws_b)] {
        let created = authed(&client, &server, reqwest::Method::POST, "/v1/sessions")
            .json(&json!({ "id": id, "cwd": cwd.display().to_string() }))
            .send()
            .await
            .expect("create session");
        assert_eq!(created.status(), StatusCode::CREATED);
        let body: Value = created.json().await.expect("session json");
        assert_eq!(body["id"], id);
        // 权限根保留调用方拼写（preserve execution-world spelling）。
        let expected = cwd.display().to_string();
        assert_eq!(body["root"], expected);
        assert_eq!(body["workingDir"], expected);
        roots.push(body["root"].as_str().expect("root").to_string());
    }
    assert_ne!(roots[0], roots[1], "两个会话的权限根必须不同");

    // 落盘容器按 cwd 分区：`<data_home>/sessions/<normalized-cwd>/<id>/`。
    let sessions_root = root.join("sessions");
    let containers: Vec<_> = std::fs::read_dir(&sessions_root)
        .expect("sessions root")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    assert_eq!(2, containers.len(), "两个 cwd 应各有一个落盘容器");
    for id in ["iso-a", "iso-b"] {
        let hits = containers
            .iter()
            .filter(|dir| dir.join(id).is_dir())
            .count();
        assert_eq!(1, hits, "会话 {id} 应只落在一个容器里");
    }

    // 默认工作目录（未给 cwd）与显式 cwd 的会话互不干扰。
    let default_session = authed(&client, &server, reqwest::Method::POST, "/v1/sessions")
        .json(&json!({ "id": "iso-default" }))
        .send()
        .await
        .expect("create default session");
    assert_eq!(default_session.status(), StatusCode::CREATED);
    let default_body: Value = default_session.json().await.expect("default json");
    assert_eq!(
        default_body["root"],
        root.join("workspace").display().to_string()
    );
}

/// `cwd` 校验：相对路径与不存在的目录一律 400（不做隐式建目录）。
#[tokio::test]
async fn invalid_cwd_is_rejected() {
    let root = std::env::temp_dir().join(format!("onion_cwd_bad_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let server = spawn_server(root.clone(), free_port()).await;
    let client = reqwest::Client::new();

    for cwd in ["relative/ws", "ws-a"] {
        let response = authed(&client, &server, reqwest::Method::POST, "/v1/sessions")
            .json(&json!({ "id": "bad", "cwd": cwd }))
            .send()
            .await
            .expect("create session");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "cwd={cwd}");
    }
    let missing = root.join("not-created");
    let response = authed(&client, &server, reqwest::Method::POST, "/v1/sessions")
        .json(&json!({ "id": "bad", "cwd": missing.display().to_string() }))
        .send()
        .await
        .expect("create session");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(!missing.exists(), "不应隐式创建工作目录");
}

/// 重启后按会话 id 跨工作目录恢复（`open` 扫全部落盘容器）。
#[tokio::test]
async fn session_survives_restart_across_containers() {
    let root = std::env::temp_dir().join(format!("onion_cwd_restart_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let ws_a = root.join("ws-a");
    std::fs::create_dir_all(&ws_a).expect("ws-a");
    let port = free_port();
    {
        let server = spawn_server_keeping(root.clone(), port, true).await;
        let client = reqwest::Client::new();
        let created = authed(&client, &server, reqwest::Method::POST, "/v1/sessions")
            .json(&json!({ "id": "restart-a", "cwd": ws_a.display().to_string() }))
            .send()
            .await
            .expect("create session");
        assert_eq!(created.status(), StatusCode::CREATED);
    }
    // 进程已退出：新实例应能从非兜底容器里找回该会话。
    let server = spawn_server(root.clone(), port).await;
    let client = reqwest::Client::new();
    let reopened = authed(&client, &server, reqwest::Method::GET, "/v1/sessions/restart-a")
        .send()
        .await
        .expect("get session after restart");
    assert_eq!(reopened.status(), StatusCode::OK);
    let body: Value = reopened.json().await.expect("session json");
    assert_eq!(body["id"], "restart-a");
    // 冷恢复以容器 `cwd` 标记还原权限根（容器名有损，不能靠名字反解）。
    assert_eq!(body["root"], ws_a.display().to_string());
    assert_eq!(body["workingDir"], ws_a.display().to_string());
}
