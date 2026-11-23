//! skill-hub：腾讯 SkillHub（skillhub.cloud.tencent.com）技能市场安装面。
//!
//! `POST /v1/skill-hub/install?session=<id>`：下载 zip 解包安装到
//! `<projectRoot>/.onion/skills/<slug>/`，校验 SKILL.md 后刷新注册表。
//!
//! 读面（搜索 / 详情）不在服务端：App 经通用外联通道（[`crate::rest::net`] 的
//! `GET /v1/net/fetch`）直连上游公开 API（`api.skillhub.cn/api/v1/search` 与
//! `/api/v1/skills/resolve`，匿名可用），上游契约由 App 侧自行解析。
//!
//! 下载链路（本模块私有，App 无法落盘故保留在服务端，实测 2026-11）：主链
//! `/api/v1/download?slug=`（302 → CDN 版本化 zip），兜底 COS 直链；社区源
//! （clawhub）可匿名完整下载，企业源与未知 slug 404。

use std::collections::HashMap;
use std::time::Duration;

use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::rest::capabilities::session_param;
use crate::rest::error::ApiError;
use crate::router::RouterState;

/// 下载主链（302 → CDN 版本化 zip）。
const DOWNLOAD_PRIMARY_URL: &str = "https://api.skillhub.cn/api/v1/download";

/// 下载兜底链（COS 直链）。
const DOWNLOAD_FALLBACK_URL: &str =
    "https://skillhub-1388575217.cos.ap-guangzhou.myqcloud.com/skills";

/// 下载超时。
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60);

/// `POST /v1/skill-hub/install` 入参。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InstallSkillRequest {
    pub slug: String,
}

/// `POST /v1/skill-hub/install` 出参。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InstallSkillResponse {
    /// 安装目标 slug（目录名）。
    pub slug: String,
    /// SKILL.md frontmatter `name`（会话内调用名，可能与 slug 不同）。
    pub name: String,
    /// 落盘目录绝对路径。
    pub dir: String,
    /// zip 内顶层条目名。
    pub files: Vec<String>,
}

/// 下载 slug 对应 zip 字节：主链失败（404 等）→ COS 直链兜底；双链皆败 → 上游错误。
async fn download_skill_zip(slug: &str) -> Result<Vec<u8>, ApiError> {
    let client = reqwest::Client::builder()
        .timeout(DOWNLOAD_TIMEOUT)
        .build()
        .map_err(|error| ApiError::internal(format!("build http client failed: {error}")))?;
    let primary = format!("{DOWNLOAD_PRIMARY_URL}?slug={slug}");
    let fallback = format!("{DOWNLOAD_FALLBACK_URL}/{slug}.zip");
    for url in [primary, fallback] {
        let bytes = match client.get(&url).send().await {
            Ok(response) => match response.error_for_status() {
                Ok(response) => response.bytes().await.map(|bytes| bytes.to_vec()),
                Err(error) => Err(error),
            },
            Err(error) => Err(error),
        };
        match bytes {
            Ok(bytes) => return Ok(bytes),
            // 404（企业源 / 未知 slug）→ 尝试兜底链；其余错误同样兜底后统一报错。
            Err(_) => continue,
        }
    }
    Err(ApiError::not_found(
        "skill_not_downloadable",
        format!("skillhub skill \"{slug}\" is not anonymously downloadable (enterprise or unknown slug)"),
    ))
}

/// zip 安全解包到 target：`enclosed_name` 杜绝 zip-slip（`..` / 绝对路径跳过）。
/// 返回顶层条目名（有序去重）。
fn extract_zip_to(bytes: &[u8], target: &std::path::Path) -> Result<Vec<String>, ApiError> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|error| ApiError::internal(format!("invalid skill zip: {error}")))?;
    let mut top_level: Vec<String> = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| ApiError::internal(format!("read zip entry failed: {error}")))?;
        let Some(relative) = entry.enclosed_name() else {
            continue;
        };
        let top = relative
            .components()
            .next()
            .map(|component| component.as_os_str().to_string_lossy().into_owned());
        if let Some(top) = top {
            if !top_level.contains(&top) {
                top_level.push(top);
            }
        }
        let path = target.join(relative);
        if entry.is_dir() {
            std::fs::create_dir_all(&path)
                .map_err(|error| ApiError::internal(format!("create dir failed: {error}")))?;
        } else {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| ApiError::internal(format!("create dir failed: {error}")))?;
            }
            let mut file = std::fs::File::create(&path)
                .map_err(|error| ApiError::internal(format!("create file failed: {error}")))?;
            std::io::copy(&mut entry, &mut file)
                .map_err(|error| ApiError::internal(format!("write file failed: {error}")))?;
        }
    }
    Ok(top_level)
}

/// `POST /v1/skill-hub/install?session=<id>`：slug 校验 → 下载 zip → 安全解包到
/// `<projectRoot>/.onion/skills/<slug>/` → SKILL.md 校验（缺 / 非法名 422 并清理）
/// → 注册表 refresh（`skills/change` 广播）→ 返回安装结果。
pub(crate) async fn install(
    State(state): State<RouterState>,
    Query(query): Query<HashMap<String, String>>,
    Json(request): Json<InstallSkillRequest>,
) -> Result<Json<InstallSkillResponse>, ApiError> {
    let session = session_param(&state, &query).await?;
    let slug = request.slug.trim().to_string();
    if !onion::skill::is_skill_name(&slug) {
        return Err(ApiError::bad_request(
            "invalid_skill_name",
            format!("\"{slug}\" is not a valid kebab-case skill slug"),
        ));
    }
    let working_dir = session.handle.session_cwd.current().as_ref().clone();
    let project_root = onion::skill::filesystem::find_project_root(&working_dir);
    let target = onion::skill::skills_dir(&project_root.join(".onion")).join(&slug);
    if target.exists() {
        return Err(ApiError::conflict(
            "skill_already_installed",
            format!("skill \"{slug}\" already installed at {}", target.display()),
        ));
    }
    let bytes = download_skill_zip(&slug).await?;
    std::fs::create_dir_all(&target)
        .map_err(|error| ApiError::internal(format!("create skill dir failed: {error}")))?;
    let files = match extract_zip_to(&bytes, &target) {
        Ok(files) => files,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&target);
            return Err(error);
        }
    };
    // 包内容校验：SKILL.md 存在且 frontmatter name 合法，否则视为无效包，清理后 422。
    let parsed =
        onion::skill::filesystem::parse_skill_file(&target.join(onion::skill::SKILL_INSTRUCTION_FILE));
    let Some(parsed) = parsed else {
        let _ = std::fs::remove_dir_all(&target);
        return Err(ApiError::unsupported_media_type(
            "invalid_skill_package",
            format!("downloaded package for \"{slug}\" has no valid SKILL.md (frontmatter name/description required)"),
        ));
    };
    session
        .handle
        .skill_registry
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .refresh();
    Ok(Json(InstallSkillResponse {
        slug,
        name: parsed.name().to_string(),
        dir: target.display().to_string(),
        files,
    }))
}
