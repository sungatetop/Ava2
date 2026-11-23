//! `commands` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::{canonical_name, feedback_text, valid_name, APPROVAL_PRESETS};

#[test]
fn canonical_name_covers_first_party_catalog() {
    // 一等命令目录（英文注册名）全部应可解析；onion 服务端已实现全部。
    for name in ["goal", "plan", "feedback", "compact", "permission"] {
        assert!(canonical_name(name).is_some(), "/{name} 应已注册");
    }
    // `export` 依赖下载路由（尚未落地）与拼写错误都按未知命令处理。
    assert!(canonical_name("export").is_none());
    assert!(canonical_name("unknown").is_none());
}

#[test]
fn valid_name_rejects_non_snake_names() {
    assert!(valid_name("feedback"));
    assert!(valid_name("permission"));
    assert!(valid_name("model_2-x"));
    assert!(!valid_name("Feedback"));
    assert!(!valid_name("2model"));
    assert!(!valid_name("feed back"));
    assert!(!valid_name(""));
}

#[test]
fn feedback_text_trims_and_rejects_blank() {
    assert_eq!(feedback_text("  太慢，希望并行  "), Some("太慢，希望并行".to_string()));
    assert_eq!(feedback_text(""), None);
    assert_eq!(feedback_text("  \n\t "), None);
}

#[test]
fn approval_presets_exclude_derived_custom() {
    // `custom` 是「当前组合不匹配任何预设」的派生态，不可作为切换目标。
    assert_eq!(APPROVAL_PRESETS, ["workspace-write", "auto", "danger-full-access"]);
    assert!(!APPROVAL_PRESETS.contains(&"custom"));
}

#[test]
fn canonical_name_resolves_english_and_chinese_spellings() {
    // 英文注册名原样返回。
    for name in ["goal", "plan", "feedback", "compact", "permission"] {
        assert_eq!(canonical_name(name), Some(name));
    }
    // 中文别名映射到一等命令。
    assert_eq!(canonical_name("目标"), Some("goal"));
    assert_eq!(canonical_name("计划"), Some("plan"));
    assert_eq!(canonical_name("反馈"), Some("feedback"));
    assert_eq!(canonical_name("压缩"), Some("compact"));
    assert_eq!(canonical_name("权限"), Some("permission"));
    // 未注册拼写（含未实现的 export）不解析。
    assert_eq!(canonical_name("export"), None);
    assert_eq!(canonical_name("导出"), None);
    assert_eq!(canonical_name("nosuch"), None);
}
