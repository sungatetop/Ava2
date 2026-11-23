//! 通用外联读通道端到端验证（`GET /v1/net/fetch`，真实调腾讯 SkillHub 上游 API）。
//!
//! 覆盖：真实上游取回（状态码/JSON 体透传）、上游 404 透传、私网与非 https
//! URL 拒绝（SSRF 边界）、缺 url 400。

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
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind ephemeral port");
    listener.local_addr().expect("local addr").port()
}

/// 启动服务（显式端口）并等待 `/healthz` 就绪。
async fn spawn_server_on(root: std::path::PathBuf, workspace: &std::path::Path, port: u16) -> Server {
    let child = Command::new(env!("CARGO_BIN_EXE_onion-server"))
        .arg("--port")
        .arg(port.to_string())
        .env("ONION_HOME", &root)
        .env("ONION_SERVER_WORKSPACE", workspace)
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

/// 真实上游取回：状态码与 JSON 体透传（search API 返回 200 + results 数组）。
#[tokio::test]
async fn fetch_real_upstream_returns_body() {
    let root = std::env::temp_dir().join(format!("onion_net_up_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let workspace = root.join("ws");
    std::fs::create_dir_all(&workspace).expect("create workspace");
    let server = spawn_server_on(root.clone(), &workspace, free_port()).await;
    let client = reqwest::Client::new();

    let url = "https://api.skillhub.cn/api/v1/search?q=pdf&limit=2";
    let response = authed(
        &client,
        &server,
        reqwest::Method::GET,
        &format!("/v1/net/fetch?url={}", urlencode(url)),
    )
    .send()
    .await
    .expect("net fetch upstream");
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.expect("net fetch json");
    assert_eq!(body["status"], json!(200), "上游状态码透传");
    assert!(body["contentType"].as_str().expect("content-type").contains("json"));
    let upstream: Value = serde_json::from_str(body["body"].as_str().expect("body str")).expect("body json");
    let results = upstream["results"].as_array().expect("results array");
    assert!(!results.is_empty(), "search pdf 应有结果");
    assert!(!results[0]["slug"].as_str().expect("slug").is_empty());
}

/// 上游 404 透传（非 200 是上游状态，不是网关错误）。
#[tokio::test]
async fn fetch_upstream_404_passthrough() {
    let root = std::env::temp_dir().join(format!("onion_net_404_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let workspace = root.join("ws");
    std::fs::create_dir_all(&workspace).expect("create workspace");
    let server = spawn_server_on(root.clone(), &workspace, free_port()).await;
    let client = reqwest::Client::new();

    let url = "https://api.skillhub.cn/api/v1/download?slug=no-such-skill-zzz-404";
    let response = authed(
        &client,
        &server,
        reqwest::Method::GET,
        &format!("/v1/net/fetch?url={}", urlencode(url)),
    )
    .send()
    .await
    .expect("net fetch 404 upstream");
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.expect("net fetch json");
    assert_eq!(body["status"], json!(404), "上游 404 应透传");
}

/// POST 通用形态：method/headers 透传（上游对 GET-only 端点回 405，证明 method
/// 原样到达）；非 GET/POST 方法 400。
#[tokio::test]
async fn fetch_post_forwards_method_and_validates() {
    let root = std::env::temp_dir().join(format!("onion_net_post_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let workspace = root.join("ws");
    std::fs::create_dir_all(&workspace).expect("create workspace");
    let server = spawn_server_on(root.clone(), &workspace, free_port()).await;
    let client = reqwest::Client::new();

    // POST 到 GET-only 的 search 端点 → 上游 405 透传（method 确实原样转发）。
    let response = authed(&client, &server, reqwest::Method::POST, "/v1/net/fetch")
        .json(&json!({
            "url": "https://api.skillhub.cn/api/v1/search?q=pdf&limit=1",
            "method": "POST",
            "headers": { "content-type": "application/json", "x-custom": "dropped" },
            "body": "{}"
        }))
        .send()
        .await
        .expect("net fetch post");
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.expect("net fetch json");
    assert_eq!(body["status"], json!(405), "上游 405 应透传（method 原样到达）");

    // 非法方法 → 400。
    let bad = authed(&client, &server, reqwest::Method::POST, "/v1/net/fetch")
        .json(&json!({ "url": "https://api.skillhub.cn/api/v1/search", "method": "DELETE" }))
        .send()
        .await
        .expect("net fetch bad method");
    assert_eq!(bad.status(), StatusCode::BAD_REQUEST);
}

/// dataUrl 编码：响应体为 `data:<content-type>;base64,` 形态。
#[tokio::test]
async fn fetch_dataurl_encoding() {
    let root = std::env::temp_dir().join(format!("onion_net_dataurl_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let workspace = root.join("ws");
    std::fs::create_dir_all(&workspace).expect("create workspace");
    let server = spawn_server_on(root.clone(), &workspace, free_port()).await;
    let client = reqwest::Client::new();

    let url = "https://api.skillhub.cn/api/v1/search?q=pdf&limit=1";
    let response = authed(&client, &server, reqwest::Method::POST, "/v1/net/fetch")
        .json(&json!({ "url": url, "encoding": "dataUrl" }))
        .send()
        .await
        .expect("net fetch dataUrl");
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.expect("net fetch json");
    let text = body["body"].as_str().expect("body str");
    assert!(
        text.starts_with("data:application/json;base64,") || text.starts_with("data:"),
        "dataUrl 编码应带 data: 前缀：{text:.60}"
    );
    // base64 解码后应为合法 JSON。
    let b64 = text.split(";base64,").nth(1).expect("base64 part");
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .expect("base64 decode");
    let upstream: Value = serde_json::from_slice(&bytes).expect("decoded json");
    assert!(!upstream["results"].as_array().expect("results").is_empty());
}

/// SSRF 边界：私网 / 环回 / 非 https / 无点主机名一律 400 拒绝。
#[tokio::test]
async fn fetch_blocks_private_and_non_https() {
    let root = std::env::temp_dir().join(format!("onion_net_ssrf_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let workspace = root.join("ws");
    std::fs::create_dir_all(&workspace).expect("create workspace");
    let server = spawn_server_on(root.clone(), &workspace, free_port()).await;
    let client = reqwest::Client::new();

    for url in [
        "http://127.0.0.1:9800/healthz",          // 环回
        "https://127.0.0.1/healthz",              // 私网 IP（即便 https）
        "http://example.com/",                    // 非 https
        "https://localhost/healthz",              // localhost
        "https://intranet/healthz",               // 无点单标签主机
        "ftp://example.com/file",                 // 非 http(s) scheme
    ] {
        let response = authed(
            &client,
            &server,
            reqwest::Method::GET,
            &format!("/v1/net/fetch?url={}", urlencode(url)),
        )
        .send()
        .await
        .unwrap_or_else(|_| panic!("fetch {url}"));
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "应拒绝 {url}");
    }

    // 缺 url → 400。
    let missing = authed(&client, &server, reqwest::Method::GET, "/v1/net/fetch")
        .send()
        .await
        .expect("fetch without url");
    assert_eq!(missing.status(), StatusCode::BAD_REQUEST);
}

/// 查询串编码（reqwest Url percent-encode 的最小等价）。
fn urlencode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
