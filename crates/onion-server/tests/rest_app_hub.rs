//! app-hub REST 端到端验证（`GET /v1/apps/hub` + `POST /v1/apps/hub/install`）。
//!
//! 真实调 GitHub 仓库的 `apps/` 目录：用本地 HTTP 桩按 GitHub 契约提供
//! `/repos/{repo}/git/trees/{branch}?recursive=1` 文件树与
//! `/{repo}/{branch}/{path}` raw 内容，内容取自仓库内真实的
//! `apps/hotboard/dist/`（热点聚合 App 的预构建产物），从而完整验证
//! 「列目录 → 搜索 → 下载 → 校验 → 落盘 → 登记」链路而不依赖外网。
//!
//! 覆盖：列表含 hotboard、关键字命中 / 不命中、真实下载安装后落盘
//! `<apps_dir>/<id>/` 且 `GET /v1/apps` 可见、已安装标记回写、重复安装 409、
//! 未知 App 404。

// 集成测试的装配辅助函数不在 `#[test]` 体内，`allow-expect-in-tests` 不覆盖，
// 这里按测试文件粒度放行（失败即断言失败）。
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use axum::extract::{Path as AxumPath, State};
use axum::http::StatusCode as AxumStatus;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use reqwest::StatusCode;
use serde_json::{json, Value};

const TOKEN: &str = "test-onion-token";
const HUB_REPO: &str = "sungatetop/Ava2";
const HUB_BRANCH: &str = "main";
/// 仓库内已入库预构建产物的示例 App（热点聚合）。
const SAMPLE_APP_ID: &str = "hotboard";

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

/// 本地 GitHub 桩的仓库内容：路径 → 字节。
type RepoFiles = BTreeMap<String, Vec<u8>>;

/// 单入口桩：按 GitHub 契约分流文件树与 raw 内容（repo 含 `/`，故用 catch-all）。
async fn stub_dispatch(
    State(files): State<RepoFiles>,
    AxumPath(rest): AxumPath<String>,
) -> impl IntoResponse {
    let tree_prefix = format!("repos/{HUB_REPO}/git/trees/{HUB_BRANCH}");
    if rest == tree_prefix {
        let tree: Vec<Value> = files
            .keys()
            .map(|path| json!({ "path": path, "type": "blob" }))
            .collect();
        return axum::Json(json!({ "tree": tree, "truncated": false })).into_response();
    }
    let raw_prefix = format!("{HUB_REPO}/{HUB_BRANCH}/");
    match rest.strip_prefix(&raw_prefix) {
        Some(path) => match files.get(path) {
            Some(body) => (AxumStatus::OK, body.clone()).into_response(),
            None => (AxumStatus::NOT_FOUND, Vec::new()).into_response(),
        },
        None => (AxumStatus::NOT_FOUND, Vec::new()).into_response(),
    }
}

/// 仓库内真实产物（构建型 App 的 `dist/` 目录）。
fn hotboard_dist() -> RepoFiles {
    let dist = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps")
        .join(SAMPLE_APP_ID)
        .join("dist");
    let mut files = RepoFiles::new();
    for entry in walkdir(&dist) {
        let relative = entry
            .strip_prefix(&dist)
            .expect("relative path")
            .to_string_lossy()
            .replace('\\', "/");
        let path = format!("apps/{SAMPLE_APP_ID}/{relative}");
        files.insert(path, std::fs::read(&entry).expect("read dist file"));
    }
    assert!(
        files.contains_key(&format!("apps/{SAMPLE_APP_ID}/manifest.json")),
        "缺少 {SAMPLE_APP_ID} 预构建产物，请先执行 apps/{SAMPLE_APP_ID} 的构建"
    );
    files
}

/// 递归列目录下的普通文件。
fn walkdir(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind ephemeral port");
    listener.local_addr().expect("local addr").port()
}

