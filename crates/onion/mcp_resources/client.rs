//! MCP 客户端：连接外部 MCP server 并提供资源操作。
//!
//! - 两种 transport：`stdio`（spawn 子进程，child-process transport）与
//!   `streamable-http`（Streamable HTTP，JSON-RPC over HTTP）；
//! - server 由**配置文件**描述；
//! - 资源操作（`resources/list` / `resources/templates/list` / `resources/read`）与
//!   工具操作（`tools/list` / `tools/call`，供 `bridge.rs` 桥接）一并提供。
//!
//! 基于 [`rmcp`] SDK（对齐 ava-harness `mcp` 模块的传输层），复用其成熟的
//! 协议编解码、`initialize` 握手与 transport 实现。onion 为教学子集：每次操作
//! 建立一次独立连接（无长驻连接 / 无重连），与既有「每次调用 spawn-kill」语义一致，
//! 免去生命周期管理复杂度；连接在独立线程 + 独立 runtime 中执行，任何调用环境均可靠。

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use rmcp::model::{CallToolRequestParams, PaginatedRequestParams, ReadResourceRequestParams};
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
};
use rmcp::transport::TokioChildProcess;
use rmcp::{RoleClient, serve_client};
use serde_json::{json, Value};
use tokio::process::Command;

use super::seam::{McpResourceError, McpResourceRequest, McpResourceProvider};

/// 单次工具/资源请求超时（缺省 60000）。
const DEFAULT_TIMEOUT_MS: u64 = 60_000;

/// 一个 MCP server 的配置（stdio 或 streamable-http）。
///
/// `PartialEq` 供配置收敛（reconcile）判定「entry 是否变更」——
/// JSON 深比较的变更检测语义。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum McpServerConfig {
    /// child-process stdio transport。
    Stdio {
        /// 稳定本地命名空间（server 名）。
        server_name: String,
        /// 启动 server 的可执行文件。
        command: String,
        /// 直接传递的参数（不做 shell 插值）。
        args: Vec<String>,
        /// 附加到净化后环境之上的额外 env。
        env: std::collections::HashMap<String, String>,
        /// 子进程工作目录（空 = 继承当前）。
        cwd: Option<std::path::PathBuf>,
        /// 单次请求超时（毫秒）。
        tool_call_timeout_ms: u64,
    },
    /// Streamable HTTP transport（SSE）。
    StreamableHttp {
        /// 稳定本地命名空间（server 名）。
        server_name: String,
        /// MCP endpoint URL。
        url: String,
        /// 附加到请求的 headers。
        headers: std::collections::HashMap<String, String>,
        /// 单次请求超时（毫秒）。
        tool_call_timeout_ms: u64,
    },
}

impl McpServerConfig {
    /// server 名。
    pub fn server_name(&self) -> &str {
        match self {
            McpServerConfig::Stdio { server_name, .. }
            | McpServerConfig::StreamableHttp { server_name, .. } => server_name,
        }
    }

    /// 单次请求超时。
    pub fn timeout(&self) -> Duration {
        let ms = match self {
            McpServerConfig::Stdio { tool_call_timeout_ms, .. }
            | McpServerConfig::StreamableHttp { tool_call_timeout_ms, .. } => *tool_call_timeout_ms,
        };
        Duration::from_millis(ms.max(1))
    }

    /// transport 类型标识（`"stdio"` | `"streamable-http"`，供状态视图与 REST 呈现）。
    pub fn transport_kind(&self) -> &'static str {
        match self {
            McpServerConfig::Stdio { .. } => "stdio",
            McpServerConfig::StreamableHttp { .. } => "streamable-http",
        }
    }

    /// 展开 transport 字段中的 `${credential:NAME}` 引用：
    /// stdio 的 command/args/env/cwd 与 http 的
    /// url/headers 全部参与。连接前按次调用；未配置的引用即
    /// [`crate::credentials::TemplateError::Missing`]，语法非法即
    /// [`crate::credentials::TemplateError::Syntax`]（fail-loud）。
    pub fn resolve_credentials(
        self,
        store: &crate::credentials::CredentialStore,
    ) -> Result<Self, crate::credentials::TemplateError> {
        match self {
            Self::Stdio { server_name, command, args, env, cwd, tool_call_timeout_ms } => {
                let command = store.resolve_template(&command)?;
                let mut resolved_args = Vec::with_capacity(args.len());
                for arg in &args {
                    resolved_args.push(store.resolve_template(arg)?);
                }
                let mut resolved_env = std::collections::HashMap::with_capacity(env.len());
                for (key, value) in &env {
                    resolved_env.insert(key.clone(), store.resolve_template(value)?);
                }
                let cwd = cwd
                    .map(|dir| {
                        store.resolve_template(&dir.to_string_lossy()).map(std::path::PathBuf::from)
                    })
                    .transpose()?;
                Ok(Self::Stdio {
                    server_name,
                    command,
                    args: resolved_args,
                    env: resolved_env,
                    cwd,
                    tool_call_timeout_ms,
                })
            }
            Self::StreamableHttp { server_name, url, headers, tool_call_timeout_ms } => {
                let url = store.resolve_template(&url)?;
                let mut resolved_headers = std::collections::HashMap::with_capacity(headers.len());
                for (key, value) in &headers {
                    resolved_headers.insert(key.clone(), store.resolve_template(value)?);
                }
                Ok(Self::StreamableHttp {
                    server_name,
                    url,
                    headers: resolved_headers,
                    tool_call_timeout_ms,
                })
            }
        }
    }
}

