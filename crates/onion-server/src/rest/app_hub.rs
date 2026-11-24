//! app-hub：从远程仓库的 `apps/` 目录发现并安装 App。
//!
//! 仓库缺省为 `sungatetop/Ava2`（可用 env `ONION_APP_HUB_REPO` 覆盖为
//! `owner/repo`，`ONION_APP_HUB_BRANCH` 覆盖分支，`ONION_APP_HUB_TOKEN`
//! 提供可选令牌以放宽匿名 API 限额）。
//!
//! - 读面 `GET /v1/apps/hub?q=`：拉取仓库文件树（一次 GitHub API 请求），
//!   筛出 `apps/<id>/manifest.json`，按 raw 通道逐个取清单原文（不计 API
//!   限额），只保留入口产物在库内的目录——即预构建产物，源码包不可安装。
//! - 写面 `POST /v1/apps/hub/install`：按同一文件树把 `apps/<id>/` 下的
//!   全部文件经 raw 通道下载到临时目录，再走与本地安装完全相同的校验 /
//!   复制 / 登记。
//!
//! 安装期不做 npm / 构建：仓库里必须是可直接加载的 ESM 产物。

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Duration;

use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::rest::apps::{install_from_source, AppEntry};
use crate::rest::error::ApiError;
use crate::router::RouterState;

/// 缺省仓库（owner/repo）。
const DEFAULT_REPO: &str = "sungatetop/Ava2";
/// 缺省分支。
const DEFAULT_BRANCH: &str = "main";
/// 缺省 GitHub API 基址。
const DEFAULT_API_BASE: &str = "https://api.github.com";
/// 缺省 raw 内容基址。
const DEFAULT_RAW_BASE: &str = "https://raw.githubusercontent.com";
/// 仓库 `apps/` 目录在仓库内的相对路径。
const APPS_PREFIX: &str = "apps/";
/// 元数据 / raw 文件请求超时。
const FILE_TIMEOUT: Duration = Duration::from_secs(60);
/// 单个文件上限（与 App 静态产物的 `MAX_ASSET_BYTES` 对齐）。
const MAX_FILE_BYTES: usize = 16 * 1024 * 1024;
/// 单个 App 下载总量上限。
const MAX_APP_BYTES: usize = 64 * 1024 * 1024;
/// 单个 App 文件数上限（防御异常仓库）。
const MAX_APP_FILES: usize = 512;

/// Hub 仓库坐标（owner/repo + 分支 + 可选令牌 + 端点基址）。
struct HubRepo {
    repo: String,
    branch: String,
    token: Option<String>,
    /// GitHub API 基址（可覆盖以指向企业版 / 镜像 / 测试桩）。
    api_base: String,
    /// raw 内容基址。
    raw_base: String,
}

impl HubRepo {
    fn from_env() -> Self {
        let pick = |key: &str, fallback: &str| {
            std::env::var(key)
                .ok()
                .map(|raw| raw.trim().trim_end_matches('/').to_string())
                .filter(|raw| !raw.is_empty())
                .unwrap_or_else(|| fallback.to_string())
        };
        HubRepo {
            repo: pick("ONION_APP_HUB_REPO", DEFAULT_REPO),
            branch: pick("ONION_APP_HUB_BRANCH", DEFAULT_BRANCH),
            token: std::env::var("ONION_APP_HUB_TOKEN")
                .ok()
                .map(|raw| raw.trim().to_string())
                .filter(|raw| !raw.is_empty()),
            api_base: pick("ONION_APP_HUB_API_BASE", DEFAULT_API_BASE),
            raw_base: pick("ONION_APP_HUB_RAW_BASE", DEFAULT_RAW_BASE),
        }
    }

    fn client(timeout: Duration) -> Result<reqwest::Client, ApiError> {
        reqwest::Client::builder()
            .timeout(timeout)
            .user_agent("ava2-onion-server")
            .build()
            .map_err(|e| ApiError::internal(format!("build http client failed: {e}")))
    }

