//! 能力枚举与资料库读面：health / meta / config / models / agents / tools /
//! skills / mcp / schedules / memory。
//!
//! 服务级端点（health / meta / config / models / agents）无会话依赖；会话级
//! 端点（tools / skills / mcp / schedules）需 `?session=<id>` 定位 live 会话的
//! 装配状态。

use std::collections::HashMap;
use std::sync::PoisonError;

use axum::extract::{Path, Query, State};
use axum::Json;

use crate::rest::dto::{
    AgentInfo, ConfigInfo, HealthInfo, McpListResponse, McpProbeResponse, McpReconcileResponse,
    McpResyncResponse, McpServerInfo, McpServerStateInfo, MemoryFileInfo, MemoryInfo,
    MemoryListResponse, MetaInfo, ModelInfo, ModelsInfo, PeersResponse, ProviderInfo,
    ScheduleListResponse, SkillInfo, SkillListResponse, ToolInfo, ToolListResponse,
};
use crate::rest::error::ApiError;
use crate::rest::require_session;
use crate::router::RouterState;

use onion::memory::{MemoryPaths, MemoryStore};
use onion::provider_catalog::ProviderCatalog;
use onion::tools::ToolRegistry;

/// REST 协议标识。
const PROTOCOL: &str = "onion-rest/v1";

// ---------------- 运维与元信息 ----------------

/// `GET /v1/health`（进程存活 + 版本 + 运行时长）。
pub(crate) async fn health(State(state): State<RouterState>) -> Json<HealthInfo> {
    Json(HealthInfo {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        uptime_ms: onion::session::epoch_millis().saturating_sub(state.started_at_ms),
        model: state.config.model.clone(),
    })
}

/// `GET /v1/meta`（工作区与会话落盘位置 + 能力清单 + 实例身份）。
pub(crate) async fn meta(State(state): State<RouterState>) -> Json<MetaInfo> {
    Json(MetaInfo {
        protocol: PROTOCOL,
        version: env!("CARGO_PKG_VERSION"),
        workspace: state.registry.workspace_root().display().to_string(),
        sessions_dir: state.registry.default_sessions_dir().display().to_string(),
        interactive_default: true,
        features: vec![
            "a2a",
            "sse",
            "ws",
            "approvals",
            "plan",
            "ask_user",
            "jobs",
            "memory",
            "schedules",
            "mcp",
        ],
        instance_id: state.instance_id.clone(),
    })
}

/// `GET /v1/peers`：对端实例表（`ONION_A2A_PEERS`，未配置返回空表）。
pub(crate) async fn peers(State(state): State<RouterState>) -> Json<PeersResponse> {
    Json(PeersResponse { peers: state.peers.as_ref().clone() })
}

/// `GET /v1/config`（脱敏：不含密钥）。
pub(crate) async fn config(State(state): State<RouterState>) -> Json<ConfigInfo> {
    Json(ConfigInfo {
        model: state.config.model.clone(),
        base_url: state.config.base_url.clone(),
        max_tokens: state.config.max_tokens,
        approval_preset: state.config.approval_preset.clone(),
        agent_preset: state.config.agent_preset.clone(),
    })
}

/// `GET /v1/models`（供应商目录；`apiKey` 不外泄）。
pub(crate) async fn models(State(state): State<RouterState>) -> Json<ModelsInfo> {
    let catalog = ProviderCatalog::load(&onion::paths::data_home(), &state.config.model);
    let providers = catalog
        .providers
        .into_iter()
        .map(|provider| ProviderInfo {
            id: provider.id,
            name: provider.name,
            base_url: provider.base_url,
            models: provider
                .models
                .into_iter()
                .map(|model| ModelInfo {
                    id: model.id,
                    name: model.name,
                    description: model.description,
                })
                .collect(),
        })
        .collect();
    Json(ModelsInfo { default_model: catalog.default_model, providers })
}

// ---------------- 能力枚举 ----------------