/// 从 JSON 配置加载 MCP server 列表。
///
/// 配置形态：
/// ```json
/// {
///   "servers": [
///     { "transport": "stdio", "serverName": "mem", "command": "npx",
///       "args": ["-y", "@modelcontextprotocol/server-memory"], "env": {}, "cwd": "" },
///     { "transport": "streamable-http", "serverName": "remote", "url": "https://.../mcp",
///       "headers": {}, "toolCallTimeoutMs": 60000 }
///   ]
/// }
/// ```
pub fn load_servers(path: &std::path::Path) -> Result<Vec<McpServerConfig>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read MCP config {}: {e}", path.display()))?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|e| format!("invalid MCP config {}: {e}", path.display()))?;
    parse_servers(&value)
}

/// 解析 `{"servers": [...]}` 形态的 MCP server 配置。
///
/// 与 [`load_servers`] 同口径，供不经文件的外部来源（如 App 贡献的内联配置）复用。
pub fn parse_servers(value: &Value) -> Result<Vec<McpServerConfig>, String> {
    let servers = value
        .get("servers")
        .and_then(Value::as_array)
        .ok_or_else(|| "MCP config must contain a \"servers\" array".to_string())?;
    servers.iter().map(parse_server).collect()
}

fn parse_server(v: &Value) -> Result<McpServerConfig, String> {
    let transport = v
        .get("transport")
        .and_then(Value::as_str)
        .ok_or_else(|| "each MCP server must have a \"transport\" field".to_string())?;
    let server_name = v
        .get("serverName")
        .and_then(Value::as_str)
        .ok_or_else(|| "each MCP server must have a \"serverName\" field".to_string())?
        .to_string();
    let timeout = v
        .get("toolCallTimeoutMs")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_TIMEOUT_MS);
    match transport {
        "stdio" => {
            let command = v
                .get("command")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("stdio server \"{server_name}\" needs a \"command\""))?
                .to_string();
            let args = v
                .get("args")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let env = v
                .get("env")
                .and_then(Value::as_object)
                .map(|m| {
                    m.iter()
                        .filter_map(|(k, val)| val.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect()
                })
                .unwrap_or_default();
            let cwd = v
                .get("cwd")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(std::path::PathBuf::from);
            Ok(McpServerConfig::Stdio { server_name, command, args, env, cwd, tool_call_timeout_ms: timeout })
        }
        "streamable-http" => {
            let url = v
                .get("url")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("streamable-http server \"{server_name}\" needs a \"url\""))?
                .to_string();
            let headers = v
                .get("headers")
                .and_then(Value::as_object)
                .map(|m| {
                    m.iter()
                        .filter_map(|(k, val)| val.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect()
                })
                .unwrap_or_default();
            Ok(McpServerConfig::StreamableHttp { server_name, url, headers, tool_call_timeout_ms: timeout })
        }
        other => Err(format!("unsupported MCP transport \"{other}\"")),
    }
}

/// 由配置构造的连接 provider（连接拥有者）。
pub struct McpClientProvider {
    config: McpServerConfig,
    next_id: AtomicU64,
}

impl McpClientProvider {
    pub fn new(config: McpServerConfig) -> Self {
        Self { config, next_id: AtomicU64::new(1) }
    }

