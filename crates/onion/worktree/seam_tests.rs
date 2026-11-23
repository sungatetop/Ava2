//! `seam` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::path::PathBuf;
use std::process::Command;

use crate::worktree::seam::{CreateWorktreeRequest, WorktreeService, WORKTREES_DIR};
use crate::worktree::WorktreeError;

/// git 是否可用（集成测试门控）。
fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// 在临时目录初始化一个 git 仓库并提交一次，返回临时目录。
fn init_git_repo() -> Option<PathBuf> {
    if !git_available() {
        return None;
    }
    let dir = std::env::temp_dir().join(format!(
        "onion_worktree_it_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).ok()?;
    let run = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(&dir)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    };
    if !run(&["init", "-q", "-b", "main"]) {
        return None;
    }
    if !run(&["config", "user.email", "test@onion"]) || !run(&["config", "user.name", "onion test"]) {
        return None;
    }
    std::fs::write(dir.join("README.md"), "hi\n").ok()?;
    if !run(&["add", "."]) || !run(&["commit", "-q", "-m", "init"]) {
        return None;
    }
    Some(dir)
}

fn cleanup(repo: &PathBuf) {
    // 在 repo 根执行 `git worktree remove --force` 清理 + 删除临时目录（忽略失败）。
    let _ = Command::new("git")
        .args(["worktree", "remove", "--force", repo.join(WORKTREES_DIR).to_str().unwrap_or("")])
        .current_dir(repo)
        .output();
    let _ = std::fs::remove_dir_all(repo);
}

#[tokio::test]
async fn create_worktree_end_to_end() {
    let Some(repo) = init_git_repo() else {
        eprintln!("git 不可用，跳过集成测试");
        return;
    };
    // 用真实会话日志 wire（create 需要 owning session）。
    let sessions_dir = std::env::temp_dir().join(format!("onion_worktree_sess_{}", std::process::id()));
    let session = std::sync::Arc::new(
        crate::session::SessionLog::open(&sessions_dir).expect("open session"),
    );
    // 源 cwd = 仓库根（权限根与初始 cwd 同为仓库根）。
    let cwd = std::sync::Arc::new(crate::session_cwd::SessionCwd::new(repo.clone()));
    cwd.wire_session(session);
    let svc = WorktreeService::with_cwd(cwd);

    let created = svc
        .create(CreateWorktreeRequest { name: Some("feature-x".into()), from: Some("HEAD".into()) })
        .await
        .expect("create_worktree 成功");
    assert_eq!(created.branch, "feature-x");
    assert_eq!(created.repository_root, repo);
    // checkout 落在 <repo>/.agents/worktrees/feature-x。
    let expected = repo.join(WORKTREES_DIR).join("feature-x");
    assert_eq!(created.path, expected.canonicalize().unwrap_or(expected.clone()));
    assert!(created.path.join("README.md").exists(), "worktree 应含检出文件");
    // 会话 cwd 已切到新 checkout。
    assert_eq!(svc.current(), created.path);

    cleanup(&repo);
    let _ = std::fs::remove_dir_all(&sessions_dir);
}

#[test]
fn generate_name_produces_valid_slug() {
    let svc = WorktreeService::new();
    let name = svc.generate_name();
    assert!(name.starts_with("onion-"));
    assert!(name.len() >= "onion-".len() + 6);
    assert!(name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
}

#[test]
fn generate_name_unique_across_calls() {
    let svc = WorktreeService::new();
    let a = svc.generate_name();
    let b = svc.generate_name();
    assert_ne!(a, b);
}

#[test]
fn request_defaults_optional_fields() {
    let req = CreateWorktreeRequest::default();
    assert!(req.name.is_none());
    assert!(req.from.is_none());
}

#[test]
fn worktrees_dir_is_relative_under_repo() {
    // WORKTREES_DIR 是相对路径片段，拼在 repository root 下。
    let root = PathBuf::from("/repo");
    let pool = root.join(WORKTREES_DIR);
    assert!(pool.starts_with(&root));
    assert_eq!(pool, root.join(".agents/worktrees"));
}

#[test]
fn error_variants_are_stringified() {
    let e = WorktreeError::NotARepository("x".into());
    assert!(e.to_string().contains("not a git repository"));
    let e = WorktreeError::GitFailed { cmd: "worktree add".into(), stderr: "boom".into() };
    assert!(e.to_string().contains("git command failed"));
}
