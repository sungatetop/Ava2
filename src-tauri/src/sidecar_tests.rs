//! `sidecar` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::normalize_windows_path;
use std::path::PathBuf;

#[test]
fn normalizes_extended_path_prefixes() {
    // resource_dir() 经 canonicalize 返回 `\\?\` 前缀，统一归一化保证路径形态一致。
    assert_eq!(
        normalize_windows_path(PathBuf::from(r"\\?\C:\Program Files\ava2\onion-server.exe")),
        PathBuf::from(r"C:\Program Files\ava2\onion-server.exe")
    );
    assert_eq!(
        normalize_windows_path(PathBuf::from(r"\\?\UNC\server\share\onion-server.exe")),
        PathBuf::from(r"\\server\share\onion-server.exe")
    );
    // 普通路径原样保留。
    assert_eq!(
        normalize_windows_path(PathBuf::from(r"C:\Program Files\ava2\onion-server.exe")),
        PathBuf::from(r"C:\Program Files\ava2\onion-server.exe")
    );
}
