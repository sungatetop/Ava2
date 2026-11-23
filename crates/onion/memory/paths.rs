//! 记忆目录解析与路径安全。
//!
//! 对齐 ava-harness::memory 设计蓝本：路径解析优先级 + 多层安全校验。
//! onion 适配：根目录基于 [`crate::paths::data_home`] + [`crate::paths::normalize_cwd`]
//! 派生 `<root>/projects/<normalized-cwd>/memory/`，完全自包含。

use std::path::{Component, Path, PathBuf};

use crate::memory::{MemoryError, Result};

/// 记忆目录环境变量（全路径覆盖）。
pub const MEMORY_PATH_OVERRIDE_ENV: &str = "ONION_MEMORY_PATH_OVERRIDE";

/// 记忆路径对象。
#[derive(Debug, Clone)]
pub struct MemoryPaths {
    /// 记忆根目录（`<data_home>/projects/<normalized-cwd>/memory/`）。
    pub root: PathBuf,
}

impl MemoryPaths {
    /// 解析记忆根目录。
    ///
    /// 优先级：
    /// 1. `ONION_MEMORY_PATH_OVERRIDE` 环境变量（全路径覆盖）。
    /// 2. 默认：`data_home()/projects/<normalize_cwd(cwd)>/memory/`。
    pub fn resolve(cwd: &Path) -> Self {
        if let Ok(dir) = std::env::var(MEMORY_PATH_OVERRIDE_ENV) {
            if !dir.trim().is_empty() && validate_memory_path(Path::new(&dir)).is_ok() {
                return Self { root: PathBuf::from(dir) };
            }
        }
        let root = crate::paths::data_home()
            .join("projects")
            .join(crate::paths::normalize_cwd(cwd))
            .join("memory");
        Self { root }
    }

    /// 索引文件路径（`root/MEMORY.md`）。
    pub fn entrypoint(&self) -> PathBuf {
        self.root.join(crate::memory::index::ENTRYPOINT_NAME)
    }

    /// 规范化前缀检查：`p` 是否位于记忆根目录内。
    ///
    /// 记忆文件名由 `slug_filename` 派生（无 `..`、无 symlink 段），故此处做
    /// **词法绝对路径**前缀比较即可。不用 `canonicalize`——Windows 上它会给路径加
    /// `\\?\` 前缀，导致与未 canonicalize 的目标路径 `starts_with` 比较失败，
    /// 且根目录在首次写入前尚不存在。
    pub fn is_within(&self, p: &Path) -> bool {
        let root_abs = absolutize(&self.root);
        let target_abs = absolutize(p);
        target_abs.starts_with(&root_abs)
    }
}

/// 绝对化路径（不解析 symlink）；相对路径基于当前目录。
fn absolutize(p: &Path) -> PathBuf {
    if p.is_absolute() {
        return p.to_path_buf();
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(p))
        .unwrap_or_else(|_| p.to_path_buf())
}

/// 校验记忆路径是合法的绝对路径。
///
/// 拒绝：相对路径、根/近根路径、Windows 盘符根、UNC、空字节。
/// 用 `has_root` 而非 `is_absolute`：后者在 Windows 上会拒绝 `/home/...`
/// 这类 Unix 风格根路径，跨平台语义不一致。
pub fn validate_memory_path(p: &Path) -> Result<()> {
    let s = p.to_string_lossy();
    if s.contains('\0') {
        return Err(MemoryError::InvalidPath("contains NUL byte".into()));
    }
    if !p.has_root() {
        return Err(MemoryError::InvalidPath(format!("not absolute: {s}")));
    }
    // Windows 盘符根（C:\）与 UNC（\\server\share）—— 保守拒绝。
    #[cfg(windows)]
    {
        let lower = s.to_ascii_lowercase();
        if lower.len() == 3 && lower.as_bytes()[1] == b':' && lower.ends_with('\\') {
            return Err(MemoryError::InvalidPath(format!("drive root: {s}")));
        }
        if lower.starts_with("\\\\") {
            return Err(MemoryError::InvalidPath(format!("unc path: {s}")));
        }
    }
    // 拒绝 ParentDir 遍历。
    for c in p.components() {
        if matches!(c, Component::ParentDir) {
            return Err(MemoryError::InvalidPath(format!("parent traversal: {s}")));
        }
    }
    Ok(())
}

/// 由记忆名派生安全文件名（slug 化，防路径穿越）。
///
/// 保留首尾连字符：中文等非 ASCII 字符被剔除后可能仅剩分隔符残留
/// （如 `"Ava 图片分析最佳实践"` → `"ava-"`）；全部无效时回退 `"memory"`。
pub fn slug_filename(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch.to_ascii_lowercase());
        } else if ch == ' ' {
            out.push('-');
        }
    }
    let all_dashes = out.chars().all(|c| c == '-');
    if all_dashes {
        "memory".into()
    } else {
        out
    }
}

#[cfg(test)]
#[path = "paths_tests.rs"]
mod tests;
