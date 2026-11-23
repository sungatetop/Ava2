//! `session_lock` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;
use std::fs;
use std::path::PathBuf;

/// 临时会话目录（测试结束清理，不污染真实数据目录）。
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ava_onion_lease_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("建临时目录");
    dir
}

#[test]
fn second_acquire_conflicts_then_reacquire_after_release() {
    let dir = temp_dir("contention");
    let lease = SessionWriteLease::acquire(&dir).expect("首次获取应成功");
    assert!(
        matches!(SessionWriteLease::acquire(&dir), Err(SessionLeaseError::AlreadyOwned)),
        "持有期间第二次获取必须冲突"
    );
    drop(lease);
    let reacquired = SessionWriteLease::acquire(&dir);
    assert!(reacquired.is_ok(), "释放后应可重新获取：{reacquired:?}");
    // 末尾释放后再清目录，避免 Windows 上句柄未关导致目录删除失败。
    drop(reacquired);
    assert!(fs::remove_dir_all(&dir).is_ok(), "锁文件不阻塞目录删除（share DELETE / close）");
}

#[test]
fn lease_file_is_created_and_content_free() {
    let dir = temp_dir("content_free");
    let lease = SessionWriteLease::acquire(&dir).expect("获取租约");
    let path = dir.join(SessionWriteLease::FILE_NAME);
    assert!(path.exists(), "锁文件应被创建");
    assert_eq!(fs::read(&path).expect("读锁文件"), Vec::<u8>::new(), "锁文件应为空");
    drop(lease);
    let _ = fs::remove_dir_all(&dir);
}
