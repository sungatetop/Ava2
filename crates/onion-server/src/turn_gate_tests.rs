//! `turn_gate` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::sync::Arc;

use super::TurnGate;

#[test]
fn driver_is_exclusive() {
    let gate = TurnGate::new();
    assert!(gate.try_acquire_driver());
    // 已有 driver：第二次抢占失败（并发 POST /messages 只起一个 driver）。
    assert!(!gate.try_acquire_driver());
    gate.release_driver();
    assert!(gate.try_acquire_driver());
}

#[test]
fn busy_tracks_enqueued_minus_completed() {
    let gate = TurnGate::new();
    assert!(!gate.is_busy());
    let seq = gate.enqueue();
    assert_eq!(seq, 1);
    assert!(gate.is_busy());
    gate.complete_one();
    assert!(!gate.is_busy());
}

#[tokio::test]
async fn wait_for_returns_immediately_when_already_completed() {
    let gate = TurnGate::new();
    let seq = gate.enqueue();
    gate.complete_one();
    // 已追平：立即返回，不挂起。
    gate.wait_for(seq).await;
}

#[tokio::test]
async fn wait_for_each_request_waits_its_own_turn() {
    let gate = Arc::new(TurnGate::new());
    let first = gate.enqueue();
    let second = gate.enqueue();
    // 两个并发 wait=true 各自等自己的序号。
    let g1 = gate.clone();
    let h1 = tokio::spawn(async move { g1.wait_for(first).await });
    let g2 = gate.clone();
    let h2 = tokio::spawn(async move { g2.wait_for(second).await });
    gate.complete_one();
    h1.await.unwrap();
    // 第二轮未完成：第二个等待者仍挂起。
    assert!(!h2.is_finished());
    gate.complete_one();
    h2.await.unwrap();
}

#[tokio::test]
async fn drain_remaining_releases_waiters() {
    let gate = Arc::new(TurnGate::new());
    let seq = gate.enqueue();
    let g = gate.clone();
    let handle = tokio::spawn(async move { g.wait_for(seq).await });
    // driver 异常退出：兜底追平，避免 wait=true 悬挂。
    gate.drain_remaining();
    handle.await.unwrap();
    assert!(!gate.is_busy());
}
