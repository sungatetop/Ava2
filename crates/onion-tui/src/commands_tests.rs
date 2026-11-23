//! commands.rs 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）：命令提示过滤。

use super::{COMMAND_CATALOG, command_hints};

#[test]
fn plain_message_yields_no_hints() {
    assert!(command_hints("").is_empty());
    assert!(command_hints("hello").is_empty());
    assert!(command_hints(" /model").is_empty(), "前导空格不触发提示");
}

#[test]
fn bare_slash_lists_catalog_prefix() {
    let hints = command_hints("/");
    assert_eq!(hints.len(), 8, "空斜杠截断到 HINT_MAX");
    assert!(hints[0].text.starts_with("/new"));
    assert!(hints[0].text.contains("新建会话"));
    // 两列对齐：描述起始列固定（命令名列全 ASCII）。
    let desc_col = hints[0].text.find("新建会话").unwrap();
    assert_eq!(desc_col, 20);
}

#[test]
fn prefix_filters_catalog_entries() {
    let hints = command_hints("/mo");
    let names: Vec<&str> = hints.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(names.len(), 1);
    assert!(names[0].starts_with("/model"));
    // /agent 前缀触发 Agent 切换命令提示。
    let hints = command_hints("/ag");
    let names: Vec<&str> = hints.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(names.len(), 1);
    assert!(names[0].starts_with("/agent"));
}

#[test]
fn unknown_prefix_yields_no_hints() {
    assert!(command_hints("/zzz").is_empty());
}

#[test]
fn catalog_commands_all_start_with_slash_and_are_unique() {
    let mut names: Vec<&str> = COMMAND_CATALOG.iter().map(|(n, _)| *n).collect();
    assert!(names.iter().all(|n| n.starts_with('/')));
    assert!(names.iter().all(|n| n.len() <= 20), "命令名列宽不超过 20");
    names.sort();
    let count = names.len();
    names.dedup();
    assert_eq!(names.len(), count, "命令名不得重复");
}