    fn source(&self) -> String {
        format!("{}@{}", self.repo, self.branch)
    }

    fn tree_url(&self) -> String {
        format!(
            "{}/repos/{}/git/trees/{}?recursive=1",
            self.api_base, self.repo, self.branch
        )
    }

    fn raw_url(&self, path: &str) -> String {
        format!("{}/{}/{}/{}", self.raw_base, self.repo, self.branch, path)
    }

    fn authed(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.token {
            Some(token) => request.header("authorization", format!("Bearer {token}")),
            None => request,
        }
    }

    /// 候选分支：配置值优先，其后 `main` / `master` 兜底（本地与远端分支
    /// 命名不一致时 Hub 不至于整体失效）。
    fn candidate_branches(&self) -> Vec<String> {
        let mut out = vec![self.branch.clone()];
        for branch in ["main", "master"] {
            if !out.iter().any(|existing| existing == branch) {
                out.push(branch.to_string());
            }
        }
        out
    }

    /// 解析可用分支并返回文件树；成功时把 `self.branch` 固化为实际分支。
    async fn resolve_tree(&mut self) -> Result<HashSet<String>, ApiError> {
        let client = HubRepo::client(FILE_TIMEOUT)?;
        let mut last_error = None;
        for branch in self.candidate_branches() {
            let previous = std::mem::replace(&mut self.branch, branch);
            match fetch_tree(&client, self).await {
                Ok(tree) => return Ok(tree),
                Err(error) => {
                    last_error = Some(error);
                    self.branch = previous;
                }
            }
        }
        Err(last_error.unwrap_or_else(|| ApiError::internal("App Hub 仓库不可用")))
    }
}

/// 一个仓库内可安装的 App（清单 + 是否已装）。
#[derive(Serialize)]
pub(crate) struct HubApp {
    /// 清单原文（前端按 `AppManifest` 消费）。
    manifest: Value,
    /// 登记表口径：本机已安装。
    installed: bool,
}

#[derive(Serialize)]
pub(crate) struct HubListResponse {
    /// 仓库坐标（`owner/repo@branch`），供 UI 展示来源。
    source: String,
    apps: Vec<HubApp>,
}

#[derive(Deserialize)]
pub(crate) struct HubInstallRequest {
    /// 仓库 `apps/` 下的目录名（与清单 id 一致）。
    id: String,
    #[serde(default)]
    pub overwrite: bool,
}

/// GitHub 文件树响应（只取 path / type）。
#[derive(Deserialize)]
struct TreeResponse {
    #[serde(default)]
    tree: Vec<TreeEntry>,
}

#[derive(Deserialize)]
struct TreeEntry {
    path: String,
    #[serde(rename = "type")]
    kind: String,
}

/// 清单声明的全部入口（顶层 entry + contributes.apps[].entry）。
fn manifest_entries(manifest: &Value) -> Vec<&str> {
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
    entries
}

/// 清单 id：与安装侧 `validate_manifest` 的合法字符集保持一致。
fn app_id_of(manifest: &Value) -> Option<String> {
    manifest
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| {
            !id.is_empty()
                && id.len() <= 64
                && id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
                && *id != "."
                && *id != ".."
        })
        .map(str::to_string)
}

/// 拉取仓库文件树，返回全部 blob 路径（相对仓库根）。
async fn fetch_tree(client: &reqwest::Client, repo: &HubRepo) -> Result<HashSet<String>, ApiError> {
    let response = repo
        .authed(client.get(repo.tree_url()))
        .send()
        .await
        .map_err(|e| ApiError::internal(format!("请求仓库文件树失败（{}）: {e}", repo.repo)))?;
    if !response.status().is_success() {
        return Err(ApiError::internal(format!(
            "仓库文件树不可用：HTTP {}（{}）",
            response.status().as_u16(),
            repo.source()
        )));
    }
    let tree: TreeResponse = response
        .json()
        .await
        .map_err(|e| ApiError::internal(format!("解析仓库文件树失败: {e}")))?;
    Ok(tree
        .tree
        .into_iter()
        .filter(|entry| entry.kind == "blob")
        .map(|entry| entry.path)
        .collect())
}

