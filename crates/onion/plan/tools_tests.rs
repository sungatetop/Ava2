//! `tools` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//! 覆盖 exit_plan_mode 的标题校验（`^#\s+\S`）与空 plan 拒绝。

use super::valid_heading;

#[test]
fn heading_accepts_hash_space_text() {
    assert!(valid_heading("# Plan title"));
    assert!(valid_heading("#\tTab heading"));
    assert!(valid_heading("#  Multiple spaces"));
}

#[test]
fn heading_rejects_bare_hash_and_missing_space() {
    // 仅 `#`、后无空白、或空白后无内容都不满足 `^#\s+\S`。
    assert!(!valid_heading("#"));
    assert!(!valid_heading("#Plan"));
    assert!(!valid_heading("# "), "空白后无非空白字符应拒绝");
    assert!(!valid_heading("#\t"), "制表符后无非空白字符应拒绝");
    assert!(!valid_heading("##"), "`##` 后无空白应拒绝");
    assert!(!valid_heading("plain text"), "非 # 开头应拒绝");
}