    /// 对任意 MCP method 发起一次 JSON-RPC 请求（供工具桥接 `tools/list` / `tools/call`
    /// 复用；资源操作经 [`McpResourceProvider::request`] 走 [`McpResourceRequest`]）。
    ///
    /// 内部在独立线程 + 独立 runtime 中建立一次性连接并执行，返回无损 JSON 结果。
    pub fn request_method(&self, method: &str, params: Value) -> Result<Value, McpResourceError> {
        let _ = self.next_id.fetch_add(1, Ordering::SeqCst); // 保留自增 id 语义
        let config = self.config.clone();
        let timeout = self.config.timeout();
        let method = method.to_string();
        with_connection(&config, timeout, move |running| {
            // `running`（RunningService）必须移入 future 保持存活直到请求完成；
            // 若在闭包内 clone peer 后丢弃 running，连接会在请求前被关闭（Transport closed）。
            async move {
                let peer = (*running).clone();
                match method.as_str() {
                    "resources/list" => {
                        let cursor = params.get("cursor").and_then(Value::as_str).map(str::to_string);
                        let result = peer
                            .list_resources(Some(PaginatedRequestParams::default().with_cursor(cursor)))
                            .await
                            .map_err(|e| McpResourceError::failed(format!("MCP request failed: {e}")))?;
                        serde_json::to_value(&result)
                            .map_err(|e| McpResourceError::malformed(format!("serialize resources/list: {e}")))
                    }
                    "resources/templates/list" => {
                        let cursor = params.get("cursor").and_then(Value::as_str).map(str::to_string);
                        let result = peer
                            .list_resource_templates(Some(
                                PaginatedRequestParams::default().with_cursor(cursor),
                            ))
                            .await
                            .map_err(|e| McpResourceError::failed(format!("MCP request failed: {e}")))?;
                        serde_json::to_value(&result)
                            .map_err(|e| McpResourceError::malformed(format!("serialize templates/list: {e}")))
                    }
                    "resources/read" => {
                        let uri = params.get("uri").and_then(Value::as_str).unwrap_or("").to_string();
                        let result = peer
                            .read_resource(ReadResourceRequestParams::new(uri))
                            .await
                            .map_err(|e| McpResourceError::failed(format!("MCP request failed: {e}")))?;
                        serde_json::to_value(&result)
                            .map_err(|e| McpResourceError::malformed(format!("serialize resources/read: {e}")))
                    }
                    "tools/list" => {
                        let result = peer
                            .list_tools(None)
                            .await
                            .map_err(|e| McpResourceError::failed(format!("MCP request failed: {e}")))?;
                        serde_json::to_value(&result)
                            .map_err(|e| McpResourceError::malformed(format!("serialize tools/list: {e}")))
                    }
                    "tools/call" => {
                        let name = params.get("name").and_then(Value::as_str).unwrap_or("").to_string();
                        let arguments = params
                            .get("arguments")
                            .and_then(Value::as_object)
                            .cloned()
                            .unwrap_or_default();
                        let result = peer
                            .call_tool(CallToolRequestParams::new(name).with_arguments(arguments))
                            .await
                            .map_err(|e| McpResourceError::failed(format!("MCP request failed: {e}")))?;
                        serde_json::to_value(&result)
                            .map_err(|e| McpResourceError::malformed(format!("serialize tools/call: {e}")))
                    }
                    other => Err(McpResourceError::failed(format!("unsupported MCP method \"{other}\""))),
                }
            }
        })
    }

    /// 列出 server 暴露的工具（`tools/list`）。
    pub fn list_tools(&self) -> Result<Value, McpResourceError> {
        self.request_method("tools/list", json!({}))
    }

    /// 调用 server 的一个工具（`tools/call`）。
    pub fn call_tool(&self, name: &str, arguments: Value) -> Result<Value, McpResourceError> {
        let params = json!({ "name": name, "arguments": arguments });
        self.request_method("tools/call", params)
    }

    /// server 名。
    pub fn server_name(&self) -> &str {
        self.config.server_name()
    }

    /// 装配配置（管理面状态视图 / probe 复用，避免暴露私有字段）。
    pub fn config(&self) -> &McpServerConfig {
        &self.config
    }