/// 取单个文件文本（raw 通道，不计 API 限额）。
async fn fetch_text(client: &reqwest::Client, url: &str) -> Option<String> {
    let response = client.get(url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    response.text().await.ok()
}

/// 从文件树里收集 `apps/<id>/` 下的全部文件（相对 App 根）。
fn app_files<'a>(paths: &'a HashSet<String>, prefix: &str) -> Vec<(String, &'a str)> {
    let mut files: Vec<(String, &str)> = paths
        .iter()
        .filter_map(|path| {
            let relative = path.strip_prefix(prefix)?;
            // 只接受目录内的普通相对路径
            if relative.is_empty() || relative.contains("..") || relative.starts_with('/') {
                return None;
            }
            Some((relative.to_string(), path.as_str()))
        })
        .collect();
    files.sort();
    files
}

/// App 产物基准目录：清单可能位于 `apps/<id>/`（产物直接入库）或
/// `apps/<id>/dist/`（构建型 App 的构建输出入库）。
fn bundle_bases(paths: &HashSet<String>) -> Vec<(String, String)> {
    let mut bases: Vec<(String, String)> = Vec::new();
    for path in paths {
        let Some(rest) = path.strip_prefix(APPS_PREFIX) else {
            continue;
        };
        let segments: Vec<&str> = rest.split('/').collect();
        for depth in [0usize, 1usize] {
            // 清单必须处在候选基准目录的根（`<id>/manifest.json` 或 `<id>/dist/manifest.json`）
            if segments.len() != depth + 2 || segments[depth + 1] != "manifest.json" {
                continue;
            }
            if depth == 1 && segments[1] != "dist" {
                continue;
            }
            let id = segments[0];
            let base = format!("{APPS_PREFIX}{}/", segments[..=depth].join("/"));
            if !bases.iter().any(|(existing, _)| existing == id) {
                bases.push((id.to_string(), base));
            }
        }
    }
    bases.sort();
    bases
}

/// `GET /v1/apps/hub?q=`：仓库 `apps/` 下可安装（预构建）App 列表，支持关键字过滤。
pub(crate) async fn list(
    State(state): State<RouterState>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<HubListResponse>, ApiError> {
    let mut repo = HubRepo::from_env();
    let blob_paths = repo.resolve_tree().await?;
    let client = HubRepo::client(FILE_TIMEOUT)?;

    let candidates = bundle_bases(&blob_paths);

    let apps_dir = (*state.apps_dir).clone();
    let installed: HashSet<String> =
        tokio::task::spawn_blocking(move || crate::rest::apps::installed_ids(&apps_dir))
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?
            .into_iter()
            .collect();

    let keyword = query
        .get("q")
        .map(|raw| raw.trim().to_lowercase())
        .unwrap_or_default();
    let mut apps = Vec::new();
    for (dir, base) in candidates {
        let Some(raw) = fetch_text(&client, &repo.raw_url(&format!("{base}manifest.json"))).await
        else {
            continue;
        };
        let Ok(manifest) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        if !manifest.is_object() {
            continue;
        }
        let Some(id) = app_id_of(&manifest) else {
            continue;
        };
        // 目录名与清单 id 不一致时跳过，避免安装目标歧义。
        if id != dir {
            continue;
        }
        // 只保留入口产物已在库内的目录：源码包（需构建）不可安装。
        let entries = manifest_entries(&manifest);
        if entries.is_empty()
            || !entries
                .iter()
                .all(|entry| blob_paths.contains(&format!("{base}{entry}")))
        {
            continue;
        }
        if !keyword.is_empty() {
            let haystack = format!(
                "{} {} {}",
                manifest
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
                id,
                manifest
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
            )
            .to_lowercase();
            if !haystack.contains(&keyword) {
                continue;
            }
        }
        apps.push(HubApp {
            installed: installed.contains(&id),
            manifest,
        });
    }

    Ok(Json(HubListResponse {
        source: repo.source(),
        apps,
    }))
}

/// 把 `apps/<id>/` 下的文件逐个下载到 `target`（保留相对路径）。
async fn download_app_files(
    repo: &HubRepo,
    files: &[(String, &str)],
    target: &Path,
) -> Result<(), ApiError> {
    let client = HubRepo::client(FILE_TIMEOUT)?;
    let mut total = 0usize;
    for (relative, repo_path) in files {
        let response = client
            .get(repo.raw_url(repo_path))
            .send()
            .await
            .map_err(|e| ApiError::internal(format!("下载 {repo_path} 失败: {e}")))?;
        if !response.status().is_success() {
            return Err(ApiError::internal(format!(
                "下载 {repo_path} 失败：HTTP {}",
                response.status().as_u16()
            )));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|e| ApiError::internal(format!("读取 {repo_path} 失败: {e}")))?;
        if bytes.len() > MAX_FILE_BYTES {
            return Err(ApiError::bad_request(
                "app_hub_file_too_large",
                format!("{repo_path} 超过单文件上限"),
            ));
        }
        total += bytes.len();
        if total > MAX_APP_BYTES {
            return Err(ApiError::bad_request(
                "app_hub_too_large",
                "App 产物总量超过上限",
            ));
        }
        let path = target.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ApiError::internal(format!("创建目录失败: {e}")))?;
        }
        std::fs::write(&path, &bytes)
            .map_err(|e| ApiError::internal(format!("写入 {relative} 失败: {e}")))?;
    }
    Ok(())
}

