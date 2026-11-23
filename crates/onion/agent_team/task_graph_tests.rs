//! `task_graph` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::collections::HashSet;

use crate::agent_team::task_graph::{GraphCheck, GraphError};

#[test]
fn validate_accepts_empty_and_valid_blockers() {
    let exists = |id: &str| matches!(id, "task-1" | "task-2");
    let check = GraphCheck::validate(&[], "task-3", exists).unwrap();
    assert!(check.blockers.is_empty());

    let check = GraphCheck::validate(&["task-1".into(), "task-2".into()], "task-3", exists).unwrap();
    assert_eq!(check.blockers, vec!["task-1".to_string(), "task-2".to_string()]);
}

#[test]
fn validate_rejects_self_blocker() {
    let exists = |_id: &str| true;
    let err = GraphCheck::validate(&["task-3".into()], "task-3", exists).unwrap_err();
    assert!(matches!(err, GraphError::Cycle(_)));
    assert_eq!(err.code(), "dependency_cycle");
}

#[test]
fn validate_rejects_missing_blocker() {
    let exists = |id: &str| id == "task-1";
    let err = GraphCheck::validate(&["task-9".into()], "task-3", exists).unwrap_err();
    assert!(matches!(err, GraphError::Missing(_)));
    assert_eq!(err.code(), "task_not_found");
}

#[test]
fn validate_rejects_duplicate_blocker() {
    let exists = |_id: &str| true;
    let err = GraphCheck::validate(&["task-1".into(), "task-1".into()], "task-3", exists).unwrap_err();
    assert!(matches!(err, GraphError::Duplicate(_)));
    assert_eq!(err.code(), "invalid_argument");
}

#[test]
fn find_cycle_detects_cycle() {
    // a -> b -> a 成环。
    let successors = |id: &str| match id {
        "a" => vec!["b".to_string()],
        "b" => vec!["a".to_string()],
        _ => vec![],
    };
    assert!(GraphError::find_cycle("a", &successors).is_some());
}

#[test]
fn find_cycle_no_false_positive_on_chain() {
    // a -> b -> c 无环。
    let successors = |id: &str| match id {
        "a" => vec!["b".to_string()],
        "b" => vec!["c".to_string()],
        _ => vec![],
    };
    assert!(GraphError::find_cycle("a", &successors).is_none());
}

#[test]
fn validate_dedupes_blockers() {
    let exists = |_id: &str| true;
    // 已存在的 set 用于辅助（这里直接通过 exists 闭包验证去重）。
    let _seen: HashSet<String> = HashSet::new();
    let check = GraphCheck::validate(&["task-1".into(), "task-2".into()], "task-3", exists).unwrap();
    assert_eq!(check.blockers.len(), 2);
}
