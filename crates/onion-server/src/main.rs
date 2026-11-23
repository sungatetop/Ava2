//! onion-server 进程入口：常驻 HTTP/WS/SSE 服务（A2A v1.0 端点 + `/v1` RESTful API）。
//!
//! 职责：
//! - 从环境变量装配 onion Config（SMOKE_LLM_*）。
//! - 构造 SessionRegistry + RouterState + axum 路由。
//! - 启动 HTTP 监听，多 session 服务（每 context_id 一个 Runtime）。
//!
//! 用环境变量：
//! - `ONION_SERVER_PORT` / `--port N`：监听端口（默认 9800）。
//! - `ONION_SERVER_STDIO` / `--stdio acp|sdk`：改走 stdio JSON-RPC（不起 HTTP 监听，
//!   供编辑器/宿主 SDK 以子进程方式接入；见 `acp` / `sdk_wire` 模块）。
//! - `ONION_SERVER_WORKSPACE`：服务工作区（默认进程当前目录）。
//! - `ONION_SERVER_TOKEN`：非空时 `/v1` 要求 `X-Onion-Token`（运维面）。
//! - `ONION_A2A_TOKEN`：非空时 `/a2a*` 接受该 mesh 令牌或运维令牌任一（agent 面）。
//! - `ONION_SERVER_CORS_ORIGINS`：逗号分隔的允许来源（配置时启用 CORS）。
//! - `ONION_GITHUB_WEBHOOK_SECRET`：非空时启用 `POST /webhooks/github`（HMAC-SHA256
//!   验签；空 = 端点关闭返回 503）；`ONION_GITHUB_WEBHOOK_CWD` 指定会话权限根。
//! - `ONION_BUNDLED_APPS_DIR`：安装包内置默认 App 目录（首次启动种子安装，见
//!   `rest::apps::seed_default_apps`）。
//!
//! 用法：`onion-server [--port N] [--stdio acp|sdk]`（默认 127.0.0.1:9800）。

mod a2a;
mod acp;
mod app_contrib;
mod bridge;
mod convert;
mod jsonrpc;
mod rest;
mod router;
mod scheduler;
mod sdk_wire;
mod session;
mod title;
mod turn_gate;
mod webhook_github;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use a2a::{AgentCapabilities, AgentCard, AgentProvider, AgentSkill};
use onion::config::Config;
use router::RouterState;
use session::SessionRegistry;

