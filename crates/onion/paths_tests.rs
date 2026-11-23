//! `paths` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;
use std::path::PathBuf;

#[test]
fn env_override_wins() {
    assert_eq!(
        data_home_from(Some("D:/custom-root"), Some(Path::new("/home/u"))),
        PathBuf::from("D:/custom-root")
    );
}

#[test]
fn blank_env_treated_as_unset() {
    assert_eq!(
        data_home_from(Some("   "), Some(Path::new("/home/u"))),
        PathBuf::from("/home/u/.onion")
    );
}

#[test]
fn falls_back_to_home_dot_onion() {
    assert_eq!(
        data_home_from(None, Some(Path::new("/home/u"))),
        PathBuf::from("/home/u/.onion")
    );
}

#[test]
fn no_home_defaults_to_cwd() {
    assert_eq!(data_home_from(None, None), PathBuf::from("./.onion"));
}

#[test]
fn config_and_credentials_live_under_root() {
    let root = Path::new("/root");
    assert_eq!(config_file(root), PathBuf::from("/root/config.toml"));
    assert_eq!(credentials_file(root), PathBuf::from("/root/.credentials"));
    assert_eq!(logs_dir(root), PathBuf::from("/root/logs"));
}

#[test]
fn job_log_file_spills_under_logs() {
    assert_eq!(
        job_log_file(Path::new("/root"), "job-1"),
        PathBuf::from("/root/logs/jobs/job-1.log")
    );
}

#[test]
fn normalize_windows_path() {
    assert_eq!(normalize_cwd(Path::new(r"C:\workspace\demo")), "C--workspace-demo");
}

#[test]
fn normalize_posix_path() {
    assert_eq!(normalize_cwd(Path::new("/workspace/demo")), "-workspace-demo");
}

#[test]
fn sessions_dir_is_project_scoped() {
    assert_eq!(
        sessions_dir(Path::new("/root"), Path::new(r"C:\w\S4")),
        PathBuf::from("/root/sessions/C--w-S4")
    );
}

#[test]
fn session_layout_matches_reference() {
    // <root>/sessions/<normalized-cwd>/<encoded-id>/session.v1.jsonl
    let dir = session_dir(
        &sessions_dir(Path::new("/root"), Path::new("/w/p")),
        "onion-123",
    );
    assert_eq!(dir, PathBuf::from("/root/sessions/-w-p/onion-123"));
    assert_eq!(
        session_log_file(&dir),
        PathBuf::from("/root/sessions/-w-p/onion-123/session.v1.jsonl")
    );
}

#[test]
fn encode_session_id_blocks_traversal() {
    // `../../etc/passwd` 的 9 个非字母数字字符（. . / . . / e t c 中的 . 与 / 共 5 个，
    // 逐字符映射）全部替换为 `_`，路径分隔符不残留。
    assert!(!encode_session_id("../../etc/passwd").contains('/'));
    assert_eq!(encode_session_id("../../etc/passwd"), "______etc_passwd");
    assert_eq!(encode_session_id("onion-1792383243321"), "onion-1792383243321");
    assert_eq!(encode_session_id(""), "session");
}
