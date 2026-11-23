//! `spill::cleanup` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//!
//! 不伪造 mtime（std 无可移植写接口）：以「截止时间取当前时间 ±1 天」把
//! 「过期 / 未过期」两侧都覆盖到——刚创建的文件 mtime 介于二者之间。

use std::fs;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, SystemTime};

use super::{cleanup_cutoff, sweep_spill_root, SweepStats};

fn temp_root(tag: &str) -> std::path::PathBuf {
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("onion_spill_cleanup_{tag}_{}_{}", std::process::id(), seq))
}

#[test]
fn cleanup_cutoff_subtracts_days() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
    assert_eq!(cleanup_cutoff(now, 0), now);
    assert_eq!(cleanup_cutoff(now, 30), now - Duration::from_secs(30 * 86_400));
}

#[test]
fn sweep_with_future_cutoff_removes_all_and_prunes_dirs() {
    let root = temp_root("expired");
    let a = root.join("sessions").join("session-a");
    let b = root.join("sessions").join("session-b");
    fs::create_dir_all(&a).unwrap();
    fs::create_dir_all(&b).unwrap();
    fs::write(a.join("old.txt"), "x".repeat(100)).unwrap();
    fs::write(b.join("older.txt"), "y".repeat(10)).unwrap();

    // 截止时间取未来 → 全部过期。
    let cutoff = SystemTime::now() + Duration::from_secs(86_400);
    let stats = sweep_spill_root(&root, cutoff).unwrap();
    assert_eq!(
        stats,
        SweepStats {
            scanned_session_dirs: 2,
            removed_files: 2,
            removed_bytes: 110,
            pruned_dirs: 2,
        }
    );
    assert!(!root.join("sessions").join("session-a").exists());
    assert!(!root.join("sessions").join("session-b").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn sweep_with_past_cutoff_keeps_files_and_dirs() {
    let root = temp_root("fresh");
    let dir = root.join("sessions").join("fresh-session");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("keep.txt"), "z").unwrap();

    // 截止时间取过去 → 刚创建的文件全部保留。
    let cutoff = SystemTime::now() - Duration::from_secs(86_400);
    let stats = sweep_spill_root(&root, cutoff).unwrap();
    assert_eq!(
        stats,
        SweepStats {
            scanned_session_dirs: 1,
            removed_files: 0,
            removed_bytes: 0,
            pruned_dirs: 0,
        }
    );
    assert!(dir.join("keep.txt").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn sweep_missing_root_is_noop() {
    let root = temp_root("missing");
    assert_eq!(sweep_spill_root(&root, SystemTime::now()).unwrap(), SweepStats::default());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn sweep_skips_subdirs_and_top_level_files() {
    let root = temp_root("skip");
    let dir = root.join("sessions").join("mixed");
    let nested = dir.join("nested");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("inner.txt"), "i").unwrap();
    fs::write(root.join("sessions").join("loose.txt"), "l").unwrap();

    // 子目录与顶层散文件一律跳过；会话目录因子目录存活不修剪。
    let cutoff = SystemTime::now() + Duration::from_secs(86_400);
    let stats = sweep_spill_root(&root, cutoff).unwrap();
    assert_eq!(stats.scanned_session_dirs, 1);
    assert_eq!(stats.removed_files, 0);
    assert_eq!(stats.pruned_dirs, 0);
    assert!(nested.join("inner.txt").exists());
    assert!(root.join("sessions").join("loose.txt").exists());
    let _ = fs::remove_dir_all(root);
}
