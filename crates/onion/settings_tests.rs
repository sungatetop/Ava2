//! `settings` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;

fn temp_root(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("onion_settings_{}_{}", std::process::id(), name))
}

#[test]
fn load_missing_file_returns_default() {
    let root = temp_root("missing");
    let _ = std::fs::remove_file(crate::paths::settings_file(&root));
    let file = SettingsFile::load(&root);
    assert_eq!(file.model_target(), None);
}

#[test]
fn save_then_load_roundtrips() {
    let root = temp_root("roundtrip");
    let target = PersistedModelTarget {
        model: "Qwen3.8-27B".to_string(),
        base_url: "http://localhost:8080/v1".to_string(),
        effort: Some("high".to_string()),
    };
    SettingsFile::save_model_target(&root, &target).expect("写回成功");
    let loaded = SettingsFile::load(&root);
    assert_eq!(loaded.model_target(), Some(target));
}

#[test]
fn save_does_not_keep_effort_when_none() {
    let root = temp_root("noeffort");
    let target = PersistedModelTarget {
        model: "DeepSeek-V4-Flash-0731".to_string(),
        base_url: "http://localhost:8080/v1".to_string(),
        effort: None,
    };
    SettingsFile::save_model_target(&root, &target).expect("写回成功");
    let content = std::fs::read_to_string(crate::paths::settings_file(&root)).expect("读回");
    assert!(!content.contains("effort"), "effort=None 应被序列化省略: {content}");
    let loaded = SettingsFile::load(&root);
    assert_eq!(loaded.model_target(), Some(target));
}

#[test]
fn corrupt_file_falls_back_to_default() {
    let root = temp_root("corrupt");
    let path = crate::paths::settings_file(&root);
    std::fs::create_dir_all(path.parent().expect("父目录")).expect("建目录");
    std::fs::write(&path, "not-json{").expect("写坏文件");
    let file = SettingsFile::load(&root);
    assert_eq!(file.model_target(), None);
}
