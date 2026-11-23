//! LSP seam 类型与 [`LspService`]。
//!
//! 词汇约定：位置与范围为 zero-based UTF-16，匹配 LSP 线协议；模型面工具持有
//! one-based 光标约定。seam 不暴露协议类型 / 进程 / 文档控制或通用 JSON-RPC 逃生舱，
//! 只暴露四种语义操作。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// 四种语义查询（闭集联合）：新增操作须跨 seam / provider / 工具同步。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LspOperation {
    GoToDefinition,
    FindReferences,
    GoToImplementation,
    Hover,
}

impl LspOperation {
    /// 稳定字符串标识（工具 schema enum 与错误消息用）。
    pub fn as_str(&self) -> &'static str {
        match self {
            LspOperation::GoToDefinition => "goToDefinition",
            LspOperation::FindReferences => "findReferences",
            LspOperation::GoToImplementation => "goToImplementation",
            LspOperation::Hover => "hover",
        }
    }

    /// 全部操作（schema enum 与校验用）。
    pub const ALL: [LspOperation; 4] = [
        LspOperation::GoToDefinition,
        LspOperation::FindReferences,
        LspOperation::GoToImplementation,
        LspOperation::Hover,
    ];

    /// 由字符串解析操作；未知返回 None。
    pub fn parse(value: &str) -> Option<LspOperation> {
        Self::ALL.iter().copied().find(|op| op.as_str() == value)
    }
}

/// zero-based UTF-16 光标坐标（匹配 LSP 线协议）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LspPosition {
    /// zero-based 行。
    pub line: u32,
    /// zero-based UTF-16 码元偏移。
    pub character: u32,
}

/// zero-based UTF-16 半开区间 `[start, end)`。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LspRange {
    pub start: LspPosition,
    pub end: LspPosition,
}

/// 调用方归一化查询。每字段必填：`workspace_root` 调用方提供，`language_id` 来自
/// provider 注册（不在此处），消费者自持超时与结果上限。
#[derive(Clone, Debug)]
pub struct LspQueryRequest {
    /// 语义操作。
    pub operation: LspOperation,
    /// 查询源文件（相对 `workspace_root` 或绝对；provider 规范化）。
    pub file_path: String,
    /// zero-based UTF-16 光标位置。
    pub position: LspPosition,
    /// provider 解析与索引的 workspace 根；必填，永不作缺省。
    pub workspace_root: String,
}

/// 一条解析结果：目标文档 URI 与其中区间。
#[derive(Clone, Debug)]
pub struct LspLocation {
    /// 目标文档 URI（`file:` 或其它），原样来自 server。
    pub uri: String,
    /// 目标文档内区间。
    pub range: LspRange,
}

/// 归一化 hover 内容，或 `null`（该位置无 hover）。
#[derive(Clone, Debug)]
pub struct LspHover {
    /// 归一化 hover 文本（markdown 或纯文本，provider 拼接）。
    pub contents: String,
    /// server 提供时该 hover 的区间。
    pub range: Option<LspRange>,
}

/// 闭集结果联合：导航操作归一化为 `locations`；`hover` 归一化为内容或 `null`。
#[derive(Clone, Debug)]
pub enum LspQueryResult {
    /// 导航结果。`resolved_workspace_uri` 为 provider 对该 workspace 根的规范 `file:` URI，
    /// 调用方相对化 location URI 时必须使用它（勿用宿主机平台规则解析可能含符号链接的
    /// 进程路径）。
    Locations {
        locations: Vec<LspLocation>,
        resolved_workspace_uri: String,
    },
    Hover { hover: Option<LspHover> },
}

/// 语言服务器后端（注册于 [`LspService`]）。每个 provider 持稳定 id 与「扩展名 → 语言
/// id」映射。`findReferences` 恒含声明——provider 内部保证，调用方无标志。
pub trait LspProvider: Send + Sync {
    /// 稳定 provider 标识（与扩展名映射原子预留）。
    fn id(&self) -> &str;
    /// 小写、leading-dot 扩展名 → LSP 语言 id（如 `{ ".rs": "rust" }`）。
    fn extension_to_language(&self) -> &HashMap<String, String>;
    /// 运行一次查询。seam 已选好 provider 并派生 `language_id`。
    fn query(&self, request: &LspQueryRequest, language_id: &str) -> Result<LspQueryResult, LspError>;
}

/// 结构化 LSP 失败。带稳定 `code`（调用方据此路由，而非解析 message）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LspError {
    pub message: String,
    pub code: LspErrorCode,
}

impl std::fmt::Display for LspError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for LspError {}

/// 稳定错误码。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LspErrorCode {
    /// provider id 非法 / 扩展名非法 / 语言 id 为空。
    InvalidProvider,
    /// 已注册 id / 扩展名冲突。
    Conflict,
    /// 无 provider 处理该文件扩展名。
    Unavailable,
    /// provider 不支持该操作。
    UnsupportedOperation,
    /// provider 返回畸形响应。
    MalformedResponse,
}

impl LspError {
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self { message: message.into(), code: LspErrorCode::Unavailable }
    }
    pub fn invalid_provider(message: impl Into<String>) -> Self {
        Self { message: message.into(), code: LspErrorCode::InvalidProvider }
    }
    pub fn conflict(message: impl Into<String>) -> Self {
        Self { message: message.into(), code: LspErrorCode::Conflict }
    }
}

