//! skills-manager REST 端到端验证（`GET/PUT /v1/skills-manager` + `GET /v1/skills`
//! 的 `enabled` 标记联动）。
//!
//! 覆盖：PUT 回读与落盘一致（校验 + 去重排序 + 原子写）、非法名 400、
//! 禁用后 `GET /v1/skills` 列表仍可见且 `enabled=false`。

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

/// 在 `<root>/skills/<name>/SKILL.md` 落一个最小技能。
fn write_skill(workspace: &std::path::Path, name: &str) {
    let dir = workspace.join(".onion").join("skills").join(name);
    std::fs::create_dir_all(&dir).expect("create skill dir");
    std::fs::write(
        dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: Demo skill {name}\n---\nBody of {name}.\n"),
    )
    .expect("write SKILL.md");
}

/// PUT /v1/skills-manager 并返回响应（调用方断言状态码）。
async fn put_disabled(
    client: &reqwest::Client,
    server: &Server,
    session: &str,
    disabled: Value,
) -> reqwest::Response {
    authed(client, server, reqwest::Method::PUT, &format!("/v1/skills-manager?session={session}"))
        .json(&json!({ "disabled": disabled }))
        .send()
        .await
        .expect("put skills-manager")
}

/// PUT 后回读 GET，返回 `(disabled 列表, file 路径)`。
async fn get_disabled(client: &reqwest::Client, server: &Server, session: &str) -> (Vec<String>, String) {
    let response = authed(
        client,
        server,
        reqwest::Method::GET,
        &format!("/v1/skills-manager?session={session}"),
    )
    .send()
    .await
    .expect("get skills-manager");
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.expect("skills-manager json");
    (
        body["disabled"]
            .as_array()
            .expect("disabled array")
            .iter()
            .map(|v| v.as_str().expect("name str").to_string())
            .collect(),
        body["file"].as_str().expect("file str").to_string(),
    )
}

/// PUT → GET 回读一致 + 落盘文件校验（非法名剔除、去重、排序）。
#[tokio::test]
async fn put_then_get_roundtrip_and_persist() {
    let root = std::env::temp_dir().join(format!("onion_sm_roundtrip_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let workspace = root.join("ws");
    std::fs::create_dir_all(&workspace).expect("create workspace");
    let server = spawn_server_on(root.clone(), &workspace, free_port()).await;
    let client = reqwest::Client::new();

    let created = authed(&client, &server, reqwest::Method::POST, "/v1/sessions")
        .json(&json!({ "id": "sm-a", "cwd": workspace.display().to_string() }))
        .send()
        .await
        .expect("create session");
    assert_eq!(created.status(), StatusCode::CREATED);

    let response = put_disabled(&client, &server, "sm-a", json!(["zzz-late", "aa-early", "zzz-late"])).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.expect("put json");
    assert_eq!(body["disabled"], json!(["aa-early", "zzz-late"]), "去重 + 排序");
    let file = body["file"].as_str().expect("file str").to_string();
    assert_eq!(
        file,
        workspace.join(".onion").join("skills-manager.json").display().to_string(),
        "落盘路径为 <workspace>/.onion/skills-manager.json"
    );

    // 落盘内容与回读一致。
    let raw = std::fs::read_to_string(&file).expect("read persisted file");
    let persisted: Value = serde_json::from_str(&raw).expect("persisted json");
    assert_eq!(persisted["disabled"], json!(["aa-early", "zzz-late"]));

    // GET 与 PUT 一致。
    let (disabled, got_file) = get_disabled(&client, &server, "sm-a").await;
    assert_eq!(disabled, vec!["aa-early".to_string(), "zzz-late".to_string()]);
    assert_eq!(got_file, file);
}

/// 非法名（非 kebab-case）PUT 一律 400。
#[tokio::test]
async fn put_invalid_name_is_rejected() {
    let root = std::env::temp_dir().join(format!("onion_sm_invalid_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let workspace = root.join("ws");
    std::fs::create_dir_all(&workspace).expect("create workspace");
    let server = spawn_server_on(root.clone(), &workspace, free_port()).await;
    let client = reqwest::Client::new();

    let created = authed(&client, &server, reqwest::Method::POST, "/v1/sessions")
        .json(&json!({ "id": "sm-b", "cwd": workspace.display().to_string() }))
        .send()
        .await
        .expect("create session");
    assert_eq!(created.status(), StatusCode::CREATED);

    for disabled in [json!(["Bad_Name"]), json!(["ok-name", "no/slash"])] {
        let response = put_disabled(&client, &server, "sm-b", disabled).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    // 被拒请求不落盘。
    assert!(
        !workspace.join(".onion").join("skills-manager.json").exists(),
        "400 后不应留下清单文件"
    );
}

/// 禁用后 `GET /v1/skills` 列表仍可见且 `enabled=false`；启停状态经清单文件联动。
#[tokio::test]
async fn disabled_skill_stays_listed_with_enabled_false() {
    let root = std::env::temp_dir().join(format!("onion_sm_enabled_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let workspace = root.join("ws");
    std::fs::create_dir_all(&workspace).expect("create workspace");
    write_skill(&workspace, "demo-a");
    write_skill(&workspace, "demo-b");
    let server = spawn_server_on(root.clone(), &workspace, free_port()).await;
    let client = reqwest::Client::new();

    let created = authed(&client, &server, reqwest::Method::POST, "/v1/sessions")
        .json(&json!({ "id": "sm-c", "cwd": workspace.display().to_string() }))
        .send()
        .await
        .expect("create session");
    assert_eq!(created.status(), StatusCode::CREATED);

    let skills = async |client: &reqwest::Client, server: &Server| -> Vec<(String, bool)> {
        let response = authed(
            client,
            server,
            reqwest::Method::GET,
            "/v1/skills?session=sm-c",
        )
        .send()
        .await
        .expect("get skills");
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value = response.json().await.expect("skills json");
        body["skills"]
            .as_array()
            .expect("skills array")
            .iter()
            .map(|s| {
                (
                    s["name"].as_str().expect("name").to_string(),
                    s["enabled"].as_bool().expect("enabled bool"),
                )
            })
            .collect()
    };

    // 初始：project 技能全部 enabled（列表可能含宿主环境 user 级技能，只断言目标子集）。
    let listed = skills(&client, &server).await;
    let enabled_of = |list: &[(String, bool)], name: &str| -> bool {
        list.iter()
            .find(|(n, _)| n == name)
            .map(|(_, enabled)| *enabled)
            .unwrap_or_else(|| panic!("技能 {name} 应在列表中"))
    };
    assert!(enabled_of(&listed, "demo-a"));
    assert!(enabled_of(&listed, "demo-b"));

    // 禁用 demo-a：列表仍含两个技能，enabled 标记联动。
    let response = put_disabled(&client, &server, "sm-c", json!(["demo-a"])).await;
    assert_eq!(response.status(), StatusCode::OK);
    let listed = skills(&client, &server).await;
    assert!(!enabled_of(&listed, "demo-a"), "禁用后列表仍可见，仅 enabled=false");
    assert!(enabled_of(&listed, "demo-b"));

    // 清单回读一致。
    let (disabled, _) = get_disabled(&client, &server, "sm-c").await;
    assert_eq!(disabled, vec!["demo-a".to_string()]);
}
