//! `/v1/apps*`：AppHost 的 App 仓库（安装 / 卸载 / 启停 / 静态产物）。
//!
//! 安装模型：
//! - 一个 App = 一个目录（`manifest.json` + 预构建 ESM 产物 + 资源）。安装即把该目录
//!   复制进 `<apps_dir>/<id>/`；
//! - `<apps_dir>/state.json` 是安装登记表（每条 App 的启停与安装时间的有序列表）
//!   ——**未登记的目录不会被宿主发现**；
//! - 只接受**预构建产物**：安装期不做 npm / 构建步骤（不支持源码包，
//!   避免安装即执行任意脚本）。
//!
//! 宿主（web-ui 与桌面端）只经 HTTP 消费，不依赖本机文件系统能力。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::extract::{Path as AxumPath, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::rest::error::ApiError;
use crate::router::RouterState;

/// 宿主支持的 hostApi 上限（与前端 `HOST_API_SUPPORTED` 保持一致）。
const HOST_API_SUPPORTED: u64 = 2;
/// manifest.json 上限（64 KiB）：清单是元数据，防止误把大文件当清单。
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
/// 单个静态产物上限（16 MiB）。
const MAX_ASSET_BYTES: u64 = 16 * 1024 * 1024;
/// 安装时跳过的目录（开发目录里的依赖与版本库不属于产物）。
const SKIP_DIRS: [&str; 2] = ["node_modules", ".git"];

#[derive(Serialize)]
pub(crate) struct AppsResponse {
    apps: Vec<AppEntry>,
}

#[derive(Serialize)]
pub(crate) struct AppEntry {
    manifest: Value,
    enabled: bool,
    installed_at_ms: u64,
}

#[derive(Deserialize)]
pub(crate) struct InstallRequest {
    /// 待安装 App 的本地目录（绝对路径即可，安装时做受控复制）。
    path: String,
    #[serde(default)]
    overwrite: bool,
}

#[derive(Deserialize)]
pub(crate) struct EnabledRequest {
    enabled: bool,
}

/// 安装登记表（`<apps_dir>/state.json`）。
#[derive(Default, Serialize, Deserialize)]
struct InstallState {
    #[serde(default)]
    apps: BTreeMap<String, StateEntry>,
}

#[derive(Clone, Serialize, Deserialize)]
struct StateEntry {
    #[serde(default = "default_enabled")]
    enabled: bool,
    #[serde(default)]
    installed_at_ms: u64,
}

