//! model_picker 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn entry(label: &str, model: &str, desc: &str) -> ModelEntry {
    ModelEntry { label: label.to_string(), model: model.to_string(), description: desc.to_string() }
}

#[test]
fn closed_by_default_and_open_targets_current_model() {
    let mut picker = ModelPicker::new(vec![
        entry("local/Qwen3.8-27B", "Qwen3.8-27B", "http://localhost:8080/v1"),
        entry("local/DeepSeek-V4-Flash-0731", "DeepSeek-V4-Flash-0731", "http://localhost:8080/v1"),
    ]);
    assert_eq!(picker.state(), &PickerState::Closed);
    picker.open("DeepSeek-V4-Flash-0731");
    assert_eq!(
        picker.state(),
        &PickerState::Open { filter: String::new(), selected: 1 }
    );
}

#[test]
fn filter_matches_label_model_and_description_case_insensitive() {
    let mut picker = ModelPicker::new(vec![
        entry("local/Qwen3.8-27B", "Qwen3.8-27B", "http://localhost:8080/v1"),
        entry("local/DeepSeek-V4-Flash-0731", "DeepSeek-V4-Flash-0731", "http://localhost:8080/v1"),
    ]);
    picker.open("Qwen3.8-27B");
    // 键入 "deep" 命中模型名（大小写不敏感）。
    picker.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
    picker.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE));
    picker.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE));
    picker.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE));
    let visible = picker.visible();
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].model, "DeepSeek-V4-Flash-0731");
}

#[test]
fn escape_clears_filter_first_then_closes() {
    let mut picker = ModelPicker::new(vec![entry("local/Qwen3.8-27B", "Qwen3.8-27B", "http://x")]);
    picker.open("Qwen3.8-27B");
    picker.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
    // 第一次 Escape：清空过滤，保持打开。
    assert_eq!(picker.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)), None);
    assert_eq!(
        picker.state(),
        &PickerState::Open { filter: String::new(), selected: 0 }
    );
    // 第二次 Escape：关闭。
    assert_eq!(picker.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)), None);
    assert_eq!(picker.state(), &PickerState::Closed);
}

#[test]
fn enter_confirms_visible_selection() {
    let mut picker = ModelPicker::new(vec![
        entry("local/Qwen3.8-27B", "Qwen3.8-27B", "http://x"),
        entry("local/DeepSeek-V4-Flash-0731", "DeepSeek-V4-Flash-0731", "http://x"),
    ]);
    picker.open("Qwen3.8-27B");
    picker.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    let picked = picker.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(picked, Some("DeepSeek-V4-Flash-0731".to_string()));
    assert_eq!(picker.state(), &PickerState::Closed);
}

#[test]
fn render_shows_overlay_lines_when_open_and_empty_when_closed() {
    let mut picker = ModelPicker::new(vec![entry("local/Qwen3.8-27B", "Qwen3.8-27B", "http://x")]);
    assert!(picker.render().is_empty());
    picker.open("Qwen3.8-27B");
    let lines = picker.render();
    assert!(lines.iter().any(|l| l.text.contains("/model 选择器")));
    assert!(lines.iter().any(|l| l.text.contains("Qwen3.8-27B")));
}

#[test]
fn from_catalog_flattens_provider_models() {
    let catalog = onion::provider_catalog::ProviderCatalog::builtin("Qwen3.8-27B");
    let mut picker = ModelPicker::from_catalog(&catalog);
    picker.open("DeepSeek-V4-Flash-0731");
    let lines = picker.render();
    // label = provider/model，desc = 条目描述。
    assert!(lines.iter().any(|l| l.text.contains("local/Qwen3.8-27B")));
    assert!(lines.iter().any(|l| l.text.contains("local/DeepSeek-V4-Flash-0731")));
    assert!(lines.iter().any(|l| l.text.contains("local 供应商缺省模型")));
}

#[test]
fn from_catalog_skips_providers_without_models() {
    let mut catalog = onion::provider_catalog::ProviderCatalog::builtin("Qwen3.8-27B");
    catalog.providers.push(onion::provider_catalog::ProviderEntry {
        id: "empty".to_string(),
        name: "empty".to_string(),
        base_url: "http://x".to_string(),
        api_key: "k".to_string(),
        models: Vec::new(),
    });
    let mut picker = ModelPicker::from_catalog(&catalog);
    picker.open("Qwen3.8-27B");
    let lines = picker.render();
    assert!(lines.iter().any(|l| l.text.contains("local/Qwen3.8-27B")));
    assert!(!lines.iter().any(|l| l.text.contains("empty/")));
}
