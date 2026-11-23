//! 会话有效工作目录的唯一权威（WorkingDirectoryService 语义）。
//!
//! 三层职责分离（见 docs/onion/reports/onion-server-concurrency-and-worktree-isolation-analysis.md §2.1）：
//! - **权限根 `root`**：装配期固定，永不改变（durable session `header.cwd`）；
//! - **有效目录 `cwd`**：可在根内移动，是相对路径的解析基准；
//! - **durable 事件**：`set` 是唯一写入口，`wire_session` 统一重放恢复。
//!
//! 所有工具（文件 / shell / LSP / worktree / working_directory / 子代理）共享同一个
//! `Arc<SessionCwd>` 实例——这是 worktree 隔离成立的结构性前提。

use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, OnceLock};

use arc_swap::ArcSwap;

use crate::session::{SessionEvent, SessionLog};

/// 会话有效工作目录的唯一权威：权限根恒定，cwd 可在根内移动。
pub struct SessionCwd {
    /// 权限根（沙箱 root）：装配期固定，永不改变。
    root: PathBuf,
    /// 系统临时目录：写类路径的第二可写根（`writableRoots = [workspaceRoot, tmpdir()]`）。
    temp_root: PathBuf,
    /// 当前有效目录：`ArcSwap` 读多写少，避免每次工具调用抢锁。
    cwd: ArcSwap<PathBuf>,
    /// 会话接线槽（durable 事件写入 + 重放恢复）。
    session: OnceLock<Arc<SessionLog>>,
}

impl SessionCwd {
    /// 构造：初始 cwd = 权限根。
    pub fn new(root: PathBuf) -> Self {
        Self {
            temp_root: normalize(&std::env::temp_dir()),
            cwd: ArcSwap::from_pointee(root.clone()),
            root,
            session: OnceLock::new(),
        }
    }

    /// 构造：指定初始 cwd（子会话委派专用——初始目录在派生时固定，不写
    /// `WorkingDir` 事件，故不参与父会话的 durable 重放）。
    ///
    /// `initial` 必须落在 `root` 内，否则回退到 `root`（调用方应先用
    /// [`SessionCwd::resolve`] 校验并给出明确错误）。
    pub fn new_at(root: PathBuf, initial: PathBuf) -> Self {
        let start = if within(&initial, &root) { initial } else { root.clone() };
        Self {
            temp_root: normalize(&std::env::temp_dir()),
            cwd: ArcSwap::from_pointee(start),
            root,
            session: OnceLock::new(),
        }
    }

    /// 权限根（只读，恒定）。
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 当前有效目录（无锁读）。
    pub fn current(&self) -> Arc<PathBuf> {
        self.cwd.load_full()
    }

    /// 路径 → 可写根内绝对路径：相对路径基于当前有效目录，绝对路径直接使用；
    /// 结果必须落在**可写根**内。
    ///
    /// 可写根 = 权限根 + 系统临时目录（`writableRoots = [workspaceRoot,
    /// tmpdir()]`——系统 temp 是脚本 / 草稿等临时产物的合法落点）。绝对路径与 `..`
    /// 都**允许**，但一律先做词法归一化（消解 `.` / `..`）再校验最终位置：可写根内
    /// 的绝对路径（工具返回值、宿主拼出的路径、模型复述的路径）应当可直接使用；
    /// 可写根之外的绝对路径与逃逸出可写根的 `..` 仍然拒绝（`ctx.fs` 的
    /// 「root 限定」语义——限制的是**最终位置**，不是表述形式）。
    pub fn resolve(&self, input: &str) -> Result<PathBuf, String> {
        let path = normalize(&self.current().join(input));
        if !within(&path, &self.root) && !within(&path, &self.temp_root) {
            return Err(format!(
                "path outside the permission root is not allowed: {input}"
            ));
        }
        Ok(path)
    }

    /// 读类路径解析：相对路径基于当前有效目录，绝对路径直接使用，词法归一化后返回。
    ///
    /// **不做权限根校验**——`ctx.fs` 语义：「cwd 是解析默认值，不是遏制
    /// 边界；遏制只落在 write/edit 两个 mutation，读在所有模式下放行」。数据根内的
    /// 文件（shell 溢出日志 `<data_home>/logs/jobs/*.log`、系统记忆等）由此可读；
    /// 写入侧仍走 [`SessionCwd::resolve`] 的可写根遏制（权限根 + 系统临时目录）。
    pub fn resolve_read(&self, input: &str) -> Result<PathBuf, String> {
        Ok(normalize(&self.current().join(input)))
    }

    /// 校验并提交变更：写 durable 事件 → 更新 cwd（先落盘后生效）。
    ///
    /// 目标须已存在且为目录，且落在权限根内；否则拒绝。
    pub fn set(&self, path: PathBuf) -> Result<PathBuf, String> {
        self.commit(path, None)
    }

