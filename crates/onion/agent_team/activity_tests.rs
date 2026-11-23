//! `activity` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use crate::agent_team::activity::{wait_for_change, wait_impl, WaitResult};
use crate::agent_team::journal::Notify;

#[tokio::test(flavor = "multi_thread")]
async fn notify_wakes_waiter() {
    let notify = Notify::new();
    let notify2 = notify.clone();
    let notifier = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        notify2.notify();
    });
    let result = wait_impl(&notify, 5000, false).await;
    notifier.await.unwrap();
    assert_eq!(result, WaitResult { timed_out: false, closed: false });
}

#[tokio::test]
async fn timeout_returns_timed_out() {
    let notify = Notify::new();
    let result = wait_impl(&notify, 100, false).await;
    assert_eq!(result, WaitResult { timed_out: true, closed: false });
}

#[tokio::test]
async fn closed_returns_immediately() {
    let notify = Notify::new();
    let result = wait_impl(&notify, 100, true).await;
    assert_eq!(result, WaitResult { timed_out: false, closed: true });
}

#[tokio::test]
async fn invalid_timeout_rejected_by_public_api() {
    // 公开 API 强制 [10s, 1h] 区间。
    let notify = Notify::new();
    assert!(wait_for_change(&notify, 100, false).await.is_err());
    assert!(wait_for_change(&notify, 3_700_000, false).await.is_err());
}
