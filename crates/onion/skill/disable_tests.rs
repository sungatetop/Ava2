//! `skill::disable` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use super::{manager_file_path, read_disabled_names, write_disabled_names, DisablePolicy, FileDisablePolicy};
use crate::skill::registry::SkillRegistry;
use crate::skill::{SkillRegistration, SkillSource};

/// 测试专用临时目录（按进程 id 隔离，测试结束清理）。
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!("ava_skill_disable_{}_{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp dir");
        Self(path)
    }

    fn project(&self, tag: &str) -> PathBuf {
        let path = self.0.join(tag);
        fs::create_dir_all(&path).expect("create project dir");
        // `.git` 存在使 find_project_root 稳定落在此目录。
        fs::create_dir_all(path.join(".git")).expect("create .git");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write_file(path: &PathBuf, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, content).expect("write file");
}

fn register_skill(registry: &mut SkillRegistry, name: &str) {
    registry.register(SkillRegistration {
        name: name.to_string(),
        description: format!("desc {name}"),
        when_to_use: None,
        source: SkillSource::Runtime,
        resource_base: None,
        content: format!("body of {name}"),
        invocation: None,
        provider: None,
    });
}

#[test]
fn read_missing_file_is_empty() {
    let dir = TempDir::new("missing");
    let project = dir.project("p");
    let path = project.join(".onion").join("skills-manager.json");
    assert!(read_disabled_names(&path).is_empty());
}

#[test]
fn read_invalid_json_fails_open() {
    let dir = TempDir::new("invalid");
    let project = dir.project("p");
    let path = project.join(".onion").join("skills-manager.json");
    write_file(&path, "{ not json");
    assert!(read_disabled_names(&path).is_empty());

    // 字段类型不符同样 fail-open。
    write_file(&path, r#"{"disabled": "commit-helper"}"#);
    assert!(read_disabled_names(&path).is_empty());
}

#[test]
fn read_filters_invalid_names_and_dedups() {
    let dir = TempDir::new("sanitize");
    let project = dir.project("p");
    let path = project.join(".onion").join("skills-manager.json");
    write_file(&path, r#"{"disabled": ["ok-name", "Bad_Name", "ok-name", "a", ""]}"#);
    let names = read_disabled_names(&path);
    let expected: std::collections::HashSet<String> = ["a", "ok-name"].into_iter().map(String::from).collect();
    assert_eq!(names, expected);
}

#[test]
fn unknown_fields_ignored_and_empty_disabled_defaults() {
    let dir = TempDir::new("unknown");
    let project = dir.project("p");
    let path = project.join(".onion").join("skills-manager.json");
    write_file(&path, r#"{"future": 1, "disabled": []}"#);
    assert!(read_disabled_names(&path).is_empty());
    write_file(&path, "{}");
    assert!(read_disabled_names(&path).is_empty());
}

#[test]
fn write_creates_dir_and_roundtrips() {
    let dir = TempDir::new("write");
    let project = dir.project("p");
    let path = project.join(".onion").join("skills-manager.json");
    let written = write_disabled_names(
        &project,
        &["zz-late".to_string(), "Bad_Name".to_string(), "aa-early".to_string(), "zz-late".to_string()],
    )
    .expect("write ok");
    assert_eq!(written, path);
    // 校验 + 去重 + 排序落盘。
    let body = fs::read_to_string(&path).expect("read back");
    assert_eq!(body, "{\n  \"disabled\": [\n    \"aa-early\",\n    \"zz-late\"\n  ]\n}\n");
    let names = read_disabled_names(&path);
    assert_eq!(names.len(), 2);
    assert!(names.contains("aa-early") && names.contains("zz-late"));
}

#[test]
fn manager_file_path_resolves_project_root() {
    let dir = TempDir::new("path");
    let project = dir.project("p");
    // project root 内相对路径解析。
    assert_eq!(
        manager_file_path(Some(&project.join("sub"))),
        Some(project.join(".onion").join("skills-manager.json"))
    );
    assert_eq!(manager_file_path(None), None);
}

#[test]
fn policy_wired_registry_filters_list_get_and_summary() {
    let dir = TempDir::new("policy");
    let project = dir.project("p");
    write_disabled_names(&project, &["hidden-skill".to_string()]).expect("write ok");

    let mut registry = SkillRegistry::new()
        .with_disable_policy(Arc::new(FileDisablePolicy) as Arc<dyn DisablePolicy>);
    register_skill(&mut registry, "visible-skill");
    register_skill(&mut registry, "hidden-skill");

    let names: Vec<String> = registry.list(Some(&project)).into_iter().map(|s| s.name).collect();
    assert_eq!(names, vec!["visible-skill".to_string()]);

    // 管理面读视图：list_all 含禁用技能（仅标记，不缺席）。
    let all_names: Vec<String> =
        registry.list_all(Some(&project)).into_iter().map(|s| s.name).collect();
    assert_eq!(all_names, vec!["hidden-skill".to_string(), "visible-skill".to_string()]);

    assert!(registry.get("hidden-skill", Some(&project)).is_none());
    assert!(registry.summary("hidden-skill", Some(&project)).is_none());
    assert!(registry.get("visible-skill", Some(&project)).is_some());
}

#[test]
fn policy_without_cwd_keeps_all_skills() {
    let mut registry =
        SkillRegistry::new().with_disable_policy(Arc::new(FileDisablePolicy) as Arc<dyn DisablePolicy>);
    register_skill(&mut registry, "any-skill");
    // cwd 缺失无法定位清单文件 → fail-open 全部保留。
    assert_eq!(registry.list(None).len(), 1);
    assert!(registry.get("any-skill", None).is_some());
}

#[test]
fn registry_without_policy_is_unchanged() {
    let mut registry = SkillRegistry::new();
    register_skill(&mut registry, "plain-skill");
    assert_eq!(registry.list(None).len(), 1);
    assert!(registry.get("plain-skill", None).is_some());
}
