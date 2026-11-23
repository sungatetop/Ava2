//! `config` 模块单元测试：沙箱 exe 自动识别（sidecar 同目录布局）。

use std::path::PathBuf;

use super::sandbox_exe_in;

/// 临时目录（按测试名隔离，结束清理，避免污染真实数据目录）。
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ava_onion_config_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// 空目录：未放置沙箱 exe → None（直通）。
#[test]
fn sandbox_exe_in_returns_none_when_absent() {
    let dir = temp_dir("absent");
    assert_eq!(sandbox_exe_in(&dir), None);
    let _ = std::fs::remove_dir_all(&dir);
}

/// 同目录存在 onion-sandbox → 识别为绝对路径（sidecar 布局）。
#[test]
fn sandbox_exe_in_detects_sibling() {
    let dir = temp_dir("present");
    let name = if cfg!(windows) { "onion-sandbox.exe" } else { "onion-sandbox" };
    std::fs::write(dir.join(name), b"stub").expect("write stub");
    assert_eq!(sandbox_exe_in(&dir), Some(dir.join(name)));
    let _ = std::fs::remove_dir_all(&dir);
}

/// 同名目录不是文件 → 不识别（is_file 契约）。
#[test]
fn sandbox_exe_in_rejects_directory() {
    let dir = temp_dir("isdir");
    let name = if cfg!(windows) { "onion-sandbox.exe" } else { "onion-sandbox" };
    std::fs::create_dir_all(dir.join(name)).expect("create dir stub");
    assert_eq!(sandbox_exe_in(&dir), None);
    let _ = std::fs::remove_dir_all(&dir);
}
