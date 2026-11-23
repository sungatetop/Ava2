//! `preset` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//!
//! 覆盖 AgentDefinition / AgentCatalog / PresentationMode 的解析与合并语义：
//! 内置双预设（coding / work）工具名集合、YAML 自定义加载、自定义覆盖内置。

use std::path::PathBuf;

use super::{AgentCatalog, AgentFile, PresentationMode, coding_tools, work_tools};

/// 写一个临时 YAML 文件并返回其路径（测试结束由调用方清理）。
fn write_yaml(dir_tag: &str, body: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "onion_preset_{dir_tag}_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("agents.yml");
    std::fs::write(&path, body).unwrap();
    path
}

#[test]
fn builtin_catalog_has_coding_creator_and_work() {
    let catalog = AgentCatalog::builtin();
    let ids = catalog.ids();
    assert_eq!(ids, vec!["coding".to_string(), "creator".to_string(), "work".to_string()]);
    let coding = catalog.get("coding").unwrap();
    assert_eq!(coding.tools, coding_tools());
    assert_eq!(coding.presentation, PresentationMode::Native);
    let work = catalog.get("work").unwrap();
    assert_eq!(work.tools, work_tools());
    // creator（创造模式）：工具面同 coding（写代码 + shell 调安装 REST），提示词非空。
    let creator = catalog.get("creator").unwrap();
    assert_eq!(creator.name, "创造模式");
    assert_eq!(creator.tools, coding_tools());
    assert!(creator.system_prompt.contains("创造模式"));
    assert!(creator.system_prompt.contains("v1/apps/install"));
}

#[test]
fn coding_has_terminal_work_lacks_terminal_and_shell() {
    let coding = coding_tools();
    let work = work_tools();
    // coding 含 terminal 与 shell；work 不含。
    assert!(coding.contains(&"terminal_open".to_string()));
    assert!(coding.contains(&"bash".to_string()));
    assert!(coding.contains(&"pwsh".to_string()));
    assert!(!work.contains(&"terminal_open".to_string()));
    assert!(!work.contains(&"bash".to_string()));
    // 两者均含 schedule（work 侧重办公日程，coding 也保留）。
    assert!(coding.contains(&"schedule_create".to_string()));
    assert!(work.contains(&"schedule_create".to_string()));
}

#[test]
fn load_custom_parses_yaml_and_merges() {
    let path = write_yaml(
        "custom",
        "agents:\n  - id: researcher\n    name: Researcher\n    tools: [web_search, web_fetch]\n    system_prompt: Research agent\n    presentation: ptc\n",
    );
    let catalog = super::AgentCatalog::merged(Some(&path)).unwrap();
    // 内置双预设仍在。
    assert!(catalog.get("coding").is_some());
    assert!(catalog.get("work").is_some());
    // 自定义 Agent 已并入。
    let researcher = catalog.get("researcher").unwrap();
    assert_eq!(researcher.name, "Researcher");
    assert_eq!(researcher.tools, vec!["web_search".to_string(), "web_fetch".to_string()]);
    assert_eq!(researcher.presentation, PresentationMode::Ptc);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn custom_overrides_builtin_same_id() {
    let path = write_yaml(
        "override",
        "agents:\n  - id: coding\n    name: Custom Coding\n    tools: [read, write]\n",
    );
    let catalog = super::AgentCatalog::merged(Some(&path)).unwrap();
    let coding = catalog.get("coding").unwrap();
    assert_eq!(coding.name, "Custom Coding");
    assert_eq!(coding.tools, vec!["read".to_string(), "write".to_string()]);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn load_custom_missing_file_is_empty_catalog() {
    let catalog = super::AgentCatalog::load_custom(None).unwrap();
    assert!(catalog.ids().is_empty());
    let missing = std::env::temp_dir().join(format!("onion_preset_missing_{}", std::process::id()));
    let catalog = super::AgentCatalog::load_custom(Some(&missing)).unwrap();
    assert!(catalog.ids().is_empty());
}

#[test]
fn agent_file_deserialize_defaults() {
    let raw = "agents:\n  - id: a\n    name: A\n";
    let file: AgentFile = serde_yaml::from_str(raw).unwrap();
    assert_eq!(file.agents.len(), 1);
    let agent = &file.agents[0];
    assert_eq!(agent.tools, Vec::<String>::new());
    assert_eq!(agent.system_prompt, "");
    assert_eq!(agent.presentation, PresentationMode::Native);
}
