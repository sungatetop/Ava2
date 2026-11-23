//! `lsp/seam.rs` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//! 覆盖扩展名归一化、provider 注册冲突、按扩展名路由与 `LSP_UNAVAILABLE`。

use std::collections::HashMap;
use std::sync::Arc;

use super::{final_extension, LspErrorCode, LspOperation, LspPosition, LspProvider, LspQueryRequest, LspQueryResult, LspService};

/// 测试 provider：固定语言 id，query 返回空 locations。
struct FakeProvider {
    id: String,
    exts: HashMap<String, String>,
}

impl LspProvider for FakeProvider {
    fn id(&self) -> &str {
        &self.id
    }
    fn extension_to_language(&self) -> &HashMap<String, String> {
        &self.exts
    }
    fn query(&self, _request: &LspQueryRequest, _language_id: &str) -> Result<LspQueryResult, super::LspError> {
        Ok(LspQueryResult::Locations {
            locations: vec![],
            resolved_workspace_uri: "file:///ws".to_string(),
        })
    }
}

fn provider(id: &str, exts: &[(&str, &str)]) -> Arc<dyn LspProvider> {
    Arc::new(FakeProvider {
        id: id.to_string(),
        exts: exts.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
    })
}

#[test]
fn final_extension_normalizes_case_and_dotfiles() {
    assert_eq!(final_extension("Foo.TS"), ".ts");
    assert_eq!(final_extension("foo.d.ts"), ".ts");
    assert_eq!(final_extension("src/main.rs"), ".rs");
    assert_eq!(final_extension("src\\main.rs"), ".rs");
    assert_eq!(final_extension("README"), "");
    assert_eq!(final_extension(".bashrc"), "");
}

#[test]
fn register_provider_and_query_by_extension() {
    let service = LspService::new();
    service.register_provider(provider("rust", &[(".rs", "rust")])).unwrap();
    assert!(service.has_providers());
    let result = service
        .query(&LspQueryRequest {
            operation: LspOperation::GoToDefinition,
            file_path: "src/main.rs".to_string(),
            position: LspPosition { line: 0, character: 0 },
            workspace_root: "/ws".to_string(),
        })
        .unwrap();
    assert!(matches!(result, LspQueryResult::Locations { .. }));
}

#[test]
fn query_unknown_extension_returns_unavailable() {
    let service = LspService::new();
    service.register_provider(provider("rust", &[(".rs", "rust")])).unwrap();
    let err = service
        .query(&LspQueryRequest {
            operation: LspOperation::Hover,
            file_path: "notes.txt".to_string(),
            position: LspPosition { line: 0, character: 0 },
            workspace_root: "/ws".to_string(),
        })
        .unwrap_err();
    assert_eq!(err.code, LspErrorCode::Unavailable);
}

#[test]
fn register_provider_conflict_rejected() {
    let service = LspService::new();
    service.register_provider(provider("a", &[(".rs", "rust")])).unwrap();
    // 同名 id 冲突。
    let err = service.register_provider(provider("a", &[(".py", "python")])).unwrap_err();
    assert_eq!(err.code, LspErrorCode::Conflict);
    // 同扩展名跨 provider 冲突。
    let err = service.register_provider(provider("b", &[(".rs", "rust2")])).unwrap_err();
    assert_eq!(err.code, LspErrorCode::Conflict);
    // 失败注册不发布任何内容：.rs 仍由 a 处理。
    assert!(service.has_providers());
}

#[test]
fn register_provider_invalid_rejected() {
    let service = LspService::new();
    // 空 id。
    let err = service.register_provider(provider("", &[(".rs", "rust")])).unwrap_err();
    assert_eq!(err.code, LspErrorCode::InvalidProvider);
    // 空扩展名表。
    let err = service.register_provider(provider("x", &[])).unwrap_err();
    assert_eq!(err.code, LspErrorCode::InvalidProvider);
    // 非法扩展名（含点/斜杠，即使加 leading dot 仍不合法）。
    let err = service.register_provider(provider("y", &[("a.b", "rust")])).unwrap_err();
    assert_eq!(err.code, LspErrorCode::InvalidProvider);
    // 空语言 id。
    let err = service.register_provider(provider("z", &[(".ts", "")])).unwrap_err();
    assert_eq!(err.code, LspErrorCode::InvalidProvider);
}

#[test]
fn no_providers_returns_unavailable() {
    let service = LspService::new();
    assert!(!service.has_providers());
    let err = service
        .query(&LspQueryRequest {
            operation: LspOperation::FindReferences,
            file_path: "a.rs".to_string(),
            position: LspPosition { line: 0, character: 0 },
            workspace_root: "/ws".to_string(),
        })
        .unwrap_err();
    assert_eq!(err.code, LspErrorCode::Unavailable);
    assert!(err.message.contains("no LSP provider"));
}
