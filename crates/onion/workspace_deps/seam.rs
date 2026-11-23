//! `load_workspace_dependencies` 的 seam：路径派生 + 查询服务。
//!
//! 语义：
//! - `source` 为携带 `runtime.json` + `dependencies/` 的负载目录（绝对路径）；
//! - 校验元数据（平台 / 架构须与运行进程一致）→ 派生平台特定路径 → 校验入口存在；
//! - 返回绝对路径 + 记录的 Python 发行版本；
//! - 不改变 `PATH` / 包管理器设置；不复制负载。

use std::path::{Path, PathBuf};

use super::manifest::{read_primary_runtime, PrimaryRuntimeManifest};
use super::WorkspaceDepsError;

/// 绝对入口点与捆绑版本；pnpm 经返回的 Node 可执行文件运行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceDependencies {
    pub python: PathBuf,
    /// 负载未携带 Node.js 时缺失。
    pub node: Option<PathBuf>,
    /// 负载未携带 pnpm 时缺失。
    pub pnpm: Option<PathBuf>,
    pub python_packages: PathBuf,
    /// 负载未携带 Node.js 时缺失。
    pub node_packages: Option<PathBuf>,
    /// 锁定的发行版本（不含用户后装）。
    pub python_distributions: std::collections::BTreeMap<String, String>,
}

/// 查询服务：持有负载 source 目录，`resolve()` 就地解析并校验。
#[derive(Clone)]
pub struct WorkspaceDepsService {
    /// 负载目录（绝对路径，携带 `runtime.json` + `dependencies/`）。
    source: PathBuf,
}

impl WorkspaceDepsService {
    pub fn new(source: PathBuf) -> Self {
        Self { source }
    }

    /// 就地解析负载：校验元数据与入口，返回绝对路径 + 发行版本。
    pub fn resolve(&self) -> Result<WorkspaceDependencies, WorkspaceDepsError> {
        let manifest = read_primary_runtime(&self.source)?;
        // 平台 / 架构须与运行进程一致（不匹配即不可用）。
        if manifest.platform != current_platform() || manifest.arch != current_arch() {
            return Err(WorkspaceDepsError::Incompatible);
        }
        let paths = workspace_dependency_paths(&self.source, &manifest);
        validate_payload_entries(&paths)?;
        Ok(paths)
    }
}

/// 当前进程平台（`win32` / `darwin` / `linux`，与 Node 生态命名一致）。
fn current_platform() -> &'static str {
    if cfg!(windows) {
        "win32"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else {
        "linux"
    }
}

/// 当前进程架构（`x64` / `arm64`）。
fn current_arch() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x64"
    }
}

/// 派生平台特定的解释器与库位置。
fn workspace_dependency_paths(
    root: &Path,
    manifest: &PrimaryRuntimeManifest,
) -> WorkspaceDependencies {
    let dependencies = root.join("dependencies");
    let windows = manifest.platform == "win32";
    let major_minor = major_minor(&manifest.python);
    let node = manifest.node.as_ref().map(|_| {
        let node_dir = dependencies.join("node");
        (
            node_dir.join("bin").join(if windows { "node.exe" } else { "node" }),
            node_dir.join("node_modules"),
        )
    });
    let pnpm = manifest.pnpm.as_ref().map(|_| {
        dependencies
            .join("pnpm")
            .join("bin")
            .join("pnpm.mjs")
    });
    WorkspaceDependencies {
        python: dependencies.join("python").join(if windows {
            "python.exe".to_string()
        } else {
            "bin/python3".to_string()
        }),
        node: node.as_ref().map(|(p, _)| p.clone()),
        pnpm,
        python_packages: dependencies.join("python").join(if windows {
            "Lib".to_string()
        } else {
            format!("lib/python{major_minor}")
        })
        .join("site-packages"),
        node_packages: node.as_ref().map(|(_, p)| p.clone()),
        python_distributions: manifest.python_packages.clone(),
    }
}

/// `python3.12` → `3.12`（用于 `lib/python3.12/site-packages`）。
fn major_minor(version: &str) -> String {
    version.split('.').take(2).collect::<Vec<_>>().join(".")
}

/// 校验声明的入口存在且类型正确（文件 / 目录，`validatePayloadEntries` 语义）。
fn validate_payload_entries(paths: &WorkspaceDependencies) -> Result<(), WorkspaceDepsError> {
    let checks: [(&Path, bool); 5] = [
        (&paths.python, true),
        (paths.node.as_deref().unwrap_or(Path::new("")), true),
        (paths.pnpm.as_deref().unwrap_or(Path::new("")), true),
        (&paths.python_packages, false),
        (paths.node_packages.as_deref().unwrap_or(Path::new("")), false),
    ];
    for (path, is_file) in checks {
        if path.as_os_str().is_empty() {
            continue;
        }
        let meta = std::fs::symlink_metadata(path).map_err(|e| {
            WorkspaceDepsError::MissingEntry(format!("{} ({e})", path.display()))
        })?;
        let ok = if is_file {
            meta.is_file()
        } else {
            meta.is_dir()
        };
        if !ok {
            return Err(WorkspaceDepsError::MissingEntry(format!(
                "expected {} at {}",
                if is_file { "file" } else { "directory" },
                path.display()
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "seam_tests.rs"]
mod tests;