    /// 同 `set`，但以 `WorktreeChange` 事件记录（worktree 创建专用，保留 transcript 语义）。
    pub fn set_worktree(&self, path: PathBuf) -> Result<PathBuf, String> {
        self.commit(path, Some(true))
    }

    /// 提交变更的唯一实现：`worktree = Some(true)` 写 `WorktreeChange`，否则写 `WorkingDir`。
    fn commit(&self, path: PathBuf, worktree: Option<bool>) -> Result<PathBuf, String> {
        if !within(&path, &self.root) {
            return Err(format!(
                "working directory outside the permission root is not allowed: {}",
                path.display()
            ));
        }
        if !path.is_dir() {
            return Err(format!(
                "working directory does not exist: {}",
                path.display()
            ));
        }
        // 不做 `canonicalize`：Windows 会加 `\\?\` 前缀，破坏与 root 的前缀比较；
        // 需要规范化的调用方（如 worktree）自行 canonicalize 后再传入。
        let canonical = path;
        if let Some(session) = self.session.get() {
            let text = canonical.to_string_lossy().to_string();
            session.append(match worktree {
                Some(true) => SessionEvent::WorktreeChange { path: text },
                _ => SessionEvent::WorkingDir { cwd: text },
            });
        }
        self.cwd.store(Arc::new(canonical.clone()));
        Ok(canonical)
    }

    /// 目录消失时回退到权限根并写事件。
    ///
    /// 返回当前可用目录；回退发生时已提交 durable 事件。
    pub fn ensure(&self) -> PathBuf {
        let current = self.current();
        if current.is_dir() {
            return current.as_ref().clone();
        }
        let root = self.root.clone();
        if let Some(session) = self.session.get() {
            session.append(SessionEvent::WorkingDir {
                cwd: root.to_string_lossy().to_string(),
            });
        }
        self.cwd.store(Arc::new(root.clone()));
        root
    }

    /// 是否已接线会话（未接线时变更不落 durable 事件）。
    pub fn is_wired(&self) -> bool {
        self.session.get().is_some()
    }

    /// 装配后接线：重放 `WorkingDir` / `WorktreeChange` 两类事件（按事件序 last-write-wins）。
    ///
    /// 重放结果若已不在权限根内（历史数据异常），回退到权限根。
    pub fn wire_session(&self, session: Arc<SessionLog>) {
        let _ = self.session.set(session.clone());
        let replayed = replay_cwd(&session.snapshot()).unwrap_or_else(|| self.root.clone());
        let restored = if within(&replayed, &self.root) {
            replayed
        } else {
            self.root.clone()
        };
        self.cwd.store(Arc::new(restored));
    }
}

/// 从 durable 事件流重放最近一次目录变更（`WorkingDir` 与 `WorktreeChange` 同源）。
fn replay_cwd(events: &[SessionEvent]) -> Option<PathBuf> {
    events.iter().rev().find_map(|event| match event {
        SessionEvent::WorkingDir { cwd } => Some(PathBuf::from(cwd)),
        SessionEvent::WorktreeChange { path } => Some(PathBuf::from(path)),
        _ => None,
    })
}

/// 词法归一化：消解 `.` / `..`，不触碰文件系统（不做 `canonicalize`）。
///
/// `..` 在根上弹不动（`pop` 无效果），越界由调用方的 `within` 兜底判定。
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// 路径是否落在权限根内（容忍 Windows `canonicalize` 产生的 `\\?\` 前缀差异）。
fn within(path: &Path, root: &Path) -> bool {
    let path = strip_verbatim(path);
    let root = strip_verbatim(root);
    if path.starts_with(&root) {
        return true;
    }
    // Windows 路径大小写不敏感：`c:\workspace\x` 与 `C:\workspace\X` 是同一位置，
    // 逐组件忽略大小写比较，避免小写盘符 / 不同大小写目录名被误判为越界。
    if cfg!(windows) {
        let path: Vec<String> = path
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect();
        let root: Vec<String> = root
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect();
        return root.len() <= path.len()
            && root
                .iter()
                .zip(path.iter())
                .all(|(root_part, path_part)| root_part.eq_ignore_ascii_case(path_part));
    }
    false
}

/// 去掉 Windows verbatim 前缀（`\\?\` / `\\?\UNC\`），其余平台原样返回。
fn strip_verbatim(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    match text.strip_prefix(r"\\?\") {
        Some(rest) => PathBuf::from(rest),
        None => path.to_path_buf(),
    }
}

#[cfg(test)]
#[path = "session_cwd_tests.rs"]
mod tests;
