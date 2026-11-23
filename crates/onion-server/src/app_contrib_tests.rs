//! `app_contrib` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::json;

use super::{load_from, validate};

/// 临时 App 仓库：`apps/<id>/...` + `apps/state.json`，测试结束自动清理。
struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "ava_app_contrib_{name}_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("创建临时 App 仓库");
        Self { root }
    }

    fn apps_dir(&self) -> &Path {
        &self.root
    }

    /// 写入登记表：`[(id, enabled)]`。
    fn state(&self, entries: &[(&str, bool)]) {
        let mut apps = serde_json::Map::new();
        for (id, enabled) in entries {
            apps.insert(
                (*id).to_string(),
                json!({ "enabled": enabled, "installedAtMs": 1 }),
            );
        }
        fs::write(
            self.root.join("state.json"),
            json!({ "apps": apps }).to_string(),
        )
        .expect("写登记表");
    }

    /// 写入 App 清单（含 contributes）并返回 App 根目录。
    fn app(&self, id: &str, contributes: serde_json::Value) -> PathBuf {
        let dir = self.root.join(id);
        fs::create_dir_all(&dir).expect("创建 App 目录");
        fs::write(
            dir.join("manifest.json"),
            json!({
                "id": id,
                "name": id,
                "version": "1.0.0",
                "hostApi": 2,
                "entry": "index.js",
                "contributes": contributes,
            })
            .to_string(),
        )
        .expect("写 manifest");
        dir
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn write_skill(root: &Path, name: &str) {
    let dir = root.join("skills").join(name);
    fs::create_dir_all(&dir).expect("创建技能目录");
    fs::write(
        dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: 测试技能\n---\n\n正文。\n"),
    )
    .expect("写 SKILL.md");
}

#[test]
fn loads_skills_mcp_and_hooks() {
    let fixture = Fixture::new("full");
    let dir = fixture.app(
        "demo",
        json!({
            "skills": ["skills"],
            "mcpServers": [{ "name": "demo-mcp", "command": "demo-mcp-server", "args": ["--stdio"] }],
            "hooks": [{
                "event": "PreToolUse",
                "matcher": "Bash",
                "handler": { "type": "command", "command": "node ${CLAUDE_PLUGIN_ROOT}/hook.js" },
            }],
        }),
    );
    write_skill(&dir, "demo-skill");
    fixture.state(&[("demo", true)]);

    let contribs = load_from(fixture.apps_dir());

    assert_eq!(contribs.notes, Vec::<String>::new());
    assert_eq!(contribs.skills.len(), 1);
    assert_eq!(contribs.skills[0].name(), "app:demo:0");
    assert_eq!(contribs.skills[0].list(None).len(), 1);
    assert_eq!(contribs.mcp.len(), 1);
    assert_eq!(contribs.mcp[0].server_name(), "demo-mcp");

    let hooks = contribs.hooks_config(Path::new("C:/project")).expect("合并后的 hooks 配置");
    let command = hooks["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
        .as_str()
        .expect("command 字符串");
    assert!(command.starts_with("node "));
    assert!(command.contains("hook.js"));
    assert!(!command.contains("${CLAUDE_PLUGIN_ROOT}"));
}

#[test]
fn skips_disabled_app() {
    let fixture = Fixture::new("disabled");
    let dir = fixture.app(
        "off",
        json!({ "skills": ["skills"], "mcpServers": [{ "name": "off-mcp", "command": "x" }] }),
    );
    write_skill(&dir, "off-skill");
    fixture.state(&[("off", false)]);

    let contribs = load_from(fixture.apps_dir());

    assert!(contribs.skills.is_empty());
    assert!(contribs.mcp.is_empty());
    assert!(contribs.hooks.is_empty());
}

#[test]
fn keeps_single_declared_skill_file_only() {
    let fixture = Fixture::new("file");
    let dir = fixture.app("single", json!({ "skills": ["skills/demo-skill/SKILL.md"] }));
    write_skill(&dir, "demo-skill");
    write_skill(&dir, "other-skill");
    fixture.state(&[("single", true)]);

    let contribs = load_from(fixture.apps_dir());

    assert_eq!(contribs.skills.len(), 1);
    let names: Vec<String> = contribs.skills[0]
        .list(None)
        .into_iter()
        .map(|candidate| candidate.name)
        .collect();
    assert_eq!(names, vec!["demo-skill".to_string()]);
}

#[test]
fn bus_handler_kept_out_of_command_config() {
    let fixture = Fixture::new("bus");
    fixture.app(
        "app",
        json!({
            "hooks": [
                { "event": "PreToolUse", "handler": { "type": "bus", "callback": "onPre" } },
                { "event": "PostToolUse", "handler": { "type": "command", "command": "echo ok" } },
            ],
        }),
    );
    fixture.state(&[("app", true)]);

    let contribs = load_from(fixture.apps_dir());

    assert!(contribs.notes.iter().any(|note| note.contains("bus 处理器暂不支持")));
    let hooks = contribs.hooks_config(Path::new("C:/project")).expect("hooks 配置");
    assert!(hooks["hooks"]["PreToolUse"].is_null());
    assert_eq!(hooks["hooks"]["PostToolUse"][0]["hooks"][0]["command"], "echo ok");
}

#[test]
fn validate_rejects_path_escape_and_missing_skill() {
    let fixture = Fixture::new("validate");
    let dir = fixture.app("bad", json!({ "skills": ["../outside", "skills"] }));

    let error = validate(
        &serde_json::from_str(&fs::read_to_string(dir.join("manifest.json")).expect("读 manifest"))
            .expect("解析 manifest"),
        &dir,
    )
    .expect_err("应校验失败");

    assert!(error.contains("../outside"));
    assert!(error.contains("skills 路径 `skills` 不存在"));
}
