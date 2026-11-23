//! `service` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//! 覆盖 plan-review 三分支：Approve / Keep planning / Dismissed（D3）。

use std::sync::Arc;

use tokio::sync::mpsc;

use super::{PlanMode, PlanVerdict};

/// 构造一个已接线、已注入审批通道的 PlanMode。
fn wired() -> (Arc<PlanMode>, mpsc::UnboundedReceiver<super::PlanReviewRequest>) {
    let plan_mode = Arc::new(PlanMode::new());
    let (tx, rx) = mpsc::unbounded_channel();
    plan_mode.set_review_channel(tx);
    plan_mode.enter();
    (plan_mode, rx)
}

#[tokio::test]
async fn review_approve_exits_plan_mode() {
    let (plan_mode, mut rx) = wired();
    let handle = {
        let plan_mode = plan_mode.clone();
        tokio::spawn(async move { plan_mode.review("# plan".to_string()).await })
    };
    let request = rx.recv().await.unwrap();
    request.reply.send(PlanVerdict::Approve).unwrap();
    assert!(handle.await.unwrap().unwrap());
    assert!(!plan_mode.active(), "批准后应退出 plan 模式");
}

#[tokio::test]
async fn review_keep_planning_stays_in_plan_mode() {
    let (plan_mode, mut rx) = wired();
    let handle = {
        let plan_mode = plan_mode.clone();
        tokio::spawn(async move { plan_mode.review("# plan".to_string()).await })
    };
    let request = rx.recv().await.unwrap();
    request.reply.send(PlanVerdict::KeepPlanning).unwrap();
    let result = handle.await.unwrap();
    assert!(result.is_err());
    assert!(plan_mode.active(), "keep planning 后应保持 plan 模式");
}

#[tokio::test]
async fn review_dismissed_stays_in_plan_mode_and_waits() {
    let (plan_mode, mut rx) = wired();
    let handle = {
        let plan_mode = plan_mode.clone();
        tokio::spawn(async move { plan_mode.review("# plan".to_string()).await })
    };
    let request = rx.recv().await.unwrap();
    request.reply.send(PlanVerdict::Dismissed).unwrap();
    let result = handle.await.unwrap();
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("plan review dismissed"), "err: {err}");
    assert!(plan_mode.active(), "dismissed 后应保持 plan 模式并等待下一条消息");
}

#[tokio::test]
async fn review_closed_channel_counts_as_dismissed() {
    let (plan_mode, mut rx) = wired();
    let handle = {
        let plan_mode = plan_mode.clone();
        tokio::spawn(async move { plan_mode.review("# plan".to_string()).await })
    };
    let request = rx.recv().await.unwrap();
    // 宿主接管：drop reply，等价通道关闭 → dismissed。
    drop(request.reply);
    let result = handle.await.unwrap();
    assert!(result.is_err());
    assert!(plan_mode.active(), "通道关闭后应保持 plan 模式");
}