#[tokio::main]
async fn main() {
    // ---- 端口解析：--port N 或环境变量 ONION_SERVER_PORT。----
    let args: Vec<String> = std::env::args().collect();
    let port: u16 = args
        .iter()
        .position(|arg| arg == "--port")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .or_else(|| std::env::var("ONION_SERVER_PORT").ok().and_then(|v| v.parse().ok()))
        .unwrap_or(9800);

    // ---- onion Config（环境变量装配）。----
    let config = Arc::new(Config::from_env());
    eprintln!(
        "[onion-server] model={} base_url={}",
        config.model, config.base_url
    );

    // ---- 日志目录：data_home()/logs/onion-server。----
    let log_dir = onion::paths::logs_dir(&onion::paths::data_home()).join("onion-server");
    if let Err(error) = std::fs::create_dir_all(&log_dir) {
        eprintln!("[onion-server] 日志目录创建失败: {error}");
    }

    // ---- spill 工件 GC：进程启动清扫一次过期工件（按 mtime，缺省 30 天）。----
    let spill_root = onion::paths::spill_dir(&onion::paths::data_home());
    let spill_cutoff = onion::spill::cleanup::cleanup_cutoff(
        std::time::SystemTime::now(),
        onion::spill::cleanup::DEFAULT_RETENTION_DAYS,
    );
    match onion::spill::cleanup::sweep_spill_root(&spill_root, spill_cutoff) {
        Ok(stats) if stats.removed_files > 0 || stats.pruned_dirs > 0 => {
            eprintln!(
                "[onion-server] spill GC: removed {} files ({} bytes), pruned {} dirs",
                stats.removed_files, stats.removed_bytes, stats.pruned_dirs
            );
        }
        Ok(_) => {}
        Err(error) => eprintln!("[onion-server] spill GC 失败: {error}"),
    }

    // ---- 兜底工作目录与会话落盘容器（`ONION_HOME/sessions/<normalized-cwd>/`）。----
    // 会话未显式给 `cwd` 时用它（兜底工作目录语义）。
    let workspace_root = match std::env::var("ONION_SERVER_WORKSPACE") {
        Ok(dir) if !dir.trim().is_empty() => PathBuf::from(dir.trim()),
        _ => match std::env::current_dir() {
            Ok(dir) => dir,
            Err(error) => {
                eprintln!("[onion-server] 无法解析当前工作目录: {error}");
                std::process::exit(1);
            }
        },
    };
    let default_sessions_dir =
        onion::paths::sessions_dir(&onion::paths::data_home(), &workspace_root);
    eprintln!("[onion-server] workspace_root={}", workspace_root.display());
    eprintln!(
        "[onion-server] default_sessions_dir={}",
        default_sessions_dir.display()
    );

    // ---- 可选访问令牌（运维面 `/v1`）。----
    let token: Option<Arc<str>> = std::env::var("ONION_SERVER_TOKEN")
        .ok()
        .map(|raw| raw.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(Arc::from);

    // ---- A2A mesh 令牌（`/a2a*` 接受它与运维令牌任一；来自 onion Config）。----
    let a2a_token: Option<Arc<str>> = config.a2a_token.as_deref().map(Arc::from);

    // ---- AgentCard（well-known 发现文档）。----
    let instance_id = onion::paths::instance_id();
    let own_url = format!("http://127.0.0.1:{port}");
    let agent_card = Arc::new(AgentCard {
        name: "onion".to_string(),
        description: Some("Onion agent over A2A v1.0 protocol".to_string()),
        url: own_url.clone(),
        provider: Some(AgentProvider {
            organization: "onion".to_string(),
            url: "https://github.com/onion".to_string(),
        }),
        version: env!("CARGO_PKG_VERSION").to_string(),
        documentation_url: None,
        capabilities: AgentCapabilities {
            streaming: Some(true),
            push_notifications: Some(true),
            state_transition_history: Some(true),
        },
        default_input_modes: vec!["text".to_string()],
        default_output_modes: vec!["text".to_string()],
        skills: vec![AgentSkill::new(
            "general-assistant",
            "General assistant",
            "General purpose coding and reasoning assistant",
            vec!["assistant".to_string(), "coding".to_string()],
        )],
        instance_id: Some(instance_id.clone()),
    });

    // ---- Router state + axum 路由。----
    let peers = Arc::new(peers_table());
    // AppHost App 仓库：安装目录（env 覆盖，缺省 `<data_home>/apps`）。
    let apps_dir = app_contrib::apps_dir();
    if let Err(error) = std::fs::create_dir_all(&apps_dir) {
        eprintln!("[onion-server] 创建 App 仓库目录失败 {}: {error}", apps_dir.display());
        std::process::exit(1);
    }
    // 首次启动种子安装：安装包内置默认 App（chatapp / file-explorer / file-viewer）。
    let bundled_apps = rest::apps::bundled_apps_dir();
    if let Err(error) = rest::apps::seed_default_apps(&apps_dir, bundled_apps.as_deref()) {
        eprintln!("[onion-server] 内置默认 App 种子安装失败: {error:?}");
    }
    eprintln!("[onion-server] apps_dir={}", apps_dir.display());
    let apps_dir = Arc::new(apps_dir);
    let bridge = Arc::new(bridge::Bridge::new(
        (*peers).clone(),
        own_url,
        config.a2a_token.clone(),
    ));
    let state = RouterState {
        registry: SessionRegistry::new(
            (*config).clone(),
            workspace_root,
            default_sessions_dir,
            bridge,
        ),
        agent_card: agent_card.clone(),
        config: config.clone(),
        token,
        a2a_token,
        started_at_ms: onion::session::epoch_millis(),
        cors: cors_layer(),
        instance_id: instance_id.clone(),
        peers,
        apps_dir,
    };
    // ---- stdio 协议模式：编辑器/宿主以子进程方式直连，不起 HTTP 监听。----
    if let Some(mode) = stdio_mode(&args) {
        match mode.as_str() {
            "acp" => {
                eprintln!("[onion-server] stdio mode: acp");
                if let Err(error) = acp::serve(state.registry.clone()).await {
                    eprintln!("[onion-server] acp stdio 退出: {error}");
                    std::process::exit(1);
                }
                return;
            }
            "sdk" => {
                eprintln!("[onion-server] stdio mode: sdk");
                if let Err(error) = sdk_wire::serve(state.registry.clone()).await {
                    eprintln!("[onion-server] sdk stdio 退出: {error}");
                    std::process::exit(1);
                }
                return;
            }
            other => {
                eprintln!("[onion-server] 未知 stdio 模式: {other}（支持: acp, sdk）");
                std::process::exit(2);
            }
        }
    }

    let app = router::build_router(state.clone());
    // 全局定时任务调度循环（10s tick：independent 物化 → drive_once 递送）。
    scheduler::spawn(state.registry.clone());
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(error) => {
            eprintln!("[onion-server] 监听失败 {addr}: {error}");
            std::process::exit(1);
        }
    };
    eprintln!("[onion-server] listening on http://{addr}");
    eprintln!("[onion-server] agent card: http://{addr}/.well-known/agent-card.json");
    eprintln!("[onion-server] a2a endpoint: http://{addr}/a2a");
    eprintln!("[onion-server] sse stream: http://{addr}/a2a/sse?method=message/stream");
    eprintln!("[onion-server] rest api: http://{addr}/v1/health");

    // ---- graceful shutdown：SIGINT/SIGTERM（Windows 仅 SIGINT / ctrl-c）。----
    let shutdown = async {
        let _ = tokio::signal::ctrl_c().await;
        eprintln!("[onion-server] shutdown signal received");
    };

    let server = axum::serve(listener, app).with_graceful_shutdown(shutdown);
    if let Err(error) = server.await {
        eprintln!("[onion-server] server error: {error}");
        std::process::exit(1);
    }
    eprintln!("[onion-server] bye");
}

