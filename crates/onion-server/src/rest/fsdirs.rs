//! `GET /v1/fs/dirs?path=<absolute>[&files=1]`：目录枚举（工作区选择器 / 文件浏览器数据源）。
//!
//! 只读；默认只返回**子目录**（工作区选择器语义），`files=1` 时额外返回当前目录下的
//! **常规文件**（仅名字与路径，不含内容）供文件浏览器渲染。`path` 缺省时回落到用户
//! 主目录；Windows 下额外返回可用驱动器根（`roots`），便于跨盘选择。
//! `canonicalize` 后回显规范化路径（去掉 `\\?\` 前缀）。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use axum::extract::Query;
use axum::Json;
use serde::Serialize;

use crate::rest::error::ApiError;

/// 单个子目录项。
#[derive(Debug, Serialize)]
pub(crate) struct DirEntry {
    /// 目录名（末段）
    name: String,
    /// 绝对路径（可直接作为会话 cwd）
    path: String,
}

/// 单个文件项（仅 `files=1` 时返回）。
#[derive(Debug, Serialize)]
pub(crate) struct FileEntry {
    /// 文件名（末段）
    name: String,
    /// 绝对路径（可传给 /v1/workspace/file 预览）
    path: String,
}

/// 目录枚举结果。
#[derive(Debug, Serialize)]
pub(crate) struct DirListing {
    /// 当前目录绝对路径
    path: String,
    /// 上级目录；已是根（如 `C:\`）时为 null
    parent: Option<String>,
    /// 子目录（按名称排序）
    entries: Vec<DirEntry>,
    /// 常规文件（按名称排序；未请求 `files=1` 时为空）
    #[serde(skip_serializing_if = "Vec::is_empty")]
    files: Vec<FileEntry>,
    /// 可用驱动器根（仅 Windows 非空，供跨盘切换）
    roots: Vec<String>,
}

/// 规范化路径 → 展示用字符串（Windows `canonicalize` 会带 `\\?\` 前缀）。
fn display(path: &Path) -> String {
    let raw = path.to_string_lossy();
    if let Some(rest) = raw.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    raw.strip_prefix(r"\\?\").unwrap_or(&raw).to_string()
}

/// 用户主目录（Windows `USERPROFILE`，其余 `HOME`）。
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
}

/// 可用驱动器根（非 Windows 平台恒为空）。
fn drive_roots() -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        ('A'..='Z')
            .map(|letter| format!("{letter}:\\"))
            .filter(|root| Path::new(root).is_dir())
            .collect()
    }
    #[cfg(not(target_os = "windows"))]
    {
        Vec::new()
    }
}

/// 枚举 `raw` 指向目录的子目录；`None`/空串回落主目录。
/// `include_files` 为真时同时返回常规文件（文件浏览器语义）。
pub(crate) fn list_dirs(raw: Option<&str>, include_files: bool) -> Result<DirListing, ApiError> {
    let requested = raw.map(str::trim).filter(|value| !value.is_empty());
    let start = match requested {
        Some(value) => {
            if value.chars().any(char::is_control) || !Path::new(value).is_absolute() {
                return Err(ApiError::bad_request(
                    "path_invalid",
                    format!("path `{value}` must be an absolute directory path"),
                ));
            }
            PathBuf::from(value)
        }
        None => home_dir().ok_or_else(|| ApiError::internal("home directory unavailable"))?,
    };
    let current = start.canonicalize().map_err(|_| {
        ApiError::not_found("dir_not_found", format!("directory `{}` not found", display(&start)))
    })?;
    if !current.is_dir() {
        return Err(ApiError::bad_request(
            "path_invalid",
            format!("`{}` is not a directory", display(&current)),
        ));
    }

    let mut entries = Vec::new();
    let mut files = Vec::new();
    let reader = std::fs::read_dir(&current)
        .map_err(|error| ApiError::internal(format!("read_dir failed: {error}")))?;
    for item in reader.flatten() {
        let path = item.path();
        let name = item.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            entries.push(DirEntry { name, path: display(&path) });
        } else if include_files && path.is_file() {
            files.push(FileEntry { name, path: display(&path) });
        }
    }
    // 大小写不敏感排序：`sort_by_cached_key` 每个元素只取一次键，避免比较期反复分配。
    entries.sort_by_cached_key(|entry| entry.name.to_lowercase());
    files.sort_by_cached_key(|entry| entry.name.to_lowercase());

    Ok(DirListing {
        path: display(&current),
        parent: current.parent().map(display),
        entries,
        files,
        roots: drive_roots(),
    })
}

/// 目录枚举入口（`?path=`，缺省为用户主目录；`files=1` 同时返回文件）。
pub(crate) async fn dirs(
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<DirListing>, ApiError> {
    let include_files = matches!(query.get("files").map(String::as_str), Some("1" | "true"));
    Ok(Json(list_dirs(
        query.get("path").map(String::as_str),
        include_files,
    )?))
}

#[cfg(test)]
#[path = "fsdirs_tests.rs"]
mod tests;