/// `GET /v1/agents`：`?session=` 优先用 live 会话目录，否则按当前工作区加载。
pub(crate) async fn agents(
    State(state): State<RouterState>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Vec<AgentInfo>>, ApiError> {
    let catalog = match query.get("session") {
        Some(id) => require_session(&state, id).await?.handle.agent_catalog.clone(),
        None => {
            let path = state.config.agents_dir.as_ref().map(|dir| dir.join("agents.yml"));
            onion::preset::AgentCatalog::merged(path.as_deref())
                .map_err(|error| ApiError::internal(error.to_string()))?
        }
    };
    let agents = catalog
        .ids()
        .iter()
        .filter_map(|id| catalog.get(id))
        .map(|agent| AgentInfo {
            id: agent.id.clone(),
            name: agent.name.clone(),
            tools: agent.tools.clone(),
        })
        .collect();
    Ok(Json(agents))
}

/// `GET /v1/tools?session=<id>`：当前 Agent 工具面的工具清单 + 风险等级。
pub(crate) async fn tools(
    State(state): State<RouterState>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<ToolListResponse>, ApiError> {
    let session = session_param(&state, &query).await?;
    let registry: &ToolRegistry = &session.handle.tools_handle;
    let risks = registry.risk_map();
    let agent_tools = registry.state().agent_tools();
    let tools = registry
        .definitions()
        .into_iter()
        .map(|definition| ToolInfo {
            risk: risk_label(risks.get(&definition.name)),
            in_agent: agent_tools.contains(&definition.name),
            name: definition.name,
            description: definition.description,
        })
        .collect();
    Ok(Json(ToolListResponse { tools }))
}

/// `GET /v1/skills?session=<id>`：会话工作区的技能目录（**含禁用技能**，仅
/// `enabled` 标记状态——管理界面需要看到全部）。
pub(crate) async fn skills(
    State(state): State<RouterState>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<SkillListResponse>, ApiError> {
    let session = session_param(&state, &query).await?;
    let working_dir = session.handle.session_cwd.current().as_ref().clone();
    let summaries = session
        .handle
        .skill_registry
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .list_all(Some(&working_dir));
    // 禁用集与 registry 同一契约来源（fail-open 读清单文件）。
    let disabled = disabled_names_for(&working_dir);
    let skills = summaries
        .iter()
        .map(|summary| SkillInfo::from_summary(summary, !disabled.contains(&summary.name)))
        .collect();
    Ok(Json(SkillListResponse { skills }))
}

/// 当前工作区生效的禁用名集合（skills-manager 清单；fail-open 空集）。
pub(crate) fn disabled_names_for(working_dir: &std::path::Path) -> std::collections::HashSet<String> {
    onion::skill::disable::manager_file_path(Some(working_dir))
        .map(|path| onion::skill::disable::read_disabled_names(&path))
        .unwrap_or_default()
}

/// `GET /v1/mcp/servers?session=<id>`：运行期 MCP server 与其桥接工具/状态。
///
/// 配置取自 bridge 的运行期快照（而非装配期 `handle.mcp_servers`），使 reconcile
/// 后的增删在状态视图上立即可见。
pub(crate) async fn mcp_servers(
    State(state): State<RouterState>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<McpListResponse>, ApiError> {
    let session = session_param(&state, &query).await?;
    let servers = session
        .handle
        .mcp_bridge
        .configs()
        .into_iter()
        .map(|server| {
            let name = server.server_name().to_string();
            // bridge 由同一份配置构造，view 必然存在；缺省 pending 仅作防御。
            let (state, tools) = match session.handle.mcp_bridge.view(&name) {
                Some(view) => (McpServerStateInfo::from_state(&view.state), view.tools),
                None => (
                    McpServerStateInfo {
                        kind: "pending".to_string(),
                        tool_count: None,
                        error: None,
                    },
                    Vec::new(),
                ),
            };
            match server {
                onion::mcp_resources::McpServerConfig::Stdio { command, args, .. } => McpServerInfo {
                    name,
                    transport: "stdio",
                    command: Some(command),
                    args: Some(args),
                    url: None,
                    state,
                    tools,
                },
                onion::mcp_resources::McpServerConfig::StreamableHttp { url, .. } => McpServerInfo {
                    name,
                    transport: "streamable-http",
                    command: None,
                    args: None,
                    url: Some(url),
                    state,
                    tools,
                },
            }
        })
        .collect();
    Ok(Json(McpListResponse { servers }))
}

/// `POST /v1/mcp/servers/{name}/probe?session=<id>`：独立一次性连接探活
/// （纯诊断，不改运行期状态），失败按 `connect` / `list` 阶段归因。
pub(crate) async fn mcp_probe(
    State(state): State<RouterState>,
    Path(name): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<McpProbeResponse>, ApiError> {
    let session = session_param(&state, &query).await?;
    let config = session
        .handle
        .mcp_bridge
        .configs()
        .into_iter()
        .find(|server| server.server_name() == name)
        .ok_or_else(|| mcp_server_not_found(&name))?;
    let result = tokio::task::spawn_blocking(move || onion::mcp_resources::probe(&config))
        .await
        .map_err(|error| ApiError::internal(format!("probe join failed: {error}")))?;
    Ok(Json(McpProbeResponse {
        ok: result.ok,
        stage: result.stage.as_ref().map(onion::mcp_resources::McpProbeStage::as_str),
        message: result.message,
        duration_ms: result.duration_ms,
        tool_names: result.tool_names,
    }))
}

/// `POST /v1/mcp/servers/{name}/resync?session=<id>`：重拉 `tools/list` 原子换装
/// （发现失败保持现网不动），同时承担 restart 语义——无实例可重建，
/// 重发现即等效重生。
pub(crate) async fn mcp_resync(
    State(state): State<RouterState>,
    Path(name): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<McpResyncResponse>, ApiError> {
    let session = session_param(&state, &query).await?;
    if !session
        .handle
        .mcp_bridge
        .configs()
        .iter()
        .any(|server| server.server_name() == name)
    {
        return Err(mcp_server_not_found(&name));
    }
    let bridge = session.handle.mcp_bridge.clone();
    let registry = session.handle.tools_handle.clone();
    let server = name.clone();
    let (removed, added) = tokio::task::spawn_blocking(move || bridge.resync(&server, &registry))
        .await
        .map_err(|error| ApiError::internal(format!("resync join failed: {error}")))?
        .map_err(ApiError::internal)?;
    Ok(Json(McpResyncResponse { server: name, removed, added }))
}

/// `POST /v1/mcp/reconcile?session=<id>`：重跑配置发现并把运行期实例集收敛到
/// 期望配置集（新增挂载 / 变更重建 / 消失卸载 / 未变保持）。
///
/// onion 无 cordis 配置变更事件源，故由管理面显式触发；发现输入与装配期同源
/// （`data_home/mcp.json` → `<workspace>/.onion/mcp.json` → `ONION_MCP_SERVERS`）。
pub(crate) async fn mcp_reconcile(
    State(state): State<RouterState>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<McpReconcileResponse>, ApiError> {
    let session = session_param(&state, &query).await?;
    let root = session.handle.session_cwd.root().to_path_buf();
    let explicit = std::env::var_os("ONION_MCP_SERVERS").map(std::path::PathBuf::from);
    let (desired, notes) = onion::mcp_resources::discover_servers(
        root.as_path(),
        &onion::paths::data_home(),
        explicit.as_deref(),
    );
    let bridge = session.handle.mcp_bridge.clone();
    let registry = session.handle.tools_handle.clone();
    let resources = session.handle.mcp_resources.clone();
    let report = tokio::task::spawn_blocking(move || {
        bridge.reconcile(desired, &registry, &resources)
    })
    .await
    .map_err(|error| ApiError::internal(format!("reconcile join failed: {error}")))?;
    let mut all_notes = notes;
    all_notes.extend(report.notes);
    Ok(Json(McpReconcileResponse {
        added: report.added,
        rebuilt: report.rebuilt,
        removed: report.removed,
        unchanged: report.unchanged,
        notes: all_notes,
    }))
}

/// MCP server 未配置（probe / resync 共用 404）。
fn mcp_server_not_found(name: &str) -> ApiError {
    ApiError::not_found("mcp_server_not_found", format!("unknown MCP server `{name}`"))
}

/// `GET /v1/schedules?session=<id>`：会话绑定的 active 提醒。
pub(crate) async fn schedules(
    State(state): State<RouterState>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<ScheduleListResponse>, ApiError> {
    let session = session_param(&state, &query).await?;
    let schedules = session.handle.schedule_service.list_views(&session.id);
    Ok(Json(ScheduleListResponse { schedules }))
}

// ---------------- 记忆资料库 ----------------

/// `GET /v1/memory`：记忆索引（`?session=` 时用该会话的权限根，否则兜底工作目录）。
pub(crate) async fn memory_list(
    State(state): State<RouterState>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<MemoryListResponse>, ApiError> {
    let store = memory_store(&state, &query).await?;
    let headers = store.list().map_err(|error| ApiError::internal(error.to_string()))?;
    let memories = headers
        .into_iter()
        .map(|header| MemoryInfo {
            filename: header.filename,
            description: header.description,
            memory_type: header.memory_type.map(|kind| kind.as_str().to_string()),
            mtime_ms: header.mtime_ms,
        })
        .collect();
    Ok(Json(MemoryListResponse { memories }))
}

/// `GET /v1/memory/{file}`：读取单条记忆全文（`?session=` 同上）。
pub(crate) async fn memory_get(
    State(state): State<RouterState>,
    Path(file): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<MemoryFileInfo>, ApiError> {
    let store = memory_store(&state, &query).await?;
    let memory = store
        .load(&file)
        .map_err(|error| ApiError::internal(error.to_string()))?
        .ok_or_else(|| {
            ApiError::not_found("memory_not_found", format!("memory `{file}` not found"))
        })?;
    Ok(Json(MemoryFileInfo {
        filename: memory.filename,
        name: memory.name,
        description: memory.description,
        memory_type: memory.memory_type.map(|kind| kind.as_str().to_string()),
        body: memory.body,
    }))
}

// ---------------- 内部助手 ----------------

/// 取 `?session=` 指向的 live 会话（缺失 / 不存在均为 400/404）。
pub(crate) async fn session_param(
    state: &RouterState,
    query: &HashMap<String, String>,
) -> Result<std::sync::Arc<crate::session::ServerSession>, ApiError> {
    let id = query.get("session").ok_or_else(|| {
        ApiError::bad_request(
            "session_required",
            "query parameter `session` is required for this endpoint",
        )
    })?;
    require_session(state, id).await
}

/// 记忆存储：`?session=` 指向 live 会话时用其权限根，否则用兜底工作目录
/// （与引擎记忆工具同一解析路径）。
async fn memory_store(
    state: &RouterState,
    query: &HashMap<String, String>,
) -> Result<MemoryStore, ApiError> {
    let root = match query.get("session") {
        Some(id) => require_session(state, id).await?.root.clone(),
        None => state.registry.workspace_root().clone(),
    };
    Ok(MemoryStore::new(MemoryPaths::resolve(&root)))
}

/// 工具风险等级 → 稳定标签。
fn risk_label(risk: Option<&onion::approval::ToolRisk>) -> &'static str {
    match risk {
        Some(onion::approval::ToolRisk::ReadOnly) => "read_only",
        Some(onion::approval::ToolRisk::WorkspaceWrite) => "workspace_write",
        Some(onion::approval::ToolRisk::Escalating) => "escalating",
        None => "unknown",
    }
}
