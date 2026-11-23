//! `session_cwd` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::sync::Arc;

use super::SessionCwd;
use crate::session::{SessionEvent, SessionLog};

fn temp_root(tag: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("onion_cwd_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap_or_else(|error| panic!("create temp root failed: {error}"));
    root
}

#[test]
fn initial_cwd_is_root() {
    let root = temp_root("init");
    let cwd = SessionCwd::new(root.clone());
    assert_eq!(cwd.root(), root.as_path());
    assert_eq!(cwd.current().as_ref(), &root);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn resolve_uses_current_not_root() {
    let root = temp_root("resolve");
    std::fs::create_dir_all(root.join("sub")).unwrap();
    let cwd = SessionCwd::new(root.clone());
    cwd.set(root.join("sub")).unwrap();
    // 相对路径基于活动目录解析（worktree 隔离的核心语义）。
    assert_eq!(cwd.resolve("a.txt").unwrap(), root.join("sub").join("a.txt"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn resolve_accepts_absolute_and_parent_inside_root() {
    let root = temp_root("inside");
    std::fs::create_dir_all(root.join("sub")).unwrap();
    let cwd = SessionCwd::new(root.clone());
    // 根内绝对路径直接可用（工具返回值 / 宿主拼出的路径）。
    let absolute = root.join("sub").join("a.txt");
    assert_eq!(cwd.resolve(absolute.to_str().unwrap()).unwrap(), absolute);
    // `..` 允许，归一化后仍落在根内即可。
    let nested = root.join("sub").join("..").join("b.txt");
    assert_eq!(cwd.resolve(nested.to_str().unwrap()).unwrap(), root.join("b.txt"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn resolve_rejects_paths_outside_writable_roots() {
    // 可写根 = 权限根 + 系统临时目录；两者之外仍然拒绝（绝对与相对 `..` 逃逸同判）。
    // 测试根位于系统 temp 之下，因此相对路径需两层 `..` 才能越过 temp。
    let root = temp_root("reject");
    let cwd = SessionCwd::new(root.clone());
    let outside = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("outside_reject.txt");
    let err = cwd.resolve(outside.to_str().unwrap()).unwrap_err();
    assert!(err.contains("outside the permission root"), "got: {err}");
    let ups = "../".repeat(root.components().count() + 2);
    let err = cwd.resolve(format!("{ups}elsewhere.txt").as_str()).unwrap_err();
    assert!(err.contains("outside the permission root"), "got: {err}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn resolve_allows_system_temp_for_writes() {
    // 可写根 = 权限根 + 系统临时目录（`writableRoots = [workspaceRoot, tmpdir()]`）：
    // 系统临时目录是合法写
    // 目标（脚本 / 草稿落盘场景）；会话根位于 temp 之下时，相对 `..` 逃逸进
    // temp 同样合法。
    let root = temp_root("temp_allow");
    let cwd = SessionCwd::new(root.clone());
    let temp = std::env::temp_dir();
    let scratch = temp.join("ava_onion_scratch.txt");
    assert_eq!(cwd.resolve(scratch.to_str().unwrap()).unwrap(), scratch);
    assert_eq!(
        cwd.resolve("../ava_onion_scratch.txt").unwrap(),
        scratch
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn resolve_read_allows_paths_outside_root() {
    // 读类解析不做权限根校验（cwd 是解析默认值，不是遏制边界）：
    // 数据根内的 spill 日志 / 系统记忆等由此可读；写侧仍走 resolve 的 root 遏制。
    let root = temp_root("read_allow");
    let cwd = SessionCwd::new(root.clone());
    let outside = std::env::temp_dir().join("outside_read.txt");
    assert_eq!(
        cwd.resolve_read(outside.to_str().unwrap()).unwrap(),
        outside
    );
    assert_eq!(
        cwd.resolve_read("../outside_read.txt").unwrap(),
        std::env::temp_dir().join("outside_read.txt")
    );
    // 相对路径仍基于当前有效目录。
    assert_eq!(cwd.resolve_read("a/b.txt").unwrap(), root.join("a").join("b.txt"));
    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(windows)]
#[test]
fn resolve_tolerates_windows_case_difference() {
    let root = temp_root("case");
    let cwd = SessionCwd::new(root.clone());
    // Windows 路径大小写不敏感：小写盘符 / 目录名不应被误判为越界。
    let lowered = root.join("a.txt").to_string_lossy().to_lowercase();
    let resolved = cwd
        .resolve(&lowered)
        .unwrap_or_else(|error| panic!("lowercase path should resolve: {error}"));
    assert!(resolved.ends_with("a.txt"), "got: {}", resolved.display());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn set_rejects_outside_root_and_missing_dir() {
    let root = temp_root("set_reject");
    let cwd = SessionCwd::new(root.clone());
    let outside = std::env::temp_dir();
    let err = cwd.set(outside).unwrap_err();
    assert!(err.contains("outside the permission root"), "got: {err}");
    let err = cwd.set(root.join("missing")).unwrap_err();
    assert!(err.contains("does not exist"), "got: {err}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn set_appends_durable_event() {
    let root = temp_root("set_event");
    std::fs::create_dir_all(root.join("sub")).unwrap();
    let cwd = SessionCwd::new(root.clone());
    let session = Arc::new(SessionLog::in_memory());
    cwd.wire_session(session.clone());
    cwd.set(root.join("sub")).unwrap();
    let count = session
        .snapshot()
        .into_iter()
        .filter(|e| matches!(e, SessionEvent::WorkingDir { .. }))
        .count();
    assert_eq!(count, 1);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn ensure_falls_back_to_root_when_missing() {
    let root = temp_root("ensure");
    std::fs::create_dir_all(root.join("sub")).unwrap();
    let cwd = SessionCwd::new(root.clone());
    let session = Arc::new(SessionLog::in_memory());
    cwd.wire_session(session.clone());
    cwd.set(root.join("sub")).unwrap();
    std::fs::remove_dir_all(root.join("sub")).unwrap();
    assert_eq!(cwd.ensure(), root);
    // 回退也写 durable 事件（重放可恢复）。
    let last = session.snapshot().into_iter().rev().find_map(|e| match e {
        SessionEvent::WorkingDir { cwd } => Some(cwd),
        _ => None,
    });
    assert_eq!(last.as_deref(), root.to_str());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn wire_replays_last_of_both_event_kinds() {
    let root = temp_root("replay");
    std::fs::create_dir_all(root.join("a")).unwrap();
    std::fs::create_dir_all(root.join("b")).unwrap();
    let session = Arc::new(SessionLog::in_memory());
    session.append(SessionEvent::WorkingDir {
        cwd: root.join("a").to_string_lossy().to_string(),
    });
    session.append(SessionEvent::WorktreeChange {
        path: root.join("b").to_string_lossy().to_string(),
    });
    let cwd = SessionCwd::new(root.clone());
    cwd.wire_session(session);
    // 两类事件同源，按事件序 last-write-wins。
    assert_eq!(cwd.current().as_ref(), &root.join("b"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn wire_falls_back_when_replayed_outside_root() {
    let root = temp_root("replay_outside");
    let session = Arc::new(SessionLog::in_memory());
    session.append(SessionEvent::WorkingDir {
        cwd: std::env::temp_dir().to_string_lossy().to_string(),
    });
    let cwd = SessionCwd::new(root.clone());
    cwd.wire_session(session);
    assert_eq!(cwd.current().as_ref(), &root);
    let _ = std::fs::remove_dir_all(&root);
}
