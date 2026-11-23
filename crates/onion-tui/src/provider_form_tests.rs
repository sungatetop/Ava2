//! provider_form 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn provider_form_navigates_and_submits() {
    let mut form = ProviderForm::open_provider();
    assert!(form.is_open());
    assert_eq!(form.kind, FormKind::Provider);
    assert_eq!(
        form.fields(),
        &["id", "name", "base_url", "api_key", "models"]
    );

    // 输入 id
    for c in "my-prov".chars() {
        assert_eq!(form.handle_key(key(KeyCode::Char(c))), None);
    }
    assert_eq!(form.values[0], "my-prov");

    // Tab 到 name
    assert_eq!(form.handle_key(key(KeyCode::Tab)), None);
    assert_eq!(form.active, 1);
    for c in "My Prov".chars() {
        form.handle_key(key(KeyCode::Char(c)));
    }
    // Shift+Tab 回到 id
    assert_eq!(form.handle_key(key(KeyCode::BackTab)), None);
    assert_eq!(form.active, 0);

    // 直接跳到最后一个字段并提交
    form.active = 4;
    let result = form.handle_key(key(KeyCode::Enter));
    assert!(!form.is_open());
    match result {
        Some(FormResult::AddProvider(id, name, base_url, api_key, models)) => {
            assert_eq!(id, "my-prov");
            assert_eq!(name, "My Prov");
            assert_eq!(base_url, "");
            assert_eq!(api_key, "");
            assert_eq!(models, "");
        }
        other => panic!("expected AddProvider, got {other:?}"),
    }
}

#[test]
fn model_form_prefills_provider_and_submits() {
    let mut form = ProviderForm::open_model("local".to_string());
    assert_eq!(form.kind, FormKind::Model);
    assert_eq!(form.values[0], "local");
    // 初始 active 应指向 id 字段（索引 1）
    assert_eq!(form.active, 1);

    for c in "new-model".chars() {
        form.handle_key(key(KeyCode::Char(c)));
    }
    assert_eq!(form.values[1], "new-model");

    // 跳到描述并提交
    form.active = 3;
    let result = form.handle_key(key(KeyCode::Enter)).unwrap();
    match result {
        FormResult::AddModel(pid, id, name, description) => {
            assert_eq!(pid, "local");
            assert_eq!(id, "new-model");
            assert_eq!(name, "");
            assert_eq!(description, "");
        }
        other => panic!("expected AddModel, got {other:?}"),
    }
}

#[test]
fn esc_closes_without_result() {
    let mut form = ProviderForm::open_provider();
    for c in "abc".chars() {
        form.handle_key(key(KeyCode::Char(c)));
    }
    assert_eq!(form.handle_key(key(KeyCode::Esc)), None);
    assert!(!form.is_open());
    // 关闭后再按键无效果
    assert_eq!(form.handle_key(key(KeyCode::Enter)), None);
}

#[test]
fn render_shows_fields_and_active_marker() {
    let mut form = ProviderForm::open_provider();
    for c in "x".chars() {
        form.handle_key(key(KeyCode::Char(c)));
    }
    let lines = form.render();
    assert_eq!(lines[0].text, "── /provider 新增供应商 ─────────────────────────");
    assert!(lines[1].text.starts_with("> id: x"));
    assert!(lines.iter().any(|l| l.text.contains("base_url: ")));
    // 关闭后渲染为空
    form.handle_key(key(KeyCode::Esc));
    assert!(form.render().is_empty());
}