/// 一条选择路由：要运行的 provider + 同步文档的语言 id。
#[derive(Clone)]
struct Route {
    provider: Arc<dyn LspProvider>,
    language_id: String,
}

/// LSP 能力 seam。持 id 预留与「扩展名 → 路由」表；两者随
/// provider 一并填充/清除，保证路由恒有活的 provider。选择按文件最终扩展名路由，
/// 与注册顺序无关；无匹配抛 `LSP_UNAVAILABLE`。
#[derive(Clone, Default)]
pub struct LspService {
    provider_ids: Arc<Mutex<Vec<String>>>,
    routes: Arc<Mutex<HashMap<String, Route>>>,
}

impl LspService {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册 provider，原子预留其 id 与每个归一化扩展名。任何冲突 / 非法输入
    /// 不发布任何内容并抛 [`LspError`]。返回释放全部预留的同步 disposer。
    pub fn register_provider(&self, provider: Arc<dyn LspProvider>) -> Result<(), LspError> {
        let id = provider.id();
        if id.trim().is_empty() {
            return Err(LspError::invalid_provider("an LSP provider id must be a non-empty string"));
        }
        let exts = provider.extension_to_language();
        if exts.is_empty() {
            return Err(LspError::invalid_provider(format!(
                "LSP provider \"{id}\" registers no file extensions"
            )));
        }
        // 归一化扩展名并捕内部重复，再查跨 provider 冲突；全部通过后才变更。
        let mut pending: HashMap<String, Route> = HashMap::new();
        for (raw_ext, language_id) in exts {
            let ext = normalize_extension(raw_ext);
            if !is_valid_extension(&ext) {
                return Err(LspError::invalid_provider(format!(
                    "LSP provider \"{id}\" maps an invalid extension \"{raw_ext}\""
                )));
            }
            if language_id.trim().is_empty() {
                return Err(LspError::invalid_provider(format!(
                    "LSP provider \"{id}\" maps extension \"{ext}\" to an empty language id"
                )));
            }
            if pending.contains_key(&ext) {
                return Err(LspError::invalid_provider(format!(
                    "LSP provider \"{id}\" maps extension \"{ext}\" more than once"
                )));
            }
            pending.insert(ext, Route { provider: provider.clone(), language_id: language_id.clone() });
        }
        {
            let mut ids = self.provider_ids.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            if ids.iter().any(|existing| existing == id) {
                return Err(LspError::conflict(format!(
                    "an LSP provider with id \"{id}\" is already registered"
                )));
            }
            let mut routes = self.routes.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            for ext in pending.keys() {
                if routes.contains_key(ext) {
                    return Err(LspError::conflict(format!(
                        "extension \"{ext}\" is already handled by another LSP provider"
                    )));
                }
            }
            // 全部检查通过：原子提交 id 与每个扩展名。
            ids.push(id.to_string());
            for (ext, route) in pending {
                routes.insert(ext, route);
            }
        }
        Ok(())
    }

    /// 是否有任何已注册 provider（供装配 / 冒烟诊断）。
    pub fn has_providers(&self) -> bool {
        !self
            .provider_ids
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
    }

    /// 按文件扩展名选 provider 并运行一次查询。选择按查询、与注册顺序无关；
    /// 无匹配抛 [`LspError`]（`Unavailable`，即 `LSP_UNAVAILABLE`）。
    pub fn query(&self, request: &LspQueryRequest) -> Result<LspQueryResult, LspError> {
        let ext = final_extension(&request.file_path);
        let route = self
            .routes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&ext)
            .cloned();
        let Some(route) = route else {
            return Err(LspError::unavailable(format!(
                "no LSP provider handles \"{}\"",
                request.file_path
            )));
        };
        route.provider.query(request, &route.language_id)
    }
}

/// 提取文件最终扩展名：规范化、小写、leading-dot（如 `Foo.TS` → `.ts`、`foo.d.ts` →
/// `.ts`）。无扩展名或 leading-dot dotfile（`.bashrc`）返回空串——无路由会匹配。
/// 同时按 `/` 与 `\` 切分，调用方路径分隔符不影响结果。
pub fn final_extension(file_path: &str) -> String {
    let last_slash = file_path.rfind(['/', '\\']);
    let base = match last_slash {
        Some(i) => &file_path[i + 1..],
        None => file_path,
    };
    let dot = base.rfind('.');
    // dot <= 0 覆盖「无点」(-1) 与 leading-dot dotfile (0)：两者皆无扩展名。
    match dot {
        Some(i) if i > 0 => base[i..].to_lowercase(),
        _ => String::new(),
    }
}

/// 合法归一化扩展名：一个点后跟一个或多个非点、非分隔符字符。
fn is_valid_extension(ext: &str) -> bool {
    let bytes = ext.as_bytes();
    bytes.first() == Some(&b'.')
        && bytes.len() > 1
        && bytes[1..].iter().all(|b| *b != b'.' && *b != b'/' && *b != b'\\')
}

/// 小写扩展名并确保 leading-dot；不合法由调用方 `is_valid_extension` 拒绝。
fn normalize_extension(ext: &str) -> String {
    let lower = ext.to_lowercase();
    if lower.starts_with('.') { lower } else { format!(".{lower}") }
}

#[cfg(test)]
#[path = "seam_tests.rs"]
mod tests;