/// `POST /v1/apps/hub/install`：下载 `apps/<id>/` → 校验安装登记。
pub(crate) async fn install(
    State(state): State<RouterState>,
    Json(body): Json<HubInstallRequest>,
) -> Result<Json<AppEntry>, ApiError> {
    let id = body.id.trim().to_string();
    if id.is_empty()
        || id.len() > 64
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        || id == "."
        || id == ".."
    {
        return Err(ApiError::bad_request(
            "app_id_invalid",
            format!("App id `{id}` 非法"),
        ));
    }
    let mut repo = HubRepo::from_env();
    let blob_paths = repo.resolve_tree().await?;
    // 产物基准目录：`apps/<id>/` 或 `apps/<id>/dist/`（构建型 App）
    let base = bundle_bases(&blob_paths)
        .into_iter()
        .find(|(dir, _)| dir == &id)
        .map(|(_, base)| base)
        .ok_or_else(|| {
            ApiError::not_found(
                "app_hub_not_found",
                format!("仓库 apps/ 下不存在 App「{id}」"),
            )
        })?;
    let files = app_files(&blob_paths, &base);
    if files.is_empty() {
        return Err(ApiError::not_found(
            "app_hub_not_found",
            format!("仓库 apps/ 下不存在 App「{id}」"),
        ));
    }
    if files.len() > MAX_APP_FILES {
        return Err(ApiError::bad_request(
            "app_hub_too_many_files",
            format!("App「{id}」文件数超过上限"),
        ));
    }

    let staging = std::env::temp_dir().join(format!(
        "ava2-app-hub-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&staging)
        .map_err(|e| ApiError::internal(format!("创建临时目录失败: {e}")))?;

    let downloaded = download_app_files(&repo, &files, &staging).await;
    let apps_dir = (*state.apps_dir).clone();
    let overwrite = body.overwrite;
    let staging_for_install = staging.clone();
    let result = match downloaded {
        Err(error) => Err(error),
        Ok(()) => tokio::task::spawn_blocking(move || {
            install_from_source(&staging_for_install, &apps_dir, overwrite)
        })
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?,
    };
    let _ = std::fs::remove_dir_all(&staging);
    Ok(Json(result?))
}

#[cfg(test)]
#[path = "app_hub_tests.rs"]
mod tests;
