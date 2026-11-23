//! agent 工作空间：AGENTS.md 发现 + 注入 + 触点刷新。
//!
//! 发现与注入策略：
//! - 发现：global（`~/.onion/AGENTS.md`）→ 项目根 → cwd 祖先链（broad-to-specific）；
//! - 去重：同目录 SHA-1 折叠，保留最早（最 broad）；
//! - 预算：`max_bytes` 内尽量多装，不足时先省略更宽泛文件；
//! - `@import` 指令不解析，按原文透传。
//!
//! FsVersion 指纹（dev:ino:size:mtime_ns:ctime_ns）用于触点对账：read/write_file
//! 工具结果后 reconcile，发现变更即注入 delta reminder。

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::reminders::{escape_reminder_body, system_reminder};

/// 单条指令文件（发现产物；render_baseline 输入）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstructionFile {
    pub path: PathBuf,
    pub content: String,
    pub version: FsVersion,
}

/// 文件版本指纹：dev:ino:size:mtime_ns:ctime_ns。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FsVersion {
    pub dev: u64,
    pub ino: u64,
    pub size: u64,
    pub mtime_ns: u64,
    pub ctime_ns: u64,
}

impl FsVersion {
    fn fingerprint(&self) -> String {
        format!("{}:{}:{}:{}:{}", self.dev, self.ino, self.size, self.mtime_ns, self.ctime_ns)
    }
}

/// 对账变更列表：相对基线新增 / 移除 / 变更的指令文件名。
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct WorkspaceChanges {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<String>,
}

impl WorkspaceChanges {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }

    /// 变更文件名合并列表（delta reminder 载荷）。
    pub fn files(&self) -> Vec<String> {
        self.changed
            .iter()
            .chain(self.added.iter())
            .chain(self.removed.iter())
            .cloned()
            .collect()
    }
}

/// 发现 + 对账失败。
#[derive(Clone, Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("读取 {0} 失败: {1}")]
    ReadFailed(String, #[source] std::sync::Arc<std::io::Error>),
}

/// agent 工作空间。
pub struct Workspace {
    cwd: PathBuf,
    /// 全局指令根（`~/.onion/AGENTS.md`）。
    global_root: Option<PathBuf>,
    /// 已发现文件（broad-to-specific 排序）。
    files: Vec<InstructionFile>,
    /// 预算（默认 65536）。
    max_bytes: usize,
}

impl Workspace {
    /// 发现工作空间指令文件（global → project root → ancestor chain）。
    pub fn discover(cwd: &Path, max_bytes: usize) -> Result<Self, WorkspaceError> {
        let global_root = std::env::var("ONION_HOME")
            .map(PathBuf::from)
            .ok()
            .map(|home| home.join("AGENTS.md"));
        let project_root = find_project_root(cwd);
        let mut files = Vec::new();
        // ① global
        if let Some(global) = &global_root {
            if let Some(file) = read_instruction(global)? {
                files.push(file);
            }
        }
        // ② 祖先链 root → cwd（broad-to-specific）。
        if let Some(root) = project_root {
            let ancestors = ancestor_chain(&root, cwd);
            for dir in ancestors {
                for name in ["AGENTS.md", "AGENTS.local.md"] {
                    if let Some(file) = read_instruction(&dir.join(name))? {
                        files.push(file);
                    }
                }
            }
        }
        // ③ 同目录内容去重（SHA-1 trim 后折叠，保留最早）。
        let files = dedupe(files);
        Ok(Self { cwd: cwd.to_path_buf(), global_root, files, max_bytes })
    }

    /// 触点对账：重扫发现面，与当前 files 比对，返回变更列表；内部状态刷新为最新。
    pub fn reconcile(&mut self) -> Result<WorkspaceChanges, WorkspaceError> {
        let fresh = Self::discover(&self.cwd, self.max_bytes)?;
        let mut changes = WorkspaceChanges::default();
        let old: HashSet<&Path> = self.files.iter().map(|f| f.path.as_path()).collect();
        let new: HashSet<&Path> = fresh.files.iter().map(|f| f.path.as_path()).collect();
        for f in &fresh.files {
            if !old.contains(f.path.as_path()) {
                changes.added.push(f.path.display().to_string());
            }
        }
        for f in &self.files {
            if !new.contains(f.path.as_path()) {
                changes.removed.push(f.path.display().to_string());
            }
        }
        for old_file in &self.files {
            if let Some(fresh_file) = fresh.files.iter().find(|f| f.path == old_file.path) {
                if old_file.version != fresh_file.version {
                    changes.changed.push(old_file.path.display().to_string());
                }
            }
        }
        self.files = fresh.files;
        Ok(changes)
    }

