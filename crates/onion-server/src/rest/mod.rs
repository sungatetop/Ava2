//! `/v1` RESTful API：路由装配 + 会话定位助手。
//!
//! 契约面向开发者脚本与未来 web 前端（与 A2A 面并存、共用同一
//! `SessionRegistry` / `Runtime`）。会话事件订阅双通道：SSE
//! （`/v1/sessions/{id}/events`）与 WS（`/v1/sessions/{id}/ws`）。
//! 所有端点由 [`crate::rest::error::ApiError`] 统一错误模型返回。

pub(crate) mod error;

pub(crate) mod apps;
pub(crate) mod attachments;
mod capabilities;
mod commands;
mod control;
mod deliverables;
pub(crate) mod dto;
mod events;
mod files;
mod fsdirs;
mod inputs;
mod models;
mod net;
pub(crate) mod schedules;
mod sessions;
mod skill_hub;
mod skills_manager;
mod ws;

use std::sync::Arc;

use axum::routing::{get, post, put};

use crate::router::RouterState;
use crate::session::ServerSession;

use error::ApiError;

/// `/v1` 路由表（state 由上层 `build_router` 注入）。
pub(crate) fn build_rest_router() -> axum::Router<RouterState> {
    axum::Router::new()
        // 运维与元信息
        .route("/v1/health", get(capabilities::health))
        .route("/v1/meta", get(capabilities::meta))
        .route("/v1/peers", get(capabilities::peers))
        .route("/v1/config", get(capabilities::config))
        // 模型管理：目录读面 + 供应商/模型写面 + 默认模型 + 连接测试
        .route("/v1/models", get(capabilities::models).post(models::add_model))
        .route(
            "/v1/models/default",
            put(models::set_default),
        )
        .route("/v1/models/test", post(models::test_model))
        .route(
            "/v1/models/{provider}/{model}",
            axum::routing::delete(models::remove_model),
        )
        .route("/v1/providers", post(models::add_provider))
        .route(
            "/v1/providers/{id}",
            axum::routing::delete(models::remove_provider),
        )
        // 定时任务管理（全局任务面）：列表/创建/编辑·启停/删除/立即执行
        .route("/v1/tasks", get(schedules::list_tasks).post(schedules::create_task))
        .route(
            "/v1/tasks/{id}",
            axum::routing::patch(schedules::update_task).delete(schedules::delete_task),
        )
        .route("/v1/tasks/{id}/run", post(schedules::run_task))
        // 附件上传（内容寻址 durable 存储）
        .route("/v1/attachments/images", post(attachments::upload_images))
        .route("/v1/attachments/files", post(attachments::upload_file))
        // 会话生命周期
        .route("/v1/sessions", get(sessions::list).post(sessions::create))
        .route(
            "/v1/sessions/{id}",
            get(sessions::get).delete(sessions::remove),
        )
        .route(
            "/v1/sessions/{id}/messages",
            get(sessions::messages).post(sessions::post_message),
        )
        .route("/v1/sessions/{id}/events", get(events::stream))
        .route("/v1/sessions/{id}/ws", get(ws::subscribe))
        .route("/v1/sessions/{id}/interrupt", post(sessions::interrupt))
        .route("/v1/sessions/{id}/compact", post(sessions::compact))
        .route("/v1/sessions/{id}/fork", post(sessions::fork))
        .route("/v1/sessions/{id}/commands", post(commands::run))
        .route("/v1/sessions/{id}/todos", get(sessions::todos))
        .route("/v1/sessions/{id}/deliverables", get(deliverables::list))
        // 人工输入回填
        .route("/v1/sessions/{id}/pending", get(inputs::pending))
        .route(
            "/v1/sessions/{id}/approvals/{requestId}",
            post(inputs::approval),
        )
        .route(
            "/v1/sessions/{id}/plan-reviews/{requestId}",
            post(inputs::plan_review),
        )
        .route(
            "/v1/sessions/{id}/questions/{requestId}",
            post(inputs::question),
        )
        // 会话控制面
        .route(
            "/v1/sessions/{id}/agent",
            get(control::agent).put(control::set_agent),
        )
        .route("/v1/sessions/{id}/model", put(control::set_model))
        .route("/v1/sessions/{id}/approval", put(control::set_approval))
        .route(
            "/v1/sessions/{id}/goal",
            get(control::goal).put(control::set_goal),
        )
        .route("/v1/sessions/{id}/goal/actions", post(control::goal_action))
        .route(
            "/v1/sessions/{id}/plan",
            get(control::plan).post(control::set_plan),
        )
        .route("/v1/sessions/{id}/tasks", get(control::tasks))
        .route("/v1/sessions/{id}/tasks/{taskId}", get(control::task))
        .route(
            "/v1/sessions/{id}/tasks/{taskId}/cancel",
            post(control::cancel_task),
        )
        .route("/v1/sessions/{id}/jobs", get(control::jobs))
        .route("/v1/sessions/{id}/jobs/{jobId}", get(control::job))
        .route(
            "/v1/sessions/{id}/jobs/{jobId}/kill",
            post(control::kill_job),
        )
        // 能力枚举与资料库
        .route("/v1/agents", get(capabilities::agents))
        .route("/v1/tools", get(capabilities::tools))
        .route("/v1/skills", get(capabilities::skills))
        .route(
            "/v1/skills-manager",
            get(skills_manager::get_skill_manager).put(skills_manager::put_skill_manager),
        )
        // AppHost 统一联网服务（受控外联转发：POST 通用形态 + GET 只读便捷形态）
        .route("/v1/net/fetch", get(net::fetch).post(net::fetch_request))
        // 技能广场（腾讯 SkillHub）：安装到项目 .onion/skills（读面由 App 经
        // /v1/net/fetch 直连上游）
        .route("/v1/skill-hub/install", post(skill_hub::install))
        .route("/v1/mcp/servers", get(capabilities::mcp_servers))
        .route("/v1/mcp/servers/{name}/probe", post(capabilities::mcp_probe))
        .route("/v1/mcp/servers/{name}/resync", post(capabilities::mcp_resync))
        .route("/v1/mcp/reconcile", post(capabilities::mcp_reconcile))
        .route("/v1/schedules", get(capabilities::schedules))
        .route("/v1/memory", get(capabilities::memory_list))
        .route("/v1/memory/{file}", get(capabilities::memory_get))
        // 工作区只读文件（交付物卡片 / 文件提及预览）
        .route("/v1/workspace/file", get(files::read))
        // 目录枚举（工作区选择器的目录浏览数据源）
        .route("/v1/fs/dirs", get(fsdirs::dirs))
        // AppHost App 仓库：安装 / 卸载 / 启停 / 静态产物
        .route("/v1/apps", get(apps::list))
        .route("/v1/apps/install", post(apps::install))
        .route(
            "/v1/apps/{id}",
            get(apps::manifest).delete(apps::uninstall),
        )
        .route("/v1/apps/{id}/enabled", put(apps::set_enabled))
        .route("/v1/apps/{id}/{*path}", get(apps::asset))
}

/// 按 `id` 定位进程内会话：未在内存但已落盘的历史会话按需恢复（回放日志），
/// 两者皆无 → 404 `session_not_found`。
pub(crate) async fn require_session(
    state: &RouterState,
    id: &str,
) -> Result<Arc<ServerSession>, ApiError> {
    match state.registry.open(id, true).await {
        Ok(Some(session)) => Ok(session),
        Ok(None) => Err(ApiError::session_not_found(id)),
        Err(error) => Err(ApiError::internal(error.to_string())),
    }
}