fn default_enabled() -> bool {
    true
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// App id 只允许单段目录名（`[A-Za-z0-9._-]`，且不为 `.` / `..`）。
fn validate_id(id: &str) -> Result<(), ApiError> {
    let safe = !id.is_empty()
        && id.len() <= 64
        && id != "."
        && id != ".."
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if safe {
        Ok(())
    } else {
        Err(ApiError::bad_request(
            "app_id_invalid",
            format!("app id `{id}` must match [A-Za-z0-9._-] and not be `.`/`..`"),
        ))
    }
}

fn state_path(apps_dir: &Path) -> PathBuf {
    apps_dir.join("state.json")
}

fn read_state(apps_dir: &Path) -> InstallState {
    fs::read_to_string(state_path(apps_dir))
        .ok()
        // 容忍 UTF-8 BOM：登记表可能被外部工具（PowerShell Set-Content 等）重写。
        .map(|raw| raw.strip_prefix('\u{feff}').unwrap_or(&raw).to_string())
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

/// 原子写（tmp + rename），避免半写损坏登记表。
fn write_state(apps_dir: &Path, state: &InstallState) -> Result<(), ApiError> {
    fs::create_dir_all(apps_dir).map_err(|e| ApiError::internal(format!("创建 App 目录失败: {e}")))?;
    let body = serde_json::to_string_pretty(state).map_err(|e| ApiError::internal(e.to_string()))?;
    let target = state_path(apps_dir);
    let tmp = apps_dir.join("state.json.tmp");
    fs::write(&tmp, body).map_err(|e| ApiError::internal(format!("写登记表失败: {e}")))?;
    fs::rename(&tmp, &target).map_err(|e| ApiError::internal(format!("提交登记表失败: {e}")))
}

/// 读 manifest（大小受限），校验为 JSON 对象。
pub(crate) fn read_manifest(dir: &Path) -> Result<Value, ApiError> {
    let path = dir.join("manifest.json");
    let meta = fs::metadata(&path).map_err(|_| {
        ApiError::bad_request(
            "app_manifest_missing",
            format!("{} 不存在（App 根目录必须有 manifest.json）", path.display()),
        )
    })?;
    if meta.len() > MAX_MANIFEST_BYTES {
        return Err(ApiError::bad_request(
            "app_manifest_too_large",
            format!("manifest.json 超过 {MAX_MANIFEST_BYTES} 字节"),
        ));
    }
    let raw = fs::read_to_string(&path)
        .map_err(|e| ApiError::bad_request("app_manifest_invalid", format!("读取 manifest 失败: {e}")))?;
    let value: Value = serde_json::from_str(&raw)
        .map_err(|e| ApiError::bad_request("app_manifest_invalid", format!("manifest 不是合法 JSON: {e}")))?;
    if !value.is_object() {
        return Err(ApiError::bad_request(
            "app_manifest_invalid",
            "manifest 顶层必须是对象",
        ));
    }
    Ok(value)
}

/// 校验清单并返回 App id：id 合法、hostApi 不超上限、所有页入口文件真实存在。
fn validate_manifest(manifest: &Value, dir: &Path) -> Result<String, ApiError> {
    let id = manifest
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::bad_request("app_id_missing", "manifest.id 必填"))?;
    validate_id(id)?;
    match manifest.get("name").and_then(Value::as_str) {
        Some(name) if !name.is_empty() => {}
        _ => return Err(ApiError::bad_request("app_name_missing", "manifest.name 必填")),
    }
    let host_api = manifest.get("hostApi").and_then(Value::as_u64).unwrap_or(0);
    if host_api > HOST_API_SUPPORTED {
        return Err(ApiError::bad_request(
            "app_host_api_unsupported",
            format!("manifest.hostApi={host_api} 超出宿主支持上限 {HOST_API_SUPPORTED}"),
        ));
    }
    // 入口集合：顶层 entry + contributes.apps[].entry（皆相对于 App 根目录）
    let mut entries: Vec<&str> = Vec::new();
    if let Some(entry) = manifest.get("entry").and_then(Value::as_str) {
        entries.push(entry);
    }
    if let Some(pages) = manifest
        .get("contributes")
        .and_then(|c| c.get("apps"))
        .and_then(Value::as_array)
    {
        for page in pages {
            if let Some(entry) = page.get("entry").and_then(Value::as_str) {
                entries.push(entry);
            }
        }
    }
    if entries.is_empty() {
        return Err(ApiError::bad_request(
            "app_entry_missing",
            "manifest 需声明 entry 或 contributes.apps[].entry",
        ));
    }
    for entry in entries {
        if Path::new(entry).is_absolute() || entry.contains("..") {
            return Err(ApiError::bad_request(
                "app_entry_invalid",
                format!("入口 `{entry}` 必须是 App 根目录内的相对路径"),
            ));
        }
        if !dir.join(entry).is_file() {
            return Err(ApiError::bad_request(
                "app_entry_not_found",
                format!("入口文件 `{entry}` 不存在，安装包必须是预构建产物"),
            ));
        }
    }
    // 能力贡献（skills / mcpServers / hooks）：路径须落在 App 根目录内且真实存在，
    // MCP / hooks 配置须可解析；`contributes.apps` 的入口已在上面校验。
    crate::app_contrib::validate(manifest, dir)
        .map_err(|error| ApiError::bad_request("app_contributes_invalid", error))?;
    Ok(id.to_string())
}

/// 递归复制（跳过依赖与版本库目录；符号链接一律拒绝，避免安装期逃逸）。
fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        let source = entry.path();
        let target = to.join(&name);
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("App 安装包不得包含符号链接: {}", source.display()),
            ));
        }
        if file_type.is_dir() {
            if SKIP_DIRS.iter().any(|skip| name == *skip) {
                continue;
            }
            copy_tree(&source, &target)?;
        } else {
            fs::copy(&source, &target)?;
        }
    }
    Ok(())
}

/// 已启用 App 的 `(id, 根目录)`：能力贡献装配（技能 / MCP / hooks）与 `/v1/apps`
/// 共用同一份登记表口径，安装与启停后新会话即时生效。
pub(crate) fn enabled_app_dirs(apps_dir: &Path) -> Vec<(String, PathBuf)> {
    let state = read_state(apps_dir);
    state
        .apps
        .into_iter()
        .filter(|(_, entry)| entry.enabled)
        .map(|(id, _)| {
            let dir = apps_dir.join(&id);
            (id, dir)
        })
        .filter(|(_, dir)| dir.is_dir())
        .collect()
}

/// 安装包内置默认 App 的目录（Tauri 壳注入 `ONION_BUNDLED_APPS_DIR`，指向安装包资源
/// 中的 `apps/`；dev 态为 `<src-tauri>/resources/apps`，未构建时不存在则跳过）。
pub(crate) fn bundled_apps_dir() -> Option<PathBuf> {
    std::env::var("ONION_BUNDLED_APPS_DIR")
        .ok()
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty())
        .map(PathBuf::from)
}

