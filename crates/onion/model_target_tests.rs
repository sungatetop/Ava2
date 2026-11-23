//! model_target 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;

fn target(model: &str) -> ModelTarget {
    ModelTarget {
        model: model.to_string(),
        base_url: "http://x/v1".to_string(),
        api_key: "key".to_string(),
        effort: None,
    }
}

#[test]
fn new_sets_current_and_assembled_from_initial() {
    let r = ModelTargetRef::new(target("Qwen3.8-27B"));
    assert_eq!(r.current().unwrap().model, "Qwen3.8-27B");
    assert_eq!(r.assembled().unwrap().model, "Qwen3.8-27B");
}

#[test]
fn swap_current_changes_next_target_but_not_assembled() {
    let r = ModelTargetRef::new(target("A"));
    r.swap_current(target("B"));
    // current 已置换；assembled 仍为初始快照。
    assert_eq!(r.current().unwrap().model, "B");
    assert_eq!(r.assembled().unwrap().model, "A");
}

#[test]
fn snapshot_folds_current_into_assembled() {
    let r = ModelTargetRef::new(target("A"));
    r.swap_current(target("B"));
    r.snapshot();
    assert_eq!(r.assembled().unwrap().model, "B");
    assert_eq!(r.current().unwrap().model, "B");
}

#[test]
fn snapshot_preserves_effort() {
    let mut t = target("A");
    t.effort = Some("high".to_string());
    let r = ModelTargetRef::new(t);
    r.snapshot();
    assert_eq!(r.assembled().unwrap().effort.as_deref(), Some("high"));
}
