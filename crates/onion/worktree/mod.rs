//! `create_worktree` 工具：从本地 commit / branch / tag 创建 git worktree 并
//! 切换会话工作目录。
//!
//! 语义：
//! - `name` 新分支名（同时作 checkout 目录名），省略自动生成唯一名；
//! - `from` 起始本地 commit / branch / tag，缺省 `HEAD`（不做 fetch）；
//! - checkout 落在 `<repositoryRoot>/.agents/worktrees/<name>`；
//! - 分支名非法 / 分支已存在 / 目录已存在 → fail-closed 报错（不覆盖）；
//! - 成功后切换会话 cwd 到新 checkout（权限根随之移动），append durable
//!   `WorktreeChange` 事件（last-write-wins 重放恢复）；
//! - 失败时已创建的 checkout 与分支保留（刻意设计，便于排查）；
//! - 离开 worktree 用 `working_directory` 的 `cd`；checkout 与分支保留。
//!
//! 只读文件系统之外的行为：本工具会创建目录与 git worktree（副作用），故标记
//! `WorkspaceWrite` 风险；可见性由当前 Agent 工具面决定（work 类工具）。

pub mod git;
pub mod seam;
pub mod tools;

pub use seam::{CreateWorktreeRequest, CreatedWorktree, WorktreeService, WORKTREES_DIR};
pub use tools::CreateWorktreeTool;

/// worktree 错误。
#[derive(Debug, thiserror::Error)]
pub enum WorktreeError {
    #[error("not a git repository at {0}")]
    NotARepository(String),
    #[error("git command failed ({cmd}): {stderr}")]
    GitFailed { cmd: String, stderr: String },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