/// 首次启动种子安装：把内置默认 App 复制进 App 仓库并登记。
///
/// 仅在登记表（`state.json`）不存在时执行一次——用户之后的卸载 / 启停选择优先，
/// 升级发布不覆盖既有登记。内置目录里的非 App 子目录（无 manifest）自动跳过。
pub(crate) fn seed_default_apps(apps_dir: &Path, bundled: Option<&Path>) -> Result<(), ApiError> {
    let Some(bundled) = bundled else { return Ok(()) };
    if !bundled.is_dir() || state_path(apps_dir).exists() {
        return Ok(());
    }
    let mut state = InstallState::default();
    let mut seeded = false;
    let entries = fs::read_dir(bundled)
        .map_err(|e| ApiError::internal(format!("读取内置 App 目录失败: {e}")))?
        .flatten();
    for entry in entries {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let manifest = match read_manifest(&dir) {
            Ok(manifest) => manifest,
            Err(_) => continue,
        };
        let id = match validate_manifest(&manifest, &dir) {
            Ok(id) => id,
            Err(error) => {
                eprintln!("[apps] 跳过内置 App `{}`: {error:?}", dir.display());
                continue;
            }
        };
        copy_tree(&dir, &apps_dir.join(&id))
            .map_err(|e| ApiError::internal(format!("复制内置 App `{id}` 失败: {e}")))?;
        state
            .apps
            .insert(id, StateEntry { enabled: true, installed_at_ms: now_ms() });
        seeded = true;
    }
    if seeded {
        write_state(apps_dir, &state)?;
    }
    Ok(())
}

/// 扫描登记表，读取每条 App 的清单（目录缺失的登记项跳过并告警）。
fn scan(apps_dir: &Path) -> Vec<AppEntry> {
    let state = read_state(apps_dir);
    let mut out = Vec::new();
    for (id, entry) in state.apps {
        let dir = apps_dir.join(&id);
        match read_manifest(&dir) {
            Ok(manifest) => out.push(AppEntry {
                manifest,
                enabled: entry.enabled,
                installed_at_ms: entry.installed_at_ms,
            }),
            Err(_) => eprintln!("[apps] 跳过登记项 `{id}`（manifest 缺失或不可读）"),
        }
    }
    out
}

/// `GET /v1/apps`：已安装 App 清单（含清单内容与启停态）。
pub(crate) async fn list(State(state): State<RouterState>) -> Result<Json<AppsResponse>, ApiError> {
    let apps_dir = (*state.apps_dir).clone();
    let apps = tokio::task::spawn_blocking(move || scan(&apps_dir))
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(AppsResponse { apps }))
}

