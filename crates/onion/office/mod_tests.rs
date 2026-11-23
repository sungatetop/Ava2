//! `office` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//!
//! 转换冒烟依赖真实 soffice：未安装时静默跳过（不伪造产物），
//! 其余纯映射 / 清扫逻辑为确定性单测。

use super::*;

#[test]
fn family_of_maps_supported_extensions() {
    assert_eq!(family_of("doc"), Some(Family::Text));
    assert_eq!(family_of("DOCX"), Some(Family::Text));
    assert_eq!(family_of("rtf"), Some(Family::Text));
    assert_eq!(family_of("csv"), Some(Family::Spreadsheet));
    assert_eq!(family_of("xlsx"), Some(Family::Spreadsheet));
    assert_eq!(family_of("odp"), Some(Family::Presentation));
    assert_eq!(family_of("ppt"), Some(Family::Presentation));
    assert_eq!(family_of("pdf"), Some(Family::Pdf));
    assert_eq!(family_of("md"), None);
    assert_eq!(family_of(""), None);
}

#[test]
fn conversion_targets_match_families() {
    assert_eq!(conversion_targets(Family::Text), ["pdf", "docx", "odt", "txt"]);
    assert_eq!(conversion_targets(Family::Spreadsheet), ["pdf", "xlsx", "ods", "csv"]);
    assert_eq!(conversion_targets(Family::Presentation), ["pdf", "pptx", "odp"]);
    // pdf 只支持首页渲染，不做格式转换。
    assert!(conversion_targets(Family::Pdf).is_empty());
}

#[test]
fn office_error_display_carries_code_prefix() {
    assert_eq!(
        OfficeError::Unavailable.to_string(),
        "unavailable: LibreOffice (soffice) is not installed"
    );
    assert!(OfficeError::InvalidRequest.to_string().starts_with("invalid-request: "));
    assert!(OfficeError::InvalidDocument.to_string().starts_with("invalid-document: "));
    assert!(OfficeError::UnsupportedFormat.to_string().starts_with("unsupported-format: "));
    assert!(OfficeError::InvalidOutput.to_string().starts_with("invalid-output: "));
    assert!(OfficeError::OutputTooLarge.to_string().starts_with("output-too-large: "));
    assert!(OfficeError::InputTooLarge.to_string().starts_with("input-too-large: "));
    assert!(OfficeError::Timeout.to_string().starts_with("timeout: "));
    assert_eq!(OfficeError::Failed("boom".to_string()).to_string(), "failed: boom");
}

#[test]
fn sweep_removes_only_expired_runs() {
    let root = std::env::temp_dir().join(format!("onion_office_sweep_{}", std::process::id()));
    let run = root.join("run");
    std::fs::create_dir_all(&run).expect("create run dir");
    // 保留期内不清扫。
    sweep_in(&root, OUTPUT_RETENTION);
    assert!(run.exists());
    // 零保留期 = 全部过期（run 目录 mtime 早于当前时刻）。
    sweep_in(&root, Duration::from_secs(0));
    assert!(!run.exists());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn file_url_has_no_backslashes() {
    let url = file_url(Path::new("onion-office").join("profile").as_path());
    assert!(!url.contains('\\'), "unexpected backslash in {url}");
    assert!(url.starts_with("file:///"), "unexpected scheme in {url}");
}

/// 真实引擎冒烟：纯文本 → PDF（无 soffice 环境静默跳过）。
#[tokio::test]
async fn converts_plain_text_to_pdf() {
    let Some(engine) = Engine::locate() else {
        return;
    };
    let run = create_run_dir().expect("create run dir");
    let converted = engine.convert(b"onion office smoke\n", "txt", "pdf", &run).await;
    let _ = std::fs::remove_dir_all(&run);
    let converted = converted.expect("convert txt -> pdf");
    assert_eq!(converted.format, "pdf");
    assert!(converted.bytes > 0);
    assert_eq!(converted.path.extension().and_then(|ext| ext.to_str()), Some("pdf"));
}
