//! `credentials` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

use super::credential_refs_in;
use super::is_credential_ref_name;
use super::mask_credential;
use super::resolve_template;
use super::CredentialInfo;
use super::CredentialSource;
use super::CredentialStore;
use super::TemplateError;

fn temp_root(tag: &str) -> std::path::PathBuf {
    // 同进程内多个测试共用根时按计数器分目录，避免并行冲突。
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "onion_credentials_{}_{tag}_{}",
        std::process::id(),
        seq
    ))
}

fn write_credentials_file(root: &Path, refs: &[(&str, &str)]) {
    fs::create_dir_all(root).unwrap();
    let mut body = String::from("{\n  \"version\": 1,\n  \"refs\": {");
    for (idx, (name, value)) in refs.iter().enumerate() {
        if idx > 0 {
            body.push(',');
        }
        body.push_str(&format!("\"{name}\": \"{value}\""));
    }
    body.push_str("}}");
    fs::write(crate::paths::credentials_file(root), body).unwrap();
}

#[test]
fn credential_ref_name_grammar() {
    assert!(is_credential_ref_name("LOCAL_KEY"));
    assert!(is_credential_ref_name("_private"));
    assert!(is_credential_ref_name("a1_b2"));
    assert!(!is_credential_ref_name(""));
    assert!(!is_credential_ref_name("9key"));
    assert!(!is_credential_ref_name("has-dash"));
    assert!(!is_credential_ref_name("has space"));
    assert!(!is_credential_ref_name("a/b"));
}

#[test]
fn credential_refs_in_scans_tokens() {
    assert!(credential_refs_in("plain key").unwrap().is_empty());
    assert_eq!(
        credential_refs_in("${credential:LOCAL_KEY}").unwrap(),
        vec!["LOCAL_KEY"]
    );
    assert_eq!(
        credential_refs_in("pre-${credential:KEY_A}-mid-${credential:KEY_B}-post").unwrap(),
        vec!["KEY_A", "KEY_B"]
    );
    // 同一引用出现两次：按出现顺序重复收录（展开时逐处替换）。
    assert_eq!(
        credential_refs_in("${credential:KEY_A}${credential:KEY_A}").unwrap(),
        vec!["KEY_A", "KEY_A"]
    );
}

#[test]
fn credential_refs_in_rejects_malformed_syntax() {
    // 未闭合。
    assert!(credential_refs_in("${credential:KEY").is_err());
    // 非凭据模板片段。
    assert!(credential_refs_in("${HOME}").is_err());
    // 凭据名非法。
    assert!(credential_refs_in("${credential:has-dash}").is_err());
}

#[test]
fn resolve_template_substitutes_in_order() {
    let resolved = resolve_template(
        "pre-${credential:KEY_A}-mid-${credential:KEY_B}-post",
        &|name| Some(format!("v-{name}")),
    )
    .unwrap();
    assert_eq!(resolved, "pre-v-KEY_A-mid-v-KEY_B-post");
}

#[test]
fn resolve_template_passthrough_without_tokens() {
    let resolved = resolve_template("sk-plain", &|_| Some("x".to_string())).unwrap();
    assert_eq!(resolved, "sk-plain");
}

#[test]
fn resolve_template_missing_reports_name() {
    let err = resolve_template("pre-${credential:KEY_A}", &|_| None).unwrap_err();
    assert!(matches!(err, TemplateError::Missing { ref name } if name == "KEY_A"));
}

#[test]
fn resolve_template_syntax_error_fails_loud() {
    let err = resolve_template("pre-${HOME}", &|_| Some("x".to_string())).unwrap_err();
    assert!(matches!(err, TemplateError::Syntax(_)));
}