/// stdio 协议模式：`--stdio <mode>` 优先，其次 `ONION_SERVER_STDIO`；空 = HTTP 服务。
fn stdio_mode(args: &[String]) -> Option<String> {
    args.iter()
        .position(|arg| arg == "--stdio")
        .and_then(|index| args.get(index + 1))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            std::env::var("ONION_SERVER_STDIO")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
}

/// 由 `ONION_SERVER_CORS_ORIGINS`（逗号分隔）构造 CORS 层；未配置返回 `None`。
fn cors_layer() -> Option<tower_http::cors::CorsLayer> {
    use tower_http::cors::{Any, CorsLayer};

    let raw = std::env::var("ONION_SERVER_CORS_ORIGINS").ok()?;
    let origins: Vec<axum::http::HeaderValue> = raw
        .split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .filter_map(|origin| origin.parse().ok())
        .collect();
    if origins.is_empty() {
        return None;
    }
    Some(
        CorsLayer::new()
            .allow_origin(origins)
            .allow_methods(Any)
            .allow_headers(Any),
    )
}

/// 静态对端实例表：`ONION_A2A_PEERS` 指向 JSON 文件（`[{"name","url"}]`）。
/// 缺失或解析失败一律空表（不兜底、不降级）。
fn peers_table() -> Vec<crate::rest::dto::PeerInfo> {
    let Some(path) = std::env::var("ONION_A2A_PEERS")
        .ok()
        .filter(|p| !p.trim().is_empty())
        .map(PathBuf::from)
    else {
        return Vec::new();
    };
    let raw = match std::fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(error) => {
            eprintln!("[onion-server] peers 表读取失败 {}: {error}", path.display());
            return Vec::new();
        }
    };
    match serde_json::from_str(&raw) {
        Ok(peers) => peers,
        Err(error) => {
            eprintln!("[onion-server] peers 表解析失败 {}: {error}", path.display());
            Vec::new()
        }
    }
}