/// 启动本地 GitHub 桩，返回基址。
async fn spawn_stub(files: RepoFiles) -> String {
    let app = Router::new()
        .route("/{*rest}", get(stub_dispatch))
        .with_state(files);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind stub");
    let addr = listener.local_addr().expect("stub addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    format!("http://{addr}")
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

/// 列表 → 查询过滤 → 下载安装 → 已安装可见 → 重复安装 409 → 未知 App 404。
#[tokio::test]
async fn hub_lists_searches_and_installs_app() {
    let dist = hotboard_dist();
    let stub = spawn_stub(dist.clone()).await;

    let root = std::env::temp_dir().join(format!("onion_app_hub_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let workspace = root.join("ws");
    std::fs::create_dir_all(&workspace).expect("create workspace");

    let port = free_port();
    let child = Command::new(env!("CARGO_BIN_EXE_onion-server"))
        .arg("--port")
        .arg(port.to_string())
        .env("ONION_HOME", &root)
        .env("ONION_SERVER_WORKSPACE", &workspace)
        .env("ONION_SERVER_TOKEN", TOKEN)
        .env("ONION_BUNDLED_APPS_DIR", workspace.join("no-bundled-apps"))
        .env("ONION_APP_HUB_REPO", HUB_REPO)
        .env("ONION_APP_HUB_BRANCH", HUB_BRANCH)
        .env("ONION_APP_HUB_API_BASE", &stub)
        .env("ONION_APP_HUB_RAW_BASE", &stub)
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
    let server = Server {
        child,
        base,
        root: root.clone(),
    };

    // 全量列表：仓库 apps/ 下可安装的预构建 App。
    let listed = authed(&client, &server, reqwest::Method::GET, "/v1/apps/hub")
        .send()
        .await
        .expect("list hub");
    assert_eq!(listed.status(), StatusCode::OK);
    let body: Value = listed.json().await.expect("hub json");
    assert_eq!(body["source"], json!(format!("{HUB_REPO}@{HUB_BRANCH}")));
    let apps = body["apps"].as_array().expect("apps array");
    assert!(
        apps.iter()
            .any(|a| a["manifest"]["id"] == json!(SAMPLE_APP_ID)),
        "Hub 列表应含 {SAMPLE_APP_ID}：{apps:?}"
    );
    let entry = apps
        .iter()
        .find(|a| a["manifest"]["id"] == json!(SAMPLE_APP_ID))
        .expect("hotboard entry");
    assert_eq!(entry["manifest"]["name"], json!("热榜聚合"));
    assert_eq!(entry["installed"], json!(false));

    // 关键字查询：命中。
    let hit = authed(
        &client,
        &server,
        reqwest::Method::GET,
        "/v1/apps/hub?q=hotboard",
    )
    .send()
    .await
    .expect("search hub");
    let hit_body: Value = hit.json().await.expect("search json");
    assert_eq!(
        hit_body["apps"].as_array().expect("apps").len(),
        1,
        "关键字过滤应只返回匹配项：{hit_body:?}"
    );

    // 关键字查询：不命中。
    let miss = authed(
        &client,
        &server,
        reqwest::Method::GET,
        "/v1/apps/hub?q=zzz-no-such-app",
    )
    .send()
    .await
    .expect("search miss");
    let miss_body: Value = miss.json().await.expect("miss json");
    assert_eq!(miss_body["apps"].as_array().expect("apps").len(), 0);

    // 真实下载安装。
    let installed = authed(
        &client,
        &server,
        reqwest::Method::POST,
        "/v1/apps/hub/install",
    )
    .json(&json!({ "id": SAMPLE_APP_ID }))
    .send()
    .await
    .expect("install app");
    assert_eq!(installed.status(), StatusCode::OK);
    let installed_body: Value = installed.json().await.expect("install json");
    assert_eq!(installed_body["manifest"]["id"], json!(SAMPLE_APP_ID));

    // 落盘：`<root>/apps/<id>/manifest.json` 与入口产物。
    let app_dir = root.join("apps").join(SAMPLE_APP_ID);
    assert!(app_dir.join("manifest.json").is_file(), "清单应落盘");
    assert!(app_dir.join("main.js").is_file(), "入口产物应落盘");

    // 已安装清单可见。
    let apps_resp = authed(&client, &server, reqwest::Method::GET, "/v1/apps")
        .send()
        .await
        .expect("list installed");
    let apps_body: Value = apps_resp.json().await.expect("installed json");
    assert!(
        apps_body["apps"]
            .as_array()
            .expect("apps")
            .iter()
            .any(|a| a["manifest"]["id"] == json!(SAMPLE_APP_ID)),
        "安装后 GET /v1/apps 应可见：{apps_body:?}"
    );

    // 已安装标记回写列表。
    let after = authed(
        &client,
        &server,
        reqwest::Method::GET,
        "/v1/apps/hub?q=hotboard",
    )
    .send()
    .await
    .expect("list after install");
    let after_body: Value = after.json().await.expect("after json");
    assert_eq!(after_body["apps"][0]["installed"], json!(true));

    // 重复安装 → 409。
    let again = authed(
        &client,
        &server,
        reqwest::Method::POST,
        "/v1/apps/hub/install",
    )
    .json(&json!({ "id": SAMPLE_APP_ID }))
    .send()
    .await
    .expect("install again");
    assert_eq!(again.status(), StatusCode::CONFLICT);

    // 未知 App → 404。
    let unknown = authed(
        &client,
        &server,
        reqwest::Method::POST,
        "/v1/apps/hub/install",
    )
    .json(&json!({ "id": "no-such-app-zzz" }))
    .send()
    .await
    .expect("install unknown");
    assert_eq!(unknown.status(), StatusCode::NOT_FOUND);
}
