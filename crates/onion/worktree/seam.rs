//! `create_worktree` 的 seam：git worktree 创建服务 + 领域类型。
//!
//! 语义：
//! - `name` 新分支名（同时作 checkout 目录名），省略自动生成唯一名；
//! - `from` 起始本地 commit / branch / tag，缺省 `HEAD`（不做 fetch）；
//! - checkout 落在 `<repositoryRoot>/.agents/worktrees/<name>`；
//! - 分支名非法 / 分支已存在 / 目录已存在 → fail-closed 报错（不覆盖）；
//! - 成功后切换会话 cwd 到新 checkout，并 append durable `WorktreeChange` 事件；
//! - 失败时已创建的 checkout 与分支保留（不自动清理，便于排查）。

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::session_cwd::SessionCwd;
use crate::worktree::git;
use crate::worktree::WorktreeError;

/// worktree 存放目录名（`<repositoryRoot>/.agents/worktrees`）。
pub const WORKTREES_DIR: &str = ".agents/worktrees";
/// 自动生成分支名的最短 / 最长长度。
pub const MIN_AUTO_NAME_LEN: usize = 6;
pub const MAX_AUTO_NAME_LEN: usize = 24;
/// 自动生成分支名允许的字符集（路径防穿越：拒绝 `\` `:` `.` `..` 空段）。
const NAME_CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";

/// 一个已创建 worktree 的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedWorktree {
    /// canonical checkout 绝对路径。
    pub path: PathBuf,
    /// 新分支名。
    pub branch: String,
    /// 解析后的基线 commit 对象名。
    pub base_commit: String,
    /// 源 checkout 的顶层目录。
    pub repository_root: PathBuf,
}

/// `create_worktree` 请求（name / from 均可选）。
#[derive(Debug, Clone, Default)]
pub struct CreateWorktreeRequest {
    /// 新分支名 + checkout 目录名；省略自动生成唯一名。
    pub name: Option<String>,
    /// 起始本地 commit / branch / tag；缺省 `HEAD`。
    pub from: Option<String>,
}

/// `create_worktree` 工具内部状态：会话目录句柄 + 自增 id。
///
/// worktree 服务只创建，不持有目录状态：cwd 的读写全部经
/// `SessionCwd`，本服务不保留任何目录投影。
#[derive(Clone)]
pub struct WorktreeService {
    /// 会话目录句柄（取源 checkout；创建成功后经 `set` 切换）。
    cwd: Arc<SessionCwd>,
    /// 自动生成分支名的自增后缀（进程内唯一）。
    counter: Arc<AtomicU64>,
}

impl Default for WorktreeService {
    fn default() -> Self {
        Self::new()
    }
}

impl WorktreeService {
    /// 以独立 `SessionCwd` 构造（单会话 / 测试场景）。
    pub fn new() -> Self {
        Self::with_cwd(Arc::new(SessionCwd::new(PathBuf::new())))
    }

    /// 以共享的会话目录句柄构造（生产装配路径）。
    pub fn with_cwd(cwd: Arc<SessionCwd>) -> Self {
        Self {
            cwd,
            counter: Arc::new(AtomicU64::new(0)),
        }
    }

    /// 当前工作目录（绝对路径）。
    pub fn current(&self) -> PathBuf {
        self.cwd.current().as_ref().clone()
    }

    /// 延迟接线会话（宿主装配完成后调用）；重放恢复当前 cwd。
    pub fn wire_session(&self, session: Arc<crate::session::SessionLog>) {
        self.cwd.wire_session(session);
    }

    /// 会话目录句柄（供装配期共享）。
    pub fn session_cwd(&self) -> Arc<SessionCwd> {
        self.cwd.clone()
    }

    /// 校验 `name`：非空、相对路径、不以 `-` 开头、无路径分隔/穿越段。
    ///
    /// 路径防穿越：拒绝 `\` `:` `.` `..`
    /// 空段，禁止以 `-` 开头（避免被 git 当作选项）。
    fn validate_name(name: &str) -> Result<(), String> {
        if name.trim().is_empty() {
            return Err("name must be a non-empty string".to_string());
        }
        if name.starts_with('-') {
            return Err(format!("name must not start with '-': {name}"));
        }
        if name.contains(['\\', ':']) {
            return Err(format!("name must be a single path segment: {name}"));
        }
        for part in name.split('/') {
            if part.is_empty() || part == "." || part == ".." {
                return Err(format!("invalid name segment: {name}"));
            }
        }
        Ok(())
    }