    /// probe 专用 `tools/list`：双层 Result 按阶段归因——外层 `Err` = connect 阶段
    /// （凭据解析 / spawn / initialize 握手失败），内层 `Err` = list 阶段（连接成功
    /// 但 `tools/list` 失败），`Ok(Ok(names))` = 探活成功。普通
    /// [`Self::request_method`] 把两阶段折叠成单层 Result 无法归因，故 probe 单独
    /// 复用 [`with_connection`] 通道。
    pub(crate) fn probe_tools(&self) -> Result<Result<Vec<String>, String>, McpResourceError> {
        let config = self.config.clone();
        let timeout = self.config.timeout();
        with_connection(&config, timeout, |running| async move {
            // `running` 移入 future 保持存活直到请求完成（Transport closed 规避，同上）。
            // 内层 String 错误留在内层（list 阶段），不使用 `?` 上抛——`?` 会把
            // String 转成外层 McpResourceError，破坏 connect / list 阶段归因。
            let peer = (*running).clone();
            let inner = match peer.list_tools(None).await {
                Err(e) => Err(format!("MCP request failed: {e}")),
                Ok(result) => serde_json::to_value(&result)
                    .map_err(|e| format!("serialize tools/list: {e}"))
                    .and_then(|value| {
                        value
                            .get("tools")
                            .and_then(Value::as_array)
                            .ok_or_else(|| "tools/list 响应缺 tools 数组".to_string())
                            .map(|list| {
                                list.iter()
                                    .filter_map(|tool| {
                                        tool.get("name").and_then(Value::as_str).map(str::to_string)
                                    })
                                    .collect()
                            })
                    }),
            };
            Ok(inner)
        })
    }
}

impl McpResourceProvider for McpClientProvider {
    fn request(&self, request: &McpResourceRequest) -> Result<Value, McpResourceError> {
        let (method, params) = match request {
            McpResourceRequest::List { cursor } => {
                let mut p = serde_json::Map::new();
                if let Some(c) = cursor {
                    p.insert("cursor".to_string(), json!(c));
                }
                ("resources/list", Value::Object(p))
            }
            McpResourceRequest::ListTemplates { cursor } => {
                let mut p = serde_json::Map::new();
                if let Some(c) = cursor {
                    p.insert("cursor".to_string(), json!(c));
                }
                ("resources/templates/list", Value::Object(p))
            }
            McpResourceRequest::Read { uri } => ("resources/read", json!({ "uri": uri })),
        };
        self.request_method(method, params)
    }
}

/// 建立一次性连接并在同一 runtime、同一连接下执行异步闭包（对齐 ava-harness `MCPManager`）。
///
/// stdio / streamable-http 两种 transport 统一入口；连接在独立线程 + 独立 runtime 中
/// 建立并保持存活，直到闭包完成（`RunningService` 不 drop，连接不提前关闭）。
/// 同步 trait 方法（`McpResourceProvider::request` / `request_method`）无法直接驱动
/// async rmcp 调用，且工具 execute 运行在 tokio runtime 内，`Handle::block_on` 会 panic；
/// 故一律在独立线程 + 独立 runtime 执行，任何调用环境均可靠（教学子集）。
/// crate 内可见：管理面 probe（`manager.rs`）需按阶段归因，直接复用同一连接通道。
pub(crate) fn with_connection<T, F, Fut>(
    config: &McpServerConfig,
    _timeout: Duration,
    f: F,
) -> Result<T, McpResourceError>
where
    T: Send + 'static,
    F: FnOnce(rmcp::service::RunningService<RoleClient, ()>) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<T, McpResourceError>> + Send + 'static,
{
    // 凭据模板在 spawn 前按次解析（fail-fast）：存储配置保持引用原样，
    // 每次连接重读凭据文件（改密钥无需重启，按次解析契约）。
    let config = config
        .clone()
        .resolve_credentials(&crate::credentials::CredentialStore::new(&crate::paths::data_home()))
        .map_err(|err| {
            McpResourceError::failed(format!(
                "MCP server \"{}\" 凭据模板解析失败：{err}",
                config.server_name()
            ))
        })?;
    let handle = std::thread::Builder::new()
        .name("onion-mcp-client".to_string())
        .spawn(move || -> Result<T, McpResourceError> {
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| McpResourceError::failed(format!("MCP runtime init failed: {e}")))?;
            rt.block_on(async {
                let running = connect(&config).await?;
                let fut = f(running);
                fut.await
            })
        })
        .map_err(|e| McpResourceError::failed(format!("MCP client thread spawn failed: {e}")))?;
    handle
        .join()
        .map_err(|_| McpResourceError::failed("MCP client worker thread panicked".to_string()))?
}

