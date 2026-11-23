//! `workspace_deps` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::path::PathBuf;

use serde_json::json;

use crate::workspace_deps::manifest::{parse_primary_runtime, PrimaryRuntimeManifest};
use crate::workspace_deps::WorkspaceDepsError;

/// 临时目录：`<temp>/ava_workspace_deps_<pid>`，测试结束清理。
fn temp_dir() -> PathBuf {
    std::env::temp_dir().join(format!("ava_workspace_deps_{}", std::process::id()))
}

fn cleanup(root: &PathBuf) {
    if root.exists() {
        let _ = std::fs::remove_dir_all(root);
    }
}

fn write_runtime(root: &PathBuf, value: serde_json::Value) {
    std::fs::create_dir_all(root).unwrap();
    std::fs::write(root.join("runtime.json"), value.to_string()).unwrap();
}

fn valid_manifest() -> serde_json::Value {
    json!({
        "desktopVersion": "1.0.0",
        "platform": "win32",
        "arch": "x64",
        "python": "3.12.1",
        "node": "20.11.0",
        "pnpm": "9.1.0",
        "pythonPackages": {
            "numpy": "1.26.4",
            "pandas": "2.2.0"
        }
    })
}

#[test]
fn parse_valid_flat_manifest() {
    let m = parse_primary_runtime(&valid_manifest()).unwrap();
    assert_eq!(
        m,
        PrimaryRuntimeManifest {
            desktop_version: "1.0.0".into(),
            platform: "win32".into(),
            arch: "x64".into(),
            payload_digest: None,
            python: "3.12.1".into(),
            node: Some("20.11.0".into()),
            pnpm: Some("9.1.0".into()),
            python_packages: [
                ("numpy".to_string(), "1.26.4".to_string()),
                ("pandas".to_string(), "2.2.0".to_string()),
            ]
            .into_iter()
            .collect(),
        }
    );
}

#[test]
fn parse_rejects_missing_required_fields() {
    let mut v = valid_manifest();
    v.as_object_mut().unwrap().remove("python");
    assert!(matches!(
        parse_primary_runtime(&v),
        Err(WorkspaceDepsError::InvalidMetadata(_))
    ));
}

#[test]
fn parse_rejects_bad_platform_and_arch() {
    let mut v = valid_manifest();
    v["platform"] = json!("freebsd");
    assert!(matches!(
        parse_primary_runtime(&v),
        Err(WorkspaceDepsError::InvalidMetadata(_))
    ));
    let mut v = valid_manifest();
    v["arch"] = json!("riscv64");
    assert!(matches!(
        parse_primary_runtime(&v),
        Err(WorkspaceDepsError::InvalidMetadata(_))
    ));
}

#[test]
fn parse_rejects_pnpm_without_node() {
    let mut v = valid_manifest();
    v.as_object_mut().unwrap().remove("node");
    // pnpm 仍然存在但 node 缺失 → 非法。
    assert!(matches!(
        parse_primary_runtime(&v),
        Err(WorkspaceDepsError::InvalidMetadata(_))
    ));
}

#[test]
fn parse_normalizes_distribution_names() {
    let mut v = valid_manifest();
    v["pythonPackages"] = json!({ "NumPy": "1.26.4" });
    let m = parse_primary_runtime(&v).unwrap();
    assert!(m.python_packages.contains_key("numpy"));
}

#[test]
fn parse_rejects_duplicate_normalized_distributions() {
    let mut v = valid_manifest();
    v["pythonPackages"] = json!({ "numpy": "1.26.4", "NumPy": "2.0.0" });
    assert!(matches!(
        parse_primary_runtime(&v),
        Err(WorkspaceDepsError::InvalidMetadata(_))
    ));
}

#[test]
fn parse_rejects_legacy_components_format() {
    let v = json!({
        "desktopVersion": "1.0.0",
        "platform": "win32",
        "arch": "x64",
        "components": { "python": "3.12.1" },
        "pythonPackages": {}
    });
    assert!(matches!(
        parse_primary_runtime(&v),
        Err(WorkspaceDepsError::InvalidMetadata(_))
    ));
}

#[test]
fn resolve_returns_paths_for_win32_payload() {
    let root = temp_dir().join("win32");
    cleanup(&root);
    write_runtime(&root, valid_manifest());
    // 构造依赖入口（win32 布局）。
    let py = root.join("dependencies/python/python.exe");
    std::fs::create_dir_all(py.parent().unwrap()).unwrap();
    std::fs::write(&py, "x").unwrap();
    let site = root.join("dependencies/python/Lib/site-packages");
    std::fs::create_dir_all(&site).unwrap();
    let node = root.join("dependencies/node/bin/node.exe");
    std::fs::create_dir_all(node.parent().unwrap()).unwrap();
    std::fs::write(&node, "x").unwrap();
    let node_modules = root.join("dependencies/node/node_modules");
    std::fs::create_dir_all(&node_modules).unwrap();
    let pnpm = root.join("dependencies/pnpm/bin/pnpm.mjs");
    std::fs::create_dir_all(pnpm.parent().unwrap()).unwrap();
    std::fs::write(&pnpm, "x").unwrap();

    let service = crate::workspace_deps::WorkspaceDepsService::new(root.clone());
    // 仅当运行进程是 win32/x64 时才期望成功；否则标记不兼容。
    let result = service.resolve();
    if cfg!(windows) && cfg!(target_arch = "x86_64") {
        let deps = result.unwrap();
        assert_eq!(deps.python, py);
        assert_eq!(deps.python_packages, site);
        assert_eq!(deps.node, Some(node));
        assert_eq!(deps.node_packages, Some(node_modules));
        assert_eq!(deps.pnpm, Some(pnpm));
        assert_eq!(deps.python_distributions["numpy"], "1.26.4");
    } else {
        assert!(matches!(result, Err(WorkspaceDepsError::Incompatible)));
    }
    cleanup(&root);
}

#[test]
fn resolve_rejects_missing_payload_entry() {
    let root = temp_dir().join("missing");
    cleanup(&root);
    write_runtime(&root, valid_manifest());
    // 不创建任何 dependencies 入口。
    let service = crate::workspace_deps::WorkspaceDepsService::new(root.clone());
    let result = service.resolve();
    if cfg!(windows) && cfg!(target_arch = "x86_64") {
        assert!(matches!(
            result,
            Err(WorkspaceDepsError::MissingEntry(_))
        ));
    } else {
        // 平台不匹配优先于入口缺失。
        assert!(matches!(result, Err(WorkspaceDepsError::Incompatible)));
    }
    cleanup(&root);
}

#[test]
fn read_runtime_roundtrip_from_disk() {
    let root = temp_dir().join("roundtrip");
    cleanup(&root);
    write_runtime(&root, valid_manifest());
    let m = crate::workspace_deps::manifest::read_primary_runtime(&root).unwrap();
    assert_eq!(m.python, "3.12.1");
    assert_eq!(m.desktop_version, "1.0.0");
    cleanup(&root);
}
