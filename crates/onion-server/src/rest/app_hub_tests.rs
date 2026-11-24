//! app_hub 模块单元测试：文件树筛选（仅预构建产物）与清单解析。

use std::collections::HashSet;

use super::{app_files, app_id_of, bundle_bases, manifest_entries};

fn tree(paths: &[&str]) -> HashSet<String> {
    paths.iter().map(std::string::ToString::to_string).collect()
}

#[test]
fn app_files_collects_only_target_app_subtree() {
    let paths = tree(&[
        "README.md",
        "apps/hotboard/manifest.json",
        "apps/hotboard/main.js",
        "apps/hotboard/assets/logo.svg",
        "apps/other/manifest.json",
        "apps/other/main.js",
    ]);
    let files = app_files(&paths, "apps/hotboard/");
    assert_eq!(
        files,
        vec![
            (
                "assets/logo.svg".to_string(),
                "apps/hotboard/assets/logo.svg"
            ),
            ("main.js".to_string(), "apps/hotboard/main.js"),
            ("manifest.json".to_string(), "apps/hotboard/manifest.json"),
        ]
    );
}

#[test]
fn app_files_ignores_path_traversal_entries() {
    let paths = tree(&[
        "apps/hotboard/manifest.json",
        "apps/hotboard/../escape.txt",
        "apps/hotboard//abs.txt",
    ]);
    let files = app_files(&paths, "apps/hotboard/");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].0, "manifest.json");
}

#[test]
fn app_files_empty_for_unknown_app() {
    let paths = tree(&["apps/other/manifest.json"]);
    assert!(app_files(&paths, "apps/hotboard/").is_empty());
}

#[test]
fn bundle_bases_accepts_flat_and_dist_layouts() {
    let paths = tree(&[
        "apps/chatapp/manifest.json",
        "apps/chatapp/main.js",
        "apps/hotboard/dist/manifest.json",
        "apps/hotboard/dist/main.js",
        "apps/hotboard/src/main.tsx",
        "apps/hotboard/package.json",
    ]);
    let bases = bundle_bases(&paths);
    assert_eq!(
        bases,
        vec![
            ("chatapp".to_string(), "apps/chatapp/".to_string()),
            ("hotboard".to_string(), "apps/hotboard/dist/".to_string()),
        ]
    );
}

#[test]
fn bundle_bases_ignores_unrelated_manifests() {
    let paths = tree(&[
        "apps/hotboard/package.json",
        "apps/hotboard/src/manifest.json",
    ]);
    assert!(bundle_bases(&paths).is_empty());
}

#[test]
fn app_id_rejects_invalid_ids() {
    assert_eq!(
        app_id_of(&serde_json::json!({ "id": "hot-board_2.x" })),
        Some("hot-board_2.x".to_string())
    );
    assert_eq!(app_id_of(&serde_json::json!({ "id": "../evil" })), None);
    assert_eq!(app_id_of(&serde_json::json!({ "id": "" })), None);
    assert_eq!(app_id_of(&serde_json::json!({ "id": "a/b" })), None);
    assert_eq!(app_id_of(&serde_json::json!({ "id": 123 })), None);
}

#[test]
fn manifest_entries_collects_top_level_and_contributed_pages() {
    let manifest = serde_json::json!({
        "entry": "index.js",
        "contributes": { "apps": [{ "id": "main", "entry": "main.js" }] }
    });
    assert_eq!(manifest_entries(&manifest), vec!["index.js", "main.js"]);
}
