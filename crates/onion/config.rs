//! 静态装配：模型路由与运行参数。

use std::time::Duration;

use crate::agent_team::config::TeamConfig;
use crate::compaction::config::ModelPolicy;

/// 内核运行配置（一次性装配，运行期不变）。
#[derive(Clone, Debug)]
pub struct Config {
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    pub max_tokens: u32,
    pub temperature: Option<f32>,
    /// 同 step 内请求错误重试上限。
    pub max_retries: u32,
    /// 单次工具执行超时。
    pub tool_timeout: Duration,
    /// 同 step 内工具调用的并行度上限（默认 10；
    /// `1` 保持串行）。事件序仍按模型顺序提交，仅执行重叠。
    pub max_parallel_tools: usize,
    /// 是否包含固定 harness 身份开场白。
    pub include_identity: bool,
    /// 是否包含可选运行时上下文快照（required 上下文始终保留）。
    pub include_runtime_context: bool,
    /// persona 前缀模板（首方指导之前；支持 {{variable}} 严格插值）。
    pub persona_prefix: String,
    /// persona 后缀模板（首方指导之后；支持 {{variable}} 严格插值）。
    pub persona_suffix: String,
    /// 面向模型的工具显式顺序（含一次 `<unlisted-tools>` rest 标记；缺省字典序）。
    pub tool_order: Option<Vec<String>>,
    /// 模型上下文窗口 token 容量（compaction 压力评估基准）。
    pub context_window_tokens: u64,
    /// 压缩触发阈值比例，默认 0.8。
    pub compact_threshold_ratio: f32,
    /// 压缩尾部保留比例，默认 0.16。
    pub compact_retain_ratio: f32,
    /// 压缩重试上限。
    pub compact_max_retries: u32,
    /// 请求预留输出 token（即 max_tokens）。
    pub reserved_completion_tokens: u64,
    /// 压缩 headroom，默认 65536。
    pub compact_headroom_tokens: u64,
    /// per-model 压缩策略覆写（按 model 精确匹配，
    /// 缺省字段回退全局 compact_* 配置，见 `resolve_compact_spec_for`）。
    pub compact_model_policies: Vec<ModelPolicy>,
    /// 是否启用工作空间指令注入（AGENTS.md 基线 + 触点对账）。
    pub workspace_enabled: bool,
    /// 工作空间指令预算（默认 65536；不足时先省略更宽泛文件）。
    pub workspace_max_bytes: usize,
    /// 初始工具审批预设名（默认 `workspace-write`；运行中可
    /// 经 `/permission` 切换）。`auto` 预设附带 `HeuristicReviewer`。
    pub approval_preset: String,
    /// todo 部署策略：是否允许多个 `in_progress`。
    /// true 适合并行执行的 agent；false 恢复单一 active 纪律（调用标记多个被拒）。
    pub allow_parallel_in_progress: bool,
    /// web_search 搜索 provider 端点（Anthropic-compatible Messages 接口；空 = 未配置，
    /// 工具仍注册但执行期结构化失败，即「工具可见但 provider 不可用」）。
    pub web_search_base_url: String,
    /// web_search 搜索 provider 模型名（`/messages` 请求用；省略时回退主模型）。
    pub web_search_model: String,
    /// 单次 `web_search` 工具调用的 source 上限（缺省 8）。
    pub web_search_max_results: usize,
    /// 单次 `web_search` 接受的最大 query 数（缺省 4）。
    pub web_search_max_queries: usize,
    /// `web_fetch` 单次输出字符上限（缺省 200000）。
    pub web_fetch_max_output_chars: usize,
    /// session_* 单次搜索返回的最大命中数（缺省 100）。
    pub session_query_max_results: usize,
    /// `present` 单次调用声明的文件数上限（缺省 8）。
    pub present_max_files: usize,
    /// 是否允许 `terminal_send` 的 `run_in_background` 后台化（缺省 true）。
    pub terminal_enable_background: bool,
    /// `terminal_*` 渲染结果字符上限（下限 64）。
    pub terminal_max_result_chars: usize,
    /// 可插拔 OS 级沙箱 exe 绝对路径（`onion-sandbox`）。None = 未配置，
    /// shell 执行回退直通（无 OS 强制），工具层将 shell 档位一并收敛为
    /// `Escalating` 走显式审批兜底（fail-closed）；配置后经独立 exe 执行时强制隔离
    /// （执行时 OS 拒绝，而非命令词静态分类）。
    /// 解析链：`ONION_SANDBOX_EXE` 显式指定优先，否则自动识别本程序同目录的
    /// onion-sandbox（安装器/便携目录同布局），再缺省则直通。
    pub sandbox_executable: Option<std::path::PathBuf>,
    /// MCP server 配置文件路径（JSON，含 `servers` 数组）。None = 未配置，mcp_resource 工具仍注册但执行期结构化失败。
    pub mcp_servers_config: Option<std::path::PathBuf>,
    /// workspace 依赖负载目录（携带 `runtime.json` + `dependencies/`，作为依赖型
    /// 工具的 `source`）。None = 未配置，工具仍注册但执行期
    /// 结构化失败（无法解析负载路径）。
    pub workspace_deps_source: Option<std::path::PathBuf>,
    /// 当前 Agent id（默认 `coding`；工具面按 Agent 工具名集合过滤，见 `preset.rs`）。
    pub agent_preset: String,
    /// 自定义 Agent 定义 YAML 文件路径（`agents.yml`；None = 仅内置双预设）。
    pub agents_dir: Option<std::path::PathBuf>,
    /// 子代理显式模型选择白名单（`ONION_SUBAGENT_MODEL_SELECTION`，JSON 数组
    /// `[{"provider":"local","model":"..."}]`）。
    ///
    /// None = 未启用：会话不写 `subagent/model-selection-policy` 事件，子代理
    /// 显式选择只允许父会话当前路由（缺省关闭，显式 opt-in 才开启）。
    pub subagent_model_selection: Option<Vec<crate::subagent::model_selection::AllowedModelRoute>>,
    /// agent-team 配置（`ONION_AGENT_TEAM=1` 启用；None = 未启用，subagent
    /// 控制三件保持原语义，agent-team 独立装配、不与 subagent 共享状态）。
    pub agent_team: Option<TeamConfig>,
    /// A2A mesh 令牌（`ONION_A2A_TOKEN`）：出站 `a2a_send` 携此令牌访问对端
    /// `/a2a`，与运维面令牌（`ONION_SERVER_TOKEN`）分属两个信任域。None = 不携带。
    pub a2a_token: Option<String>,
}

