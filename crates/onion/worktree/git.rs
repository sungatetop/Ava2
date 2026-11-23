//! git 子进程封装 — `create_worktree` 的底层 git 操作。
//!
//! 约定：所有 git 调用
//! 都带 `-C <绝对路径>`（不依赖进程 cwd）、设置 `GIT_TERMINAL_PROMPT=0` /
//! `GIT_ASKPASS=''` 防凭据弹窗挂起、stdin 忽略。每个命令返回结构化错误，
//! 供上层 fail-closed。

use std::path::Path;
use std::process::Stdio;

use crate::worktree::WorktreeError;

/// 运行一个 git 命令，返回 `(exit_code, stdout)`。
///
/// 不抛错：git 退出非零时返回 exit_code，由调用方决定 fail-open / fail-closed。
async fn git(args: &[&str], cwd: &Path) -> Result<(i32, String), WorktreeError> {
    let mut cmd = tokio::process::Command::new("git");
    cmd.args(args)
        .current_dir(cwd)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let out = cmd.output().await.map_err(WorktreeError::Io)?;
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    Ok((code, stdout))
}

/// 校验分支名合法：`git check-ref-format refs/heads/<branch>`（exit 0 = 合法）。
pub async fn check_ref_format(branch: &str, cwd: &Path) -> bool {
    let refname = format!("refs/heads/{branch}");
    let (code, _) = git(&["check-ref-format", &refname], cwd).await.unwrap_or((-1, String::new()));
    code == 0
}

/// 解析主仓库根：`git rev-parse --show-toplevel`。
pub async fn find_repository_root(cwd: &Path) -> Result<String, WorktreeError> {
    let (code, stdout) = git(&["rev-parse", "--show-toplevel"], cwd).await?;
    if code != 0 {
        return Err(WorktreeError::NotARepository(cwd.display().to_string()));
    }
    Ok(stdout.trim().to_string())
}

/// 解析基线 commit：`git rev-parse --verify <revision>^{commit}`。
///
/// `from` 缺省 `HEAD`，不做 fetch。
pub async fn resolve_base(revision: &str, cwd: &Path) -> Result<String, WorktreeError> {
    let arg = format!("{revision}^{{commit}}");
    let (code, stdout) = git(&["rev-parse", "--verify", &arg], cwd).await?;
    if code != 0 {
        return Err(WorktreeError::GitFailed {
            cmd: format!("rev-parse {revision}"),
            stderr: stdout,
        });
    }
    Ok(stdout.trim().to_string())
}

/// 检查分支是否已存在：`git show-ref --verify --quiet refs/heads/<branch>`。
///
/// exit 0 = 已存在（调用方报错）。
pub async fn branch_exists(branch: &str, cwd: &Path) -> bool {
    let refname = format!("refs/heads/{branch}");
    let (code, _) = git(&["show-ref", "--verify", "--quiet", &refname], cwd)
        .await
        .unwrap_or((-1, String::new()));
    code == 0
}

/// 创建 worktree：`git worktree add --no-checkout -b <branch> -- <path> <base>`。
///
/// `--no-checkout`（checkout 由后续 `reset --hard` 填充），`-b` 而非 `-B`
/// （已存在分支在前面已被拒绝）。
pub async fn add_worktree(
    repo_root: &Path,
    branch: &str,
    path: &Path,
    base: &str,
) -> Result<(), WorktreeError> {
    let root = repo_root.to_str().unwrap_or("");
    let worktree_path = path.to_str().unwrap_or("");
    let (code, stdout) = git(
        &[
            "-C",
            root,
            "worktree",
            "add",
            "--no-checkout",
            "-b",
            branch,
            "--",
            worktree_path,
            base,
        ],
        repo_root,
    )
    .await?;
    if code != 0 {
        return Err(WorktreeError::GitFailed {
            cmd: format!("worktree add {branch}"),
            stderr: stdout,
        });
    }
    Ok(())
}

/// 在新 checkout 上执行 `git reset --hard <base>` 填充工作树。
///
/// 不改变仓库配置，仅填充本次 checkout。
pub async fn reset_hard(worktree_path: &Path, base: &str) -> Result<(), WorktreeError> {
    let (code, stdout) = git(&["reset", "--hard", base], worktree_path).await?;
    if code != 0 {
        return Err(WorktreeError::GitFailed {
            cmd: "reset --hard".into(),
            stderr: stdout,
        });
    }
    Ok(())
}