/// `GET /v1/apps/{id}`：单条 App 的清单。
pub(crate) async fn manifest(
    State(state): State<RouterState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<Value>, ApiError> {
    validate_id(&id)?;
    let apps_dir = (*state.apps_dir).clone();
    let manifest = tokio::task::spawn_blocking(move || read_manifest(&apps_dir.join(&id)))
        .await
        .map_err(|e| ApiError::internal(e.to_string()))??;
    Ok(Json(manifest))
}

/// `POST /v1/apps/install`：把本地目录安装为 App（受控复制 + 校验 + 登记）。
pub(crate) async fn install(
    State(state): State<RouterState>,
    Json(body): Json<InstallRequest>,
) -> Result<Json<AppEntry>, ApiError> {
    if body.path.trim().is_empty() {
        return Err(ApiError::bad_request("app_path_required", "path 必填"));
    }
    let apps_dir = (*state.apps_dir).clone();
    let source = PathBuf::from(body.path.trim());
    let overwrite = body.overwrite;
    let installed = tokio::task::spawn_blocking(move || -> Result<AppEntry, ApiError> {
        let source = source
            .canonicalize()
            .map_err(|e| ApiError::bad_request("app_path_invalid", format!("安装源不可用: {e}")))?;
        if !source.is_dir() {
            return Err(ApiError::bad_request(
                "app_path_invalid",
                "安装源必须是包含 manifest.json 的目录",
            ));
        }
        let manifest = read_manifest(&source)?;
        let id = validate_manifest(&manifest, &source)?;
        let target = apps_dir.join(&id);
        if target.exists() {
            if !overwrite {
                return Err(ApiError::conflict(
                    "app_exists",
                    format!("App「{id}」已安装；覆盖安装需 overwrite=true"),
                ));
            }
            fs::remove_dir_all(&target)
                .map_err(|e| ApiError::internal(format!("移除旧版本失败: {e}")))?;
        }
        copy_tree(&source, &target)
            .map_err(|e| ApiError::bad_request("app_copy_failed", format!("复制产物失败: {e}")))?;
        let mut state = read_state(&apps_dir);
        let installed_at_ms = now_ms();
        state.apps.insert(
            id,
            StateEntry { enabled: true, installed_at_ms },
        );
        write_state(&apps_dir, &state)?;
        Ok(AppEntry { manifest, enabled: true, installed_at_ms })
    })
    .await
    .map_err(|e| ApiError::internal(e.to_string()))??;
    Ok(Json(installed))
}

/// `DELETE /v1/apps/{id}`：卸载（移除目录与登记项）。
pub(crate) async fn uninstall(
    State(state): State<RouterState>,
    AxumPath(id): AxumPath<String>,
) -> Result<StatusCode, ApiError> {
    validate_id(&id)?;
    let apps_dir = (*state.apps_dir).clone();
    tokio::task::spawn_blocking(move || -> Result<(), ApiError> {
        let target = apps_dir.join(&id);
        if target.exists() {
            fs::remove_dir_all(&target)
                .map_err(|e| ApiError::internal(format!("移除 App 目录失败: {e}")))?;
        }
        let mut state = read_state(&apps_dir);
        if state.apps.remove(&id).is_some() {
            write_state(&apps_dir, &state)?;
        }
        Ok(())
    })
    .await
    .map_err(|e| ApiError::internal(e.to_string()))??;
    Ok(StatusCode::NO_CONTENT)
}

/// `PUT /v1/apps/{id}/enabled`：启停（宿主据此决定是否装配）。
pub(crate) async fn set_enabled(
    State(state): State<RouterState>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<EnabledRequest>,
) -> Result<StatusCode, ApiError> {
    validate_id(&id)?;
    let apps_dir = (*state.apps_dir).clone();
    let enabled = body.enabled;
    tokio::task::spawn_blocking(move || -> Result<(), ApiError> {
        let mut state = read_state(&apps_dir);
        let entry = state
            .apps
            .get_mut(&id)
            .ok_or_else(|| ApiError::not_found("app_not_found", format!("App「{id}」未安装")))?;
        entry.enabled = enabled;
        write_state(&apps_dir, &state)
    })
    .await
    .map_err(|e| ApiError::internal(e.to_string()))??;
    Ok(StatusCode::NO_CONTENT)
}

/// App 静态产物的 MIME（未知类型按二进制直出，浏览器侧不做嗅探）。
fn asset_content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" | "map" => "application/json; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "html" => "text/html; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

/// `GET /v1/apps/{id}/{*path}`：读取 App 目录内的静态产物（拒绝 `..` / 绝对路径 / 符号链接逃逸）。
pub(crate) async fn asset(
    State(state): State<RouterState>,
    AxumPath((id, rest)): AxumPath<(String, String)>,
) -> Result<Response, ApiError> {
    validate_id(&id)?;
    let relative = Path::new(&rest);
    if relative.is_absolute()
        || relative
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::RootDir | Component::Prefix(_)))
    {
        return Err(ApiError::bad_request(
            "app_asset_invalid",
            format!("路径 `{rest}` 必须是 App 目录内的相对路径"),
        ));
    }
    let root = state.apps_dir.join(&id);
    let root = root
        .canonicalize()
        .map_err(|_| ApiError::not_found("app_not_found", format!("App「{id}」未安装")))?;
    let target = root
        .join(relative)
        .canonicalize()
        .map_err(|_| ApiError::not_found("app_asset_not_found", format!("`{rest}` 不存在")))?;
    if !target.starts_with(&root) {
        return Err(ApiError::bad_request(
            "app_asset_invalid",
            format!("`{rest}` 逃逸出 App 目录"),
        ));
    }
    let meta = tokio::fs::metadata(&target)
        .await
        .map_err(|_| ApiError::not_found("app_asset_not_found", format!("`{rest}` 不存在")))?;
    if !meta.is_file() {
        return Err(ApiError::bad_request(
            "app_asset_invalid",
            format!("`{rest}` 不是常规文件"),
        ));
    }
    if meta.len() > MAX_ASSET_BYTES {
        return Err(ApiError::unsupported_media_type(
            "app_asset_too_large",
            format!("`{rest}` 超过 {MAX_ASSET_BYTES} 字节"),
        ));
    }
    let bytes = tokio::fs::read(&target)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut response = (StatusCode::OK, Body::from(bytes)).into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(asset_content_type(&target)),
    );
    // 产物 URL 带 `?v=<version>` 指纹由宿主追加，这里禁缓存以免热替换后跑旧模块。
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    Ok(response)
}

#[cfg(test)]
#[path = "apps_tests.rs"]
mod tests;
