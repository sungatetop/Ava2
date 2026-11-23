//! skill-hub REST 端到端验证（`POST /v1/skill-hub/install`，真实调腾讯 SkillHub
//! 上游下载 API）。搜索 / 详情读面已下沉为 App 直连（`tests/rest_net.rs` 覆盖
//! 外联通道），本文件只验证服务端安装链路。
//!
//! 覆盖：真实下载 zip → 解包落盘 `<projectRoot>/.onion/skills/<slug>/` 后注册表
//! 可见、重复安装 409、非法 slug 400、上游不可下载 404。

// 集成测试的装配辅助函数不在 `#[test]` 体内，`allow-expect-in-tests` 不覆盖，
// 这里按测试文件粒度放行（失败即断言失败）。
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use reqwest::StatusCode;
use serde_json::{json, Value};

const TOKEN: &str = "test-onion-token";

/// 真实上游的样例社区技能（clawhub 源，zip 约 23 KiB，SKILL.md name = `pdf`）。
const SAMPLE_SLUG: &str = "latex-formula-extraction-pdf";
const SAMPLE_SKILL_NAME: &str = "pdf";

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

/// 创建 cwd 指向 workspace 的会话。
async fn create_session(client: &reqwest::Client, server: &Server, id: &str, workspace: &std::path::Path) {
    let created = authed(client, server, reqwest::Method::POST, "/v1/sessions")
        .json(&json!({ "id": id, "cwd": workspace.display().to_string() }))
        .send()
        .await
        .expect("create session");
    assert_eq!(created.status(), StatusCode::CREATED);
}

/// 安装：真实下载 zip → 解包到 `<workspace>/.onion/skills/<slug>/` → 注册表可见；
/// 重复安装 409。
#[tokio::test]
async fn install_downloads_and_registers_skill() {
    let root = std::env::temp_dir().join(format!("onion_hub_install_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let workspace = root.join("ws");
    std::fs::create_dir_all(&workspace).expect("create workspace");
    let server = spawn_server_on(root.clone(), &workspace, free_port()).await;
    let client = reqwest::Client::new();
    create_session(&client, &server, "hub-a", &workspace).await;

    // 非法 slug → 400（不发起下载）。
    let invalid = authed(&client, &server, reqwest::Method::POST, "/v1/skill-hub/install?session=hub-a")
        .json(&json!({ "slug": "Bad_Slug" }))
        .send()
        .await
        .expect("install invalid slug");
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);

    // 真实安装。
    let response = authed(&client, &server, reqwest::Method::POST, "/v1/skill-hub/install?session=hub-a")
        .json(&json!({ "slug": SAMPLE_SLUG }))
        .send()
        .await
        .expect("install sample skill");
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.expect("install json");
    assert_eq!(body["slug"], json!(SAMPLE_SLUG));
    assert_eq!(body["name"], json!(SAMPLE_SKILL_NAME));
    assert_eq!(
        body["dir"],
        json!(workspace.join(".onion").join("skills").join(SAMPLE_SLUG).display().to_string())
    );
    assert!(
        body["files"].as_array().expect("files").iter().any(|f| f.as_str() == Some("SKILL.md")),
        "zip 顶层应含 SKILL.md"
    );

    // 落盘校验：SKILL.md 存在。
    let skill_md = workspace.join(".onion").join("skills").join(SAMPLE_SLUG).join("SKILL.md");
    assert!(skill_md.exists(), "SKILL.md 应落盘");

    // 注册表可见（refresh 后无需重启）：GET /v1/skills 含 frontmatter name。
    let listed = authed(&client, &server, reqwest::Method::GET, "/v1/skills?session=hub-a")
        .send()
        .await
        .expect("list skills");
    assert_eq!(listed.status(), StatusCode::OK);
    let skills: Value = listed.json().await.expect("skills json");
    let names: Vec<&str> = skills["skills"]
        .as_array()
        .expect("skills array")
        .iter()
        .filter_map(|s| s["name"].as_str())
        .collect();
    assert!(names.contains(&SAMPLE_SKILL_NAME), "安装后注册表应含 {SAMPLE_SKILL_NAME}：{names:?}");

    // 重复安装 → 409。
    let again = authed(&client, &server, reqwest::Method::POST, "/v1/skill-hub/install?session=hub-a")
        .json(&json!({ "slug": SAMPLE_SLUG }))
        .send()
        .await
        .expect("install again");
    assert_eq!(again.status(), StatusCode::CONFLICT);
}

/// 上游不可下载（企业源 / 未知 slug）→ 404，且不留下半成品目录。
#[tokio::test]
async fn install_undownloadable_slug_fails_clean() {
    let root = std::env::temp_dir().join(format!("onion_hub_undl_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let workspace = root.join("ws");
    std::fs::create_dir_all(&workspace).expect("create workspace");
    let server = spawn_server_on(root.clone(), &workspace, free_port()).await;
    let client = reqwest::Client::new();
    create_session(&client, &server, "hub-b", &workspace).await;

    // 未知 slug（双下载链都 404）。
    let response = authed(&client, &server, reqwest::Method::POST, "/v1/skill-hub/install?session=hub-b")
        .json(&json!({ "slug": "no-such-skill-zzz-404" }))
        .send()
        .await
        .expect("install unknown slug");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert!(
        !workspace.join(".onion").join("skills").join("no-such-skill-zzz-404").exists(),
        "失败安装不应留下目录"
    );
}