    /// 生成唯一分支名：`onion-<8 位随机串>`（自动命名唯一性）。
    fn generate_name(&self) -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let counter = self.counter.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let mut out = String::with_capacity(MIN_AUTO_NAME_LEN + 8);
        out.push_str("onion-");
        let mut seed = (nanos as u64) ^ (counter.wrapping_mul(0x9E3779B97F4A7C15));
        while out.len() < MIN_AUTO_NAME_LEN + 6 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            out.push(NAME_CHARS[(seed >> 33) as usize % NAME_CHARS.len()] as char);
        }
        out
    }

    /// 创建 worktree 并切换会话 cwd（核心流程）。
    pub async fn create(&self, req: CreateWorktreeRequest) -> Result<CreatedWorktree, String> {
        if !self.cwd.is_wired() {
            return Err("create_worktree requires an owning agent session".to_string());
        }
        let source_cwd = self.cwd.ensure();
        if !source_cwd.is_dir() {
            return Err(format!(
                "current working directory does not exist: {}",
                source_cwd.display()
            ));
        }

        // 解析主仓库根（源 checkout 顶层目录）。
        let repository_root = git::find_repository_root(&source_cwd)
            .await
            .map_err(|e| e.to_string())?;
        let repository_root = PathBuf::from(repository_root);
        let repository_root_str = repository_root.to_string_lossy().to_string();

        // 确定分支名 + 校验。
        let branch = match &req.name {
            Some(n) => n.clone(),
            None => self.generate_name(),
        };
        Self::validate_name(&branch)?;
        if !git::check_ref_format(&branch, &repository_root).await {
            return Err(format!("invalid branch name: {branch}"));
        }
        // 分支已存在 → 报错。
        if git::branch_exists(&branch, &repository_root).await {
            return Err(format!("worktree branch already exists: {branch}"));
        }

        // 解析基线 commit（缺省 HEAD，不做 fetch）。
        let from = req.from.as_deref().unwrap_or("HEAD");
        let base_commit = git::resolve_base(from, &repository_root)
            .await
            .map_err(|e| e.to_string())?;

        // 计算目标路径 + 防重复。
        let pool = repository_root.join(WORKTREES_DIR);
        let destination = pool.join(&branch);
        if destination.exists() {
            return Err(format!("worktree path already exists: {}", destination.display()));
        }

        // 创建目录 + `.gitignore`（隔离 worktree 目录不被 git 跟踪）。
        std::fs::create_dir_all(&destination).map_err(WorktreeError::Io).map_err(|e| e.to_string())?;
        let gitignore = pool.join(".gitignore");
        if !gitignore.exists() {
            let _ = std::fs::write(&gitignore, "*\n");
        }

        // 真正创建 worktree（branch 已校验唯一，用 -b；失败保留已建目录）。
        let create_result = git::add_worktree(&repository_root, &branch, &destination, &base_commit).await;
        if let Err(e) = create_result {
            return Err(format!(
                "Could not finish worktree {branch} at {}. Any created checkout and branch are retained. {e}",
                destination.display()
            ));
        }
        // 用 `reset --hard` 填充工作树。
        if let Err(e) = git::reset_hard(&destination, &base_commit).await {
            return Err(format!(
                "Could not finish worktree {branch} at {}. Any created checkout and branch are retained. {e}",
                destination.display()
            ));
        }

        // 切换会话 cwd 到新 checkout（唯一写入口：写 durable 事件 + 更新投影）。
        let canonical = destination
            .canonicalize()
            .unwrap_or_else(|_| destination.clone());
        self.cwd.set_worktree(canonical.clone())?;

        Ok(CreatedWorktree {
            path: canonical,
            branch,
            base_commit,
            repository_root: PathBuf::from(repository_root_str),
        })
    }
}

#[cfg(test)]
#[path = "seam_tests.rs"]
mod tests;
