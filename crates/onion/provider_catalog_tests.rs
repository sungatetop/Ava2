//! provider_catalog 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;

#[test]
fn builtin_catalog_has_local_provider_and_default() {
    let cat = ProviderCatalog::builtin("Qwen3.8-27B");
    assert_eq!(cat.default_model, "Qwen3.8-27B");
    assert!(cat.contains_model("Qwen3.8-27B"));
    assert!(cat.contains_model("DeepSeek-V4-Flash-0731"));
    assert!(!cat.contains_model("does-not-exist"));
}

#[test]
fn target_for_derives_provider_route() {
    let cat = ProviderCatalog::builtin("Qwen3.8-27B");
    let t = cat.target_for("Qwen3.8-27B").unwrap();
    assert_eq!(t.model, "Qwen3.8-27B");
    assert_eq!(t.base_url, crate::config::default_base_url());
    assert_eq!(t.api_key, crate::config::default_api_key());
    assert_eq!(t.effort, None);
}

#[test]
fn target_for_unknown_model_returns_none() {
    let cat = ProviderCatalog::builtin("Qwen3.8-27B");
    assert!(cat.target_for("nope").is_none());
}

#[test]
fn load_reads_providers_json() {
    let root = std::env::temp_dir().join(format!("onion_catalog_{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("providers.json");
    std::fs::write(
        &path,
        r#"{"groups":[{"id":"local","name":"local","baseUrl":"http://x/v1","apiKey":"k",
            "models":[{"id":"M1","name":"M1","reasoning":{"efforts":[{"id":"low","name":"Low"}],"defaultEffort":"low"}}]}]}"#,
    )
    .unwrap();
    let cat = ProviderCatalog::load(&root, "M1");
    assert_eq!(cat.providers.len(), 1);
    let m = &cat.providers[0].models[0];
    assert_eq!(m.id, "M1");
    let t = cat.target_for("M1").unwrap();
    assert_eq!(t.effort.as_deref(), Some("low"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn load_falls_back_to_builtin_when_file_absent() {
    let root = std::env::temp_dir().join(format!("onion_catalog_missing_{}", std::process::id()));
    let cat = ProviderCatalog::load(&root, "Qwen3.8-27B");
    assert!(cat.contains_model("Qwen3.8-27B"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn load_filters_failure_group() {
    let root = std::env::temp_dir().join(format!("onion_catalog_fail_{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("providers.json");
    std::fs::write(
        &path,
        r#"{"groups":[{"id":"failure","name":"broken","baseUrl":"","apiKey":"","models":[]}]}"#,
    )
    .unwrap();
    let cat = ProviderCatalog::load(&root, "D");
    assert!(cat.providers.is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn add_provider_inserts_and_replaces() {
    let mut cat = ProviderCatalog::builtin("Qwen3.8-27B");
    // 新增
    let replaced = cat.add_provider("p2".into(), "P2".into(), "http://p2/v1".into(), "k2".into());
    assert!(!replaced);
    assert_eq!(cat.providers.len(), 2);
    let p2 = cat.providers.iter().find(|p| p.id == "p2").unwrap();
    assert_eq!(p2.base_url, "http://p2/v1");
    // 替换同 id
    let replaced = cat.add_provider("p2".into(), "P2b".into(), "http://p2b/v1".into(), "k2b".into());
    assert!(replaced);
    assert_eq!(cat.providers.len(), 2);
    let p2 = cat.providers.iter().find(|p| p.id == "p2").unwrap();
    assert_eq!(p2.name, "P2b");
    assert_eq!(p2.base_url, "http://p2b/v1");
}

#[test]
fn add_model_to_existing_and_missing_provider() {
    let mut cat = ProviderCatalog::builtin("Qwen3.8-27B");
    // 追加到现有供应商
    let replaced = cat.add_model("local", "new-model".into(), "New Model".into(), Some("desc".into()));
    assert!(!replaced);
    assert!(cat.contains_model("new-model"));
    // 同 id 替换
    let replaced = cat.add_model("local", "new-model".into(), "New Model 2".into(), None);
    assert!(replaced);
    // 供应商不存在 → 创建占位
    let replaced = cat.add_model("ghost", "gm".into(), "GM".into(), None);
    assert!(!replaced);
    let g = cat.providers.iter().find(|p| p.id == "ghost").unwrap();
    assert_eq!(g.base_url, "");
    assert!(g.models.iter().any(|m| m.id == "gm"));
}

#[test]
fn save_roundtrips_catalog() {
    let root = std::env::temp_dir().join(format!("onion_catalog_save_{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut cat = ProviderCatalog::builtin("Qwen3.8-27B");
    cat.add_provider("p2".into(), "P2".into(), "http://p2/v1".into(), "k2".into());
    cat.add_model("p2", "m2".into(), "M2".into(), Some("d2".into()));
    let path = cat.save(&root).unwrap();
    assert!(path.exists());
    // 重新加载验证往返一致
    let reloaded = ProviderCatalog::load(&root, "Qwen3.8-27B");
    assert_eq!(reloaded.providers.len(), 2);
    let p2 = reloaded.providers.iter().find(|p| p.id == "p2").unwrap();
    assert_eq!(p2.base_url, "http://p2/v1");
    assert_eq!(p2.api_key, "k2");
    assert!(p2.models.iter().any(|m| m.id == "m2"));
    let _ = std::fs::remove_dir_all(&root);
}

/// 在临时根写入凭据文件（api_key 引用解析测试用）。
fn write_credentials(root: &std::path::Path, refs: &[(&str, &str)]) {
    std::fs::create_dir_all(root).unwrap();
    let mut body = String::from("{\"version\":1,\"refs\":{");
    for (idx, (name, value)) in refs.iter().enumerate() {
        if idx > 0 {
            body.push(',');
        }
        body.push_str(&format!("\"{name}\": \"{value}\""));
    }
    body.push_str("}}");
    std::fs::write(crate::paths::credentials_file(root), body).unwrap();
}

#[test]
fn target_for_resolves_api_key_credential_reference() {
    let root = std::env::temp_dir().join(format!("onion_catalog_cred_{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("providers.json"),
        r#"{"groups":[{"id":"local","name":"local","baseUrl":"http://x/v1","apiKey":"${credential:LOCAL_KEY}",
            "models":[{"id":"M1","name":"M1"}]}]}"#,
    )
    .unwrap();
    write_credentials(&root, &[("LOCAL_KEY", "sk-secret")]);
    let cat = ProviderCatalog::load(&root, "M1");
    let t = cat.target_for("M1").unwrap();
    assert_eq!(t.api_key, "sk-secret");
    // 内存投影保留引用原样（save 落盘不回写明文，配置面不触达明文）。
    assert_eq!(cat.providers[0].api_key, "${credential:LOCAL_KEY}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn target_for_missing_credential_reference_resolves_empty() {
    let root = std::env::temp_dir().join(format!("onion_catalog_creddm_{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("providers.json"),
        r#"{"groups":[{"id":"local","name":"local","baseUrl":"http://x/v1","apiKey":"${credential:LOCAL_KEY}",
            "models":[{"id":"M1","name":"M1"}]}]}"#,
    )
    .unwrap();
    // 凭据文件缺席（引用未配置）→ 空 api_key + warn，不阻断取目标。
    let cat = ProviderCatalog::load(&root, "M1");
    let t = cat.target_for("M1").unwrap();
    assert_eq!(t.api_key, "");
    assert_eq!(t.base_url, "http://x/v1");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn save_roundtrip_preserves_credential_reference() {
    let root = std::env::temp_dir().join(format!("onion_catalog_credsav_{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut cat = ProviderCatalog::builtin("Qwen3.8-27B");
    cat.add_provider(
        "p2".into(),
        "P2".into(),
        "http://p2/v1".into(),
        "${credential:P2_KEY}".into(),
    );
    cat.add_model("p2", "m2".into(), "M2".into(), None);
    cat.save(&root).unwrap();
    // 重新加载：引用原样保留（落盘不回写明文）。
    let reloaded = ProviderCatalog::load(&root, "Qwen3.8-27B");
    let p2 = reloaded.providers.iter().find(|p| p.id == "p2").unwrap();
    assert_eq!(p2.api_key, "${credential:P2_KEY}");
    // 凭据文件配置值后 target_for 按次解析。
    write_credentials(&root, &[("P2_KEY", "sk-p2")]);
    let t = reloaded.target_for("m2").unwrap();
    assert_eq!(t.api_key, "sk-p2");
    let _ = std::fs::remove_dir_all(&root);
}