impl Config {
    /// 环境变量覆盖（SMOKE_LLM_* / SMOKE_COMPACT_*），缺省 local 供应商冒烟约定。
    /// compact 参数暴露覆盖点：短对话冒烟需要缩小的阈值/保留预算。
    pub fn from_env() -> Self {
        Self {
            model: env_or("SMOKE_LLM_MODEL", "Qwen3.8-27B"),
            base_url: default_base_url(),
            api_key: default_api_key(),
            // 输出预留缺省 32768：旧默认 2048 在开思考模型（如 Qwen3.8-27B）上会被
            // thinking 整段烧光，正文/工具参数截断，以 Length 静默收场。
            max_tokens: env_parse("SMOKE_LLM_MAX_TOKENS", 32 * 1024),
            temperature: None,
            // 无内建 step 预算：防失控
            // 责任在宿主/策略侧，经 turn-stopping 钩子 cancel 收轮。
            max_retries: 1,
            tool_timeout: Duration::from_secs(60),
            max_parallel_tools: env_parse("ONION_MAX_PARALLEL_TOOLS", 10),
            include_identity: true,
            include_runtime_context: true,
            persona_prefix: "You are the onion assistant; follow user instructions faithfully."
                .to_string(),
            persona_suffix: String::new(),
            tool_order: None,
            context_window_tokens: env_parse("SMOKE_LLM_CONTEXT_WINDOW", 128_000),
            compact_threshold_ratio: env_parse("SMOKE_COMPACT_THRESHOLD_RATIO", 0.8),
            compact_retain_ratio: env_parse("SMOKE_COMPACT_RETAIN_RATIO", 0.16),
            compact_max_retries: env_parse("SMOKE_COMPACT_MAX_RETRIES", 1),
            reserved_completion_tokens: env_parse("SMOKE_LLM_RESERVED_COMPLETION", 2048),
            compact_headroom_tokens: env_parse("SMOKE_COMPACT_HEADROOM_TOKENS", 65_536),
            // per-model 压缩策略：缺省空表（全部走全局 compact_* 配置）。
            compact_model_policies: Vec::new(),
            workspace_enabled: env_parse("ONION_WORKSPACE_ENABLED", true),
            workspace_max_bytes: env_parse("ONION_WORKSPACE_MAX_BYTES", 65_536),
            approval_preset: env_or("ONION_PRESET", "workspace-write"),
            allow_parallel_in_progress: env_parse("ONION_ALLOW_PARALLEL_IN_PROGRESS", false),
            // web 工具：搜索 provider 缺省未配置（空端点 → 执行期结构化失败，
            // 工具可见但 provider 不可用）；fetch 无需密钥开箱即用。
            web_search_base_url: env_or("ONION_WEB_SEARCH_BASE_URL", ""),
            web_search_model: env_or("ONION_WEB_SEARCH_MODEL", ""),
            web_search_max_results: env_parse("ONION_WEB_SEARCH_MAX_RESULTS", 8),
            web_search_max_queries: env_parse("ONION_WEB_SEARCH_MAX_QUERIES", 4),
            web_fetch_max_output_chars: env_parse("ONION_WEB_FETCH_MAX_OUTPUT_CHARS", 200_000),
            session_query_max_results: env_parse("ONION_SESSION_QUERY_MAX_RESULTS", 100),
            present_max_files: env_parse("ONION_PRESENT_MAX_FILES", 8),
            terminal_enable_background: env_parse("ONION_TERMINAL_ENABLE_BACKGROUND", true),
            terminal_max_result_chars: env_parse("ONION_TERMINAL_MAX_RESULT_CHARS", 32_768),
            // 沙箱 exe：`ONION_SANDBOX_EXE` 显式指定优先；否则自动识别本程序
            // 同目录的 onion-sandbox（安装器/便携目录同布局开箱即用）；都缺省直通。
            sandbox_executable: std::env::var("ONION_SANDBOX_EXE")
                .ok()
                .filter(|p| !p.is_empty())
                .map(std::path::PathBuf::from)
                .or_else(auto_sandbox_exe),
            // MCP server 配置：缺省未配置，经 `ONION_MCP_SERVERS` 指向 JSON 配置文件。
            mcp_servers_config: std::env::var("ONION_MCP_SERVERS").ok().filter(|p| !p.is_empty()).map(std::path::PathBuf::from),
            // workspace 依赖负载：缺省未配置，经 `ONION_WORKSPACE_DEPS_SOURCE` 指向负载目录。
            workspace_deps_source: std::env::var("ONION_WORKSPACE_DEPS_SOURCE").ok().filter(|p| !p.is_empty()).map(std::path::PathBuf::from),
            // Agent 预设：当前 Agent id（默认 coding）+ 自定义 YAML 文件（ONION_AGENTS_DIR）。
            agent_preset: env_or("ONION_AGENT_PRESET", "coding"),
            agents_dir: std::env::var("ONION_AGENTS_DIR").ok().filter(|p| !p.is_empty()).map(std::path::PathBuf::from),
            // 子代理显式模型选择白名单：缺省关闭（只允许父路由）。
            subagent_model_selection: std::env::var("ONION_SUBAGENT_MODEL_SELECTION")
                .ok()
                .filter(|raw| !raw.trim().is_empty())
                .map(|raw| {
                    serde_json::from_str::<Vec<crate::subagent::model_selection::AllowedModelRoute>>(
                        &raw,
                    )
                    .unwrap_or_else(|e| panic!("ONION_SUBAGENT_MODEL_SELECTION 解析失败: {e}"))
                }),
            // agent-team：opt-in（ONION_AGENT_TEAM=1），缺省不启用；启用时读
            // 可选限额（默认 16/256/65536/5000）。
            agent_team: env_parse::<bool>("ONION_AGENT_TEAM", false).then(|| TeamConfig {
                max_members: env_parse("ONION_AGENT_TEAM_MAX_MEMBERS", 16),
                max_tasks: env_parse("ONION_AGENT_TEAM_MAX_TASKS", 256),
                max_message_bytes: env_parse("ONION_AGENT_TEAM_MAX_MESSAGE_BYTES", 65_536),
                disposal_timeout_ms: env_parse("ONION_AGENT_TEAM_DISPOSAL_TIMEOUT_MS", 5_000),
            }),
            // A2A mesh 令牌：缺省未配置（跨实例委派走匿名 `/a2a` 面）。
            a2a_token: std::env::var("ONION_A2A_TOKEN")
                .ok()
                .map(|raw| raw.trim().to_string())
                .filter(|value| !value.is_empty()),
        }
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

/// 本地默认值文件（`<ONION_HOME>/local-defaults.json`，不入库）：
/// `{"baseUrl":"...","apiKey":"..."}`。缺失或字段为空时返回 None。
fn local_default(key: &str) -> Option<String> {
    let path = crate::paths::data_home().join("local-defaults.json");
    let raw = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

/// 缺省模型基址：`SMOKE_LLM_BASE_URL` > 本地默认值文件 > 空（未配置）。
pub(crate) fn default_base_url() -> String {
    env_or("SMOKE_LLM_BASE_URL", &local_default("baseUrl").unwrap_or_default())
}

/// 缺省模型密钥：`SMOKE_LLM_API_KEY` > 本地默认值文件 > 空（未配置）。
pub(crate) fn default_api_key() -> String {
    env_or("SMOKE_LLM_API_KEY", &local_default("apiKey").unwrap_or_default())
}

/// 环境变量解析为可解析类型（解析失败回退默认值）。
fn env_parse<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key).ok().and_then(|value| value.parse().ok()).unwrap_or(default)
}

/// 自动识别 `dir` 下的 onion-sandbox 可执行文件（与当前程序同目录，sidecar 布局）。
fn sandbox_exe_in(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let name = if cfg!(windows) { "onion-sandbox.exe" } else { "onion-sandbox" };
    let candidate = dir.join(name);
    candidate.is_file().then_some(candidate)
}

/// 以当前可执行文件所在目录为基准自动识别沙箱 exe；定位失败返回 None（直通）。
fn auto_sandbox_exe() -> Option<std::path::PathBuf> {
    sandbox_exe_in(std::env::current_exe().ok()?.parent()?)
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