#[test]
fn store_resolve_prefers_file_over_env() {
    let root = temp_root("precedence");
    write_credentials_file(&root, &[("LOCAL_KEY", "from-file")]);
    let store = CredentialStore::new(&root);
    let resolved = store
        .resolve_with("LOCAL_KEY", &|_| Some("from-env".to_string()))
        .unwrap()
        .expect("file 层供值");
    assert_eq!(
        resolved,
        super::ResolvedCredential {
            value: "from-file".to_string(),
            source: CredentialSource::File
        }
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn store_resolve_falls_back_to_env_and_treats_empty_as_absent() {
    let root = temp_root("env_fallback");
    let store = CredentialStore::new(&root);
    let env_only = |name: &str| (name == "ENV_KEY").then(|| "from-env".to_string());
    let resolved = store
        .resolve_with("ENV_KEY", &env_only)
        .unwrap()
        .expect("env 回退");
    assert_eq!(resolved.source, CredentialSource::Env);

    // 空值处处缺席：环境空值不算配置。
    let empty_env = |name: &str| (name == "ENV_KEY").then(String::new);
    assert!(store.resolve_with("ENV_KEY", &empty_env).unwrap().is_none());
    assert!(store
        .resolve_with("MISSING_KEY", &env_only)
        .unwrap()
        .is_none());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn store_resolve_skips_empty_file_value() {
    let root = temp_root("empty_file_value");
    write_credentials_file(&root, &[("LOCAL_KEY", ""), ("ENV_KEY", "")]);
    let store = CredentialStore::new(&root);
    let env = |name: &str| (name == "LOCAL_KEY").then(|| "from-env".to_string());
    // 文件空值不短路——回退到环境。
    let resolved = store
        .resolve_with("LOCAL_KEY", &env)
        .unwrap()
        .expect("env 供值");
    assert_eq!(resolved.value, "from-env");
    // 文件与环境都空 → 缺席。
    assert!(store.resolve_with("ENV_KEY", &|_| None).unwrap().is_none());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn store_resolve_rejects_invalid_name_and_corrupt_document() {
    let root = temp_root("invalid");
    fs::create_dir_all(&root).unwrap();
    let store = CredentialStore::new(&root);
    assert!(store.resolve_with("bad-name", &|_| None).is_err());

    // 损坏文档 fail-closed：报错而非当空文档。
    fs::write(crate::paths::credentials_file(&root), "not json").unwrap();
    assert!(store.resolve_with("LOCAL_KEY", &|_| None).is_err());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn store_set_unset_roundtrip_persists_across_instances() {
    let root = temp_root("roundtrip");
    fs::create_dir_all(&root).unwrap();
    let store = CredentialStore::new(&root);

    // set：拒空值、拒非法名。
    assert!(store.set("LOCAL_KEY", "   ").is_err());
    assert!(store.set("bad-name", "v").is_err());

    store.set("LOCAL_KEY", "sk-secret").unwrap();
    // 新实例按次重读：写入立即可见（无缓存）。
    let reopened = CredentialStore::new(&root);
    let resolved = reopened.resolve("LOCAL_KEY").unwrap().expect("持久化");
    assert_eq!(resolved.value, "sk-secret");
    assert_eq!(resolved.source, CredentialSource::File);

    // unset 后缺席；移除缺席引用 no-op。
    reopened.unset("LOCAL_KEY").unwrap();
    assert!(reopened.resolve("LOCAL_KEY").unwrap().is_none());
    reopened.unset("LOCAL_KEY").unwrap();
    assert!(reopened.resolve("LOCAL_KEY").unwrap().is_none());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn store_describe_masks_value() {
    let root = temp_root("describe");
    write_credentials_file(&root, &[("LOCAL_KEY", "sk-0123456789abcdef")]);
    let store = CredentialStore::new(&root);
    let info = store.describe("LOCAL_KEY").unwrap();
    assert_eq!(
        info,
        CredentialInfo {
            configured: true,
            source: Some(CredentialSource::File),
            writable: true,
            masked: Some(mask_credential("sk-0123456789abcdef")),
        }
    );
    // 掩码永不包含全值。
    let masked = info.masked.expect("masked");
    assert!(!masked.contains("sk-0123456789abcdef"));

    let absent = store.describe("MISSING_KEY").unwrap();
    assert_eq!(
        absent,
        CredentialInfo {
            configured: false,
            source: None,
            writable: true,
            masked: None,
        }
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn mask_credential_hides_short_values_entirely() {
    assert_eq!(
        mask_credential("sk-0123456789abcdef"),
        "sk-0…cdef（19 字符）"
    );
    assert_eq!(mask_credential("short"), "••••••••（5 字符）");
    assert_eq!(mask_credential(""), "••••••••（0 字符）");
}

#[test]
fn store_resolve_template_end_to_end() {
    let root = temp_root("template");
    write_credentials_file(&root, &[("LOCAL_KEY", "sk-secret")]);
    let store = CredentialStore::new(&root);
    assert_eq!(
        store
            .resolve_template("Bearer ${credential:LOCAL_KEY}")
            .unwrap(),
        "Bearer sk-secret"
    );
    // 未配置 → Missing 点名。
    let err = store
        .resolve_template("${credential:MISSING_KEY}")
        .unwrap_err();
    assert!(matches!(err, TemplateError::Missing { ref name } if name == "MISSING_KEY"));
    let _ = fs::remove_dir_all(root);
}
