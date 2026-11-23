//! `/v1/attachments` 端到端冒烟：启动真实 `onion-server` 子进程 + 真实 HTTP 调用。
//!
//! 覆盖：图片批量上传（引用形状 / 内容寻址去重）、文件上传（canonical 引用 +
//! 允许空字节）、限额与错误模型（`INVALID_IMAGE_BASE64` / `TOO_MANY_IMAGES` /
//! `INVALID_FILE_BASE64`）、鉴权保护。消息入口 bytes 路径（`parts_to_agent_message`）
//! 由 `src/rest/dto_tests.rs` 单测覆盖。

// 集成测试的装配辅助函数不在 `#[test]` 体内，`allow-expect-in-tests` 不覆盖，
// 这里按测试文件粒度放行（失败即断言失败）。
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use reqwest::StatusCode;
use serde_json::{json, Value};

/// 测试用访问令牌。
const TOKEN: &str = "test-onion-token";

/// 1x1 PNG（70 字节，头签名 89504e470d0a1a0a）。
const PNG_1PX_B64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

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

/// 启动服务并等待 `/healthz` 就绪（`label` 隔离各测试的临时数据根）。
async fn spawn_server(label: &str) -> Server {
    let root = std::env::temp_dir().join(format!("onion_rest_attachments_{label}_{}", std::process::id()));
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
fn authed(
    client: &reqwest::Client,
    server: &Server,
    path: &str,
) -> reqwest::RequestBuilder {
    client
        .post(format!("{}{path}", server.base))
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
async fn rest_attachments_smoke() {
    let server = spawn_server("smoke").await;
    let client = reqwest::Client::new();

    // ---- 鉴权：附件端点受保护。----
    let denied = client
        .post(format!("{}/v1/attachments/images", server.base))
        .json(&json!({"images": []}))
        .send()
        .await
        .expect("unauthenticated upload");
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);

    // ---- 图片上传：引用形状。----
    let image_payload = json!({"images": [
        {"mediaType": "image/png", "data": PNG_1PX_B64, "name": "pixel.png"}
    ]});
    let first: Value = authed(&client, &server, "/v1/attachments/images")
        .json(&image_payload)
        .send()
        .await
        .expect("upload image")
        .json()
        .await
        .expect("upload json");
    let refs = first["attachments"].as_array().expect("attachments array");
    assert_eq!(refs.len(), 1);
    let ref0 = &refs[0];
    assert!(
        ref0["attachmentId"].as_str().unwrap_or_default().starts_with("sha256:"),
        "unexpected attachment id: {ref0}"
    );
    assert_eq!(ref0["mediaType"], "image/png");
    assert_eq!(ref0["bytes"], 70);
    assert_eq!(ref0["width"], 1);
    assert_eq!(ref0["height"], 1);
    assert_eq!(ref0["name"], "pixel.png");

    // ---- 同内容再次上传 → 同 id（内容寻址去重）。----
    let second: Value = authed(&client, &server, "/v1/attachments/images")
        .json(&image_payload)
        .send()
        .await
        .expect("re-upload image")
        .json()
        .await
        .expect("re-upload json");
    assert_eq!(second["attachments"][0]["attachmentId"], ref0["attachmentId"]);

    // ---- 非法 base64 → 400 INVALID_IMAGE_BASE64。----
    let bad = authed(&client, &server, "/v1/attachments/images")
        .json(&json!({"images": [{"mediaType": "image/png", "data": "not-base64!!!"}]}))
        .send()
        .await
        .expect("bad upload");
    assert_eq!(bad.status(), StatusCode::BAD_REQUEST);
    assert_eq!(error_code(&bad.json().await.expect("error json")), "INVALID_IMAGE_BASE64");

    // ---- 9 张 → 400 TOO_MANY_IMAGES（缺省批量张数限额）。----
    let images: Vec<_> =
        (0..9).map(|_| json!({"mediaType": "image/png", "data": PNG_1PX_B64})).collect();
    let many = authed(&client, &server, "/v1/attachments/images")
        .json(&json!({ "images": images }))
        .send()
        .await
        .expect("many upload");
    assert_eq!(many.status(), StatusCode::BAD_REQUEST);
    assert_eq!(error_code(&many.json().await.expect("error json")), "TOO_MANY_IMAGES");

    // ---- 文件上传：canonical 引用。----
    let file: Value = authed(&client, &server, "/v1/attachments/files")
        .json(&json!({"data": "aGVsbG8=", "name": "notes.txt"}))
        .send()
        .await
        .expect("upload file")
        .json()
        .await
        .expect("file json");
    assert!(
        file["attachmentId"].as_str().unwrap_or_default().starts_with("sha256:"),
        "unexpected file id: {file}"
    );
    assert_eq!(file["name"], "notes.txt");
    assert_eq!(file["bytes"], 5);

    // ---- 空文件允许（文件无空限制）。----
    let empty: Value = authed(&client, &server, "/v1/attachments/files")
        .json(&json!({"data": "", "name": "empty.bin"}))
        .send()
        .await
        .expect("empty file upload")
        .json()
        .await
        .expect("empty file json");
    assert_eq!(empty["bytes"], 0);

    // ---- 非法文件 base64 → 400 INVALID_FILE_BASE64。----
    let bad_file = authed(&client, &server, "/v1/attachments/files")
        .json(&json!({"data": "@@@@", "name": "x.bin"}))
        .send()
        .await
        .expect("bad file upload");
    assert_eq!(bad_file.status(), StatusCode::BAD_REQUEST);
    assert_eq!(error_code(&bad_file.json().await.expect("error json")), "INVALID_FILE_BASE64");
}
