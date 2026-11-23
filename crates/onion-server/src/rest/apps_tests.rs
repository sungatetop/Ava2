//! apps 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//!
//! 覆盖 `seed_default_apps`：首次启动种子安装复制内置 App 并登记、
//! 非 App 目录跳过、已有登记表时不覆盖（用户卸载优先）。

use std::fs;
use std::path::PathBuf;

use super::{read_state, seed_default_apps, InstallState};

/// 构造一个 App 目录（manifest + 入口文件），返回根路径。
fn make_app(root: &PathBuf, id: &str, name: &str) {
    let dir = root.join(id);
    fs::create_dir_all(&dir).unwrap();
    let manifest = serde_json::json!({
        "id": id,
        "name": name,
        "hostApi": 2,
        "contributes": { "apps": [{ "id": "main", "entry": "main.js" }] }
    });
    fs::write(dir.join("manifest.json"), serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
    fs::write(dir.join("main.js"), "export default {};\n").unwrap();
}

#[test]
fn seed_copies_valid_apps_and_skips_non_app_dirs() {
    let tmp = std::env::temp_dir().join(format!("ava2_apps_seed_1_{}", std::process::id()));
    let bundled = tmp.join("bundled");
    let apps_dir = tmp.join("apps");
    fs::create_dir_all(&bundled).unwrap();
    fs::create_dir_all(&apps_dir).unwrap();
    fs::write(bundled.join("junk.txt"), "not an app").unwrap();

    make_app(&bundled, "chatapp", "对话");
    make_app(&bundled, "file-explorer", "文件浏览");
    make_app(&bundled, "file-viewer", "文件预览");

    let bundled_dir = bundled;
    let target = apps_dir;
    seed_default_apps(&target, Some(&bundled_dir)).unwrap();

    // 三个合法 App 全部复制进仓库目录。
    for id in ["chatapp", "file-explorer", "file-viewer"] {
        assert!(target.join(id).join("main.js").is_file(), "{id} 入口未复制");
    }
    // 非 App 目录（junk.txt）不产生登记项。
    assert!(!target.join("junk.txt").exists());

    // 登记表：三个 App 皆 enabled，junk 不出现。
    let state: InstallState = serde_json::from_str(&fs::read_to_string(target.join("state.json")).unwrap()).unwrap();
    assert_eq!(state.apps.len(), 3);
    for id in ["chatapp", "file-explorer", "file-viewer"] {
        let entry = state.apps.get(id).unwrap_or_else(|| panic!("{id} 未登记"));
        assert!(entry.enabled);
    }

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn seed_does_not_touch_existing_registry() {
    let tmp = std::env::temp_dir().join(format!("ava2_apps_seed_2_{}", std::process::id()));
    let bundled = tmp.join("bundled");
    let apps_dir = tmp.join("apps");
    fs::create_dir_all(&bundled).unwrap();
    fs::create_dir_all(&apps_dir).unwrap();
    make_app(&bundled, "chatapp", "对话");

    // 首次种子：chatapp 复制并登记为 enabled。
    let bundled_dir = bundled;
    let target = apps_dir;
    seed_default_apps(&target, Some(&bundled_dir)).unwrap();
    let state: InstallState = serde_json::from_str(&fs::read_to_string(target.join("state.json")).unwrap()).unwrap();
    assert!(state.apps["chatapp"].enabled);

    // 模拟用户停用（卸载优先：删除目录 + 登记 enabled=false）。
    fs::remove_dir_all(target.join("chatapp")).unwrap();
    let mut state = state;
    state.apps.get_mut("chatapp").unwrap().enabled = false;
    fs::write(
        target.join("state.json"),
        serde_json::to_vec_pretty(&state).unwrap(),
    )
    .unwrap();

    // 再次种子（如升级后重启）：登记表已存在 → 不复制、不恢复。
    seed_default_apps(&target, Some(&bundled_dir)).unwrap();
    assert!(!target.join("chatapp").exists(), "用户卸载的 App 不得被种子恢复");
    let after: InstallState = serde_json::from_str(&fs::read_to_string(target.join("state.json")).unwrap()).unwrap();
    assert!(!after.apps["chatapp"].enabled);

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn read_state_tolerates_utf8_bom() {
    let tmp = std::env::temp_dir().join(format!("ava2_apps_bom_{}", std::process::id()));
    fs::create_dir_all(&tmp).unwrap();
    let body = r#"{"apps":{"chatapp":{"enabled":true,"installed_at_ms":1}}}"#;
    fs::write(tmp.join("state.json"), format!("\u{feff}{body}")).unwrap();

    let state = read_state(&tmp);
    assert!(state.apps["chatapp"].enabled);

    let _ = fs::remove_dir_all(&tmp);
}