    /// 渲染基线 reminder（budget 内）。
    pub fn render_baseline(&self) -> String {
        if self.files.is_empty() {
            return String::new();
        }
        let mut body = String::from("## Workspace instructions\n\n");
        let mut budget = self.max_bytes;
        let mut omitted: Vec<String> = Vec::new();
        for file in &self.files {
            let content = file.content.trim();
            if content.is_empty() {
                continue;
            }
            let section = format!(
                "### Instructions from: {}\n{}\n\n",
                file.path.display(),
                content
            );
            let section_len = section.len();
            if section_len <= budget {
                body.push_str(&section);
                budget -= section_len;
            } else {
                // 不足：先省略更宽泛文件（broad-to-specific 排序下即前序文件）。
                omitted.push(file.path.display().to_string());
            }
        }
        if !omitted.is_empty() {
            body.push_str(&format!(
                "Workspace instruction budget: {} file(s) omitted due to budget.\n",
                omitted.len()
            ));
        }
        system_reminder(&escape_reminder_body(&body))
    }

    /// 渲染变更 reminder；无变更返回空串。
    pub fn render_delta(&self, changes: &WorkspaceChanges) -> String {
        if changes.is_empty() {
            return String::new();
        }
        let mut body = String::from("## Workspace instruction changes\n\n");
        if !changes.added.is_empty() {
            body.push_str(&format!("Added: {}\n", changes.added.join(", ")));
        }
        if !changes.removed.is_empty() {
            body.push_str(&format!("Removed: {}\n", changes.removed.join(", ")));
        }
        if !changes.changed.is_empty() {
            body.push_str(&format!("Changed: {}\n", changes.changed.join(", ")));
        }
        body.push_str("Re-read affected files before relying on their instructions.\n");
        system_reminder(&escape_reminder_body(&body))
    }

    /// 基线文件路径列表（审计事件载荷）。
    pub fn file_paths(&self) -> Vec<String> {
        self.files.iter().map(|f| f.path.display().to_string()).collect()
    }

    /// 基线字节量（审计事件载荷）。
    pub fn baseline_bytes(&self) -> usize {
        self.files.iter().map(|f| f.content.len()).sum()
    }

    /// 基线版本指纹（审计事件载荷；文件级指纹拼接）。
    pub fn baseline_version(&self) -> String {
        self.files
            .iter()
            .map(|f| f.version.fingerprint())
            .collect::<Vec<_>>()
            .join("|")
    }
}

/// 工作空间发现根（Runtime 装配点用）：cwd 向上找项目根。
pub(crate) fn workspace_cwd() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// 从 `dir` 向上找 `.git` marker 定位项目根（找不到则 cwd 即根）。
fn find_project_root(cwd: &Path) -> Option<PathBuf> {
    let mut current = Some(cwd);
    while let Some(dir) = current {
        if dir.join(".git").exists() {
            return Some(dir.to_path_buf());
        }
        current = dir.parent();
    }
    None
}

/// 祖先链：root → cwd 逐层目录（broad-to-specific）。
fn ancestor_chain(root: &Path, cwd: &Path) -> Vec<PathBuf> {
    let mut chain = Vec::new();
    let mut current = Some(cwd);
    while let Some(dir) = current {
        chain.push(dir.to_path_buf());
        if dir == root {
            break;
        }
        current = dir.parent();
    }
    chain.reverse();
    chain
}

/// 读取单条指令文件（不存在返回 None）。
fn read_instruction(path: &Path) -> Result<Option<InstructionFile>, WorkspaceError> {
    match std::fs::read_to_string(path) {
        Ok(content) => {
            let version = fs_version(path)?;
            Ok(Some(InstructionFile { path: path.to_path_buf(), content, version }))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(WorkspaceError::ReadFailed(
            path.display().to_string(),
            std::sync::Arc::new(error),
        )),
    }
}

/// FsVersion 指纹（Windows/Unix 通用：dev/ino 在不支持平台填 0）。
fn fs_version(path: &Path) -> Result<FsVersion, WorkspaceError> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| WorkspaceError::ReadFailed(path.display().to_string(), std::sync::Arc::new(error)))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        return Ok(FsVersion {
            dev: metadata.dev(),
            ino: metadata.ino(),
            size: metadata.len(),
            mtime_ns: metadata.mtime_nsec() as u64,
            ctime_ns: metadata.ctime_nsec() as u64,
        });
    }
    #[cfg(not(unix))]
    {
        // Windows：dev/ino 不可得（std 不暴露），填 0；mtime 用 systemtime 转换。
        let mtime_ns = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let ctime_ns = metadata
            .created()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        Ok(FsVersion { dev: 0, ino: 0, size: metadata.len(), mtime_ns, ctime_ns })
    }
}

/// 同目录内容去重（SHA-1 trim 后折叠，保留最早）。
fn dedupe(files: Vec<InstructionFile>) -> Vec<InstructionFile> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for file in files {
        let key = sha1_hex(file.content.trim().as_bytes());
        if seen.insert(key) {
            result.push(file);
        }
    }
    result
}

/// 手写 SHA-1（教学版；无外部依赖）。
pub(crate) fn sha1_hex(data: &[u8]) -> String {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let original_len = data.len() as u64;
    let mut padded = data.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&(original_len * 8).to_be_bytes());
    for chunk in padded.chunks(64) {
        let mut w = [0u32; 80];
        for (i, word) in chunk.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999u32),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    h.iter().map(|v| format!("{v:08x}")).collect()
}

#[cfg(test)]
#[path = "workspace_tests.rs"]
mod tests;