/// 建立到单个 server 的连接，返回保持连接存活的 `RunningService`（对齐 ava-harness `Conn._running`）。
async fn connect(
    config: &McpServerConfig,
) -> Result<rmcp::service::RunningService<RoleClient, ()>, McpResourceError> {
    match config {
        McpServerConfig::Stdio { command, args, env, cwd, .. } => {
            let mut cmd = build_stdio_command(command);
            cmd.args(args);
            if let Some(dir) = cwd {
                cmd.current_dir(dir);
            }
            for (k, v) in env {
                cmd.env(k, v);
            }
            let transport = TokioChildProcess::new(cmd)
                .map_err(|e| McpResourceError::failed(format!("failed to spawn MCP process: {e}")))?;
            serve_client((), transport).await.map_err(|e| {
                McpResourceError::failed(format!("MCP stdio initialize failed: {e}"))
            })
        }
        McpServerConfig::StreamableHttp { url, headers, .. } => {
            let mut config = StreamableHttpClientTransportConfig::with_uri(url.as_str());
            if !headers.is_empty() {
                let mut h = std::collections::HashMap::new();
                for (k, v) in headers {
                    let name = http::HeaderName::from_bytes(k.as_bytes()).map_err(|e| {
                        McpResourceError::failed(format!("invalid header name '{k}': {e}"))
                    })?;
                    let value = http::HeaderValue::from_str(v).map_err(|e| {
                        McpResourceError::failed(format!("invalid header value for '{k}': {e}"))
                    })?;
                    h.insert(name, value);
                }
                config = config.custom_headers(h);
            }
            let transport = StreamableHttpClientTransport::from_config(config);
            serve_client((), transport).await.map_err(|e| {
                McpResourceError::failed(format!("MCP http initialize failed: {e}"))
            })
        }
    }
}

/// 构建 stdio 子进程命令，处理 Windows 下 `.cmd`/`.bat` 脚本的兼容问题。
///
/// Windows 上 `npx`/`npm` 等命令实为 `.cmd` 批处理脚本，`tokio::process::Command`
/// 默认只查找 `.exe`，直接 spawn 会报 `program not found`。这里在 Windows 下将
/// 非可执行文件的命令交给 `cmd /C` 执行；Linux/macOS 直接使用原命令。
fn build_stdio_command(command: &str) -> Command {
    #[cfg(windows)]
    {
        let has_ext = std::path::Path::new(command)
            .extension()
            .map(|e| !e.is_empty())
            .unwrap_or(false);
        if !has_ext {
            let mut cmd = Command::new("cmd");
            cmd.arg("/C").arg(command);
            return cmd;
        }
    }
    Command::new(command)
}

/// 自动发现 MCP 配置（装配期统一入口）。
///
/// 候选路径（优先级从低到高，同名 `serverName` 后者覆盖前者，对齐 ava
/// 「工作区 MCP 覆盖」语义）：
/// 1. 全局：`<ONION_HOME>/mcp.json`（程序数据文件夹，[`data_home`]）；
/// 2. 工作空间：`<workspace>/.onion/mcp.json`（显式提交物，对齐 ava `.ava/mcp.json`）；
/// 3. env 显式路径：`ONION_MCP_SERVERS`（最高优先，保留旧入口）。
///
/// 文件不存在 → 静默跳过（自动发现正常态）；格式错误 → 错误进 notes 呈现
/// （装配层不 panic，对齐 front door 呈现类信息通道）。
pub fn discover_servers(
    root: &std::path::Path,
    data_home: &std::path::Path,
    explicit: Option<&std::path::Path>,
) -> (Vec<McpServerConfig>, Vec<String>) {
    let mut servers = Vec::new();
    let mut notes = Vec::new();
    let mut candidates: Vec<std::path::PathBuf> = vec![
        data_home.join("mcp.json"),
        root.join(".onion").join("mcp.json"),
    ];
    if let Some(explicit) = explicit {
        candidates.push(explicit.to_path_buf());
    }
    let mut seen: Vec<std::path::PathBuf> = Vec::new();
    for path in &candidates {
        if !path.exists() || seen.iter().any(|p| p == path) {
            continue; // 不存在 → 静默跳过（自动发现正常态）；重复路径只加载一次。
        }
        seen.push(path.clone());
        match load_servers(path) {
            Ok(found) => {
                // 同名 serverName：后者覆盖前者（工作区覆盖全局）。
                for server in found {
                    let name = server.server_name().to_string();
                    if let Some(pos) = servers
                        .iter()
                        .position(|s: &McpServerConfig| s.server_name() == name)
                    {
                        notes.push(format!(
                            "MCP 配置覆盖：server \"{name}\" 以 {} 为准",
                            path.display()
                        ));
                        servers[pos] = server;
                    } else {
                        servers.push(server);
                    }
                }
            }
            Err(e) => notes.push(format!("MCP 配置文件加载失败：{e}")),
        }
    }
    (servers, notes)
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
