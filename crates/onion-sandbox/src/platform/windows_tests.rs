//! `windows` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;

#[test]
fn denies_on_plain_english_marker() {
    assert!(looks_like_access_denied(
        "New-Item : Access to the path 'C:\\a\\b' is denied."
    ));
}

#[test]
fn denies_on_line_wrapped_exception_name() {
    // 实测形态：PowerShell 按控制台宽度折行，长标识被换行 + 空格切开。
    let output = "    + CategoryInfo          : PermissionDenied: (C:\\ws\\t3:String) [New-Item], UnauthorizedAcc\r\n    essException\r\n";
    assert!(looks_like_access_denied(output));
}

#[test]
fn denies_on_chinese_marker() {
    assert!(looks_like_access_denied("New-Item : 对路径“t3”的访问被拒绝。"));
    assert!(looks_like_access_denied("拒绝访问。"));
}

#[test]
fn keeps_ordinary_failures() {
    // 业务失败（用例失败、test -f 返回 1）不应当被判成沙箱拒绝。
    assert!(!looks_like_access_denied(
        "test result: FAILED. 2 passed; 1 failed"
    ));
    assert!(!looks_like_access_denied(""));
}

#[test]
fn quotes_arguments_with_spaces() {
    assert_eq!(quote_windows_arg("plain"), "plain");
    assert_eq!(quote_windows_arg("a b"), "\"a b\"");
    assert_eq!(quote_windows_arg(""), "\"\"");
}
