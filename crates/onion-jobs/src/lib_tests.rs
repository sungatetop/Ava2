//! `onion-jobs` crate 单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;

fn spec(id: &str, owner: &str) -> JobSpec {
    JobSpec {
        id: JobId(id.to_string()),
        owner: SessionId(owner.to_string()),
        kind: JobKind::Shell,
        command: format!("echo {id}"),
    }
}

#[test]
fn settle_is_first_wins() {
    let registry = JobRegistry::new();
    registry.register(spec("j1", "s1"), OutputRing::new());
    assert!(registry.settle("j1", JobOutcome::Exited { code: 0 }));
    // 二次 settle 不覆盖已落地事实。
    assert!(!registry.settle("j1", JobOutcome::Exited { code: 1 }));
    assert_eq!(
        registry.get("j1").map(|view| view.status),
        Some(JobStatus::Settled(JobOutcome::Exited { code: 0 }))
    );
}

#[test]
fn kill_then_settle_keeps_killed() {
    let registry = JobRegistry::new();
    registry.register(spec("j1", "s1"), OutputRing::new());
    assert_eq!(registry.kill("j1"), KillResult::Requested);
    // kill 已落地 Killed，后续真实退出结果不覆盖。
    assert!(!registry.settle("j1", JobOutcome::Exited { code: 0 }));
    assert_eq!(
        registry.get("j1").map(|view| view.status),
        Some(JobStatus::Settled(JobOutcome::Killed))
    );
}

#[test]
fn kill_reports_terminal_states() {
    let registry = JobRegistry::new();
    registry.register(spec("j1", "s1"), OutputRing::new());
    registry.settle("j1", JobOutcome::Exited { code: 0 });
    assert_eq!(registry.kill("j1"), KillResult::AlreadySettled);
    assert_eq!(registry.kill("missing"), KillResult::NotFound);
}

#[test]
fn list_is_owner_scoped_and_sorted() {
    let registry = JobRegistry::new();
    registry.register(spec("j2", "s1"), OutputRing::new());
    registry.register(spec("j1", "s1"), OutputRing::new());
    registry.register(spec("j3", "s2"), OutputRing::new());
    let ids: Vec<String> = registry
        .list(&SessionId("s1".to_string()))
        .into_iter()
        .map(|view| view.spec.id.0)
        .collect();
    assert_eq!(ids, vec!["j1".to_string(), "j2".to_string()]);
}

#[test]
fn cleanup_owner_removes_only_own_jobs() {
    let registry = JobRegistry::new();
    registry.register(spec("j1", "s1"), OutputRing::new());
    registry.register(spec("j2", "s1"), OutputRing::new());
    registry.register(spec("j3", "s2"), OutputRing::new());
    assert_eq!(registry.cleanup_owner(&SessionId("s1".to_string())), 2);
    assert!(registry.get("j1").is_none());
    assert!(registry.get("j3").is_some());
}

#[test]
fn output_ring_is_bounded_and_marks_truncation() {
    let mut ring = OutputRing::with_capacity(2);
    ring.push("a\nb\nc\n");
    assert_eq!(ring.tail(10), "b\nc");
    assert!(ring.truncated());
}

#[test]
fn outcome_success_only_on_zero_exit() {
    assert!(JobOutcome::Exited { code: 0 }.is_success());
    assert!(!JobOutcome::Exited { code: 1 }.is_success());
    assert!(!JobOutcome::Killed.is_success());
    assert!(!JobOutcome::Failed { error: "x".to_string() }.is_success());
}
