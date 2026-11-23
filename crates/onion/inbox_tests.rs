//! `inbox` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::*;

fn inbox() -> Inbox {
    Inbox::new(EventBus::new())
}

#[test]
fn partitions_and_wakeup_flags() {
    let inbox = inbox();
    // followup → next-turn + 唤醒
    inbox.followup(AgentMessage::user("a")).unwrap();
    assert!(inbox.has_next_turn());
    assert!(inbox.take_wakeup());
    assert!(!inbox.take_wakeup()); // 取走即清除
    // steer → next-step + 唤醒
    inbox.steer(AgentMessage::user("b")).unwrap();
    assert!(inbox.has_next_step());
    assert!(inbox.take_wakeup());
    // inject → next-step 不唤醒
    inbox.inject(AgentMessage::user("c")).unwrap();
    assert!(inbox.has_next_step());
    assert!(!inbox.take_wakeup());
    // 两分区互不影响
    assert!(inbox.has_pending());
}

#[test]
fn claim_open_takes_whole_batch_in_order() {
    let inbox = inbox();
    inbox.followup(AgentMessage::user("first")).unwrap();
    inbox.followup(AgentMessage::user("second")).unwrap();
    // 开轮批量领取：同批投递的全量 next-turn 按入队序一次取走（#15 单 turn 合并）。
    let batch = inbox.claim_open();
    let texts: Vec<String> = batch.iter().map(|message| message.text_content()).collect();
    assert_eq!(texts, vec!["first".to_string(), "second".to_string()]);
    // claim 即移除：再取为空。
    assert!(inbox.claim_open().is_empty());
}

#[test]
fn drain_next_step_takes_whole_batch_in_order() {
    let inbox = inbox();
    inbox.steer(AgentMessage::user("s1")).unwrap();
    inbox.inject(AgentMessage::user("s2")).unwrap();
    let batch = inbox.drain_next_step();
    let texts: Vec<String> = batch.iter().map(|m| m.text_content()).collect();
    assert_eq!(texts, vec!["s1", "s2"]);
    assert!(!inbox.has_next_step());
    assert!(inbox.drain_next_step().is_empty());
}

#[test]
fn reject_keeps_messages_removed() {
    let inbox = inbox();
    inbox.followup(AgentMessage::user("blocked")).unwrap();
    let _claimed = inbox.claim_open();
    // 领取后不回队列（持久取消语义）
    assert!(!inbox.has_next_turn());
    assert!(!inbox.has_pending());
}

#[test]
fn full_partition_returns_error() {
    let inbox = inbox();
    for index in 0..INBOX_CAPACITY {
        inbox.followup(AgentMessage::user(format!("m{index}"))).unwrap();
    }
    let error = inbox.followup(AgentMessage::user("overflow")).unwrap_err();
    assert_eq!(error, InboxError::Full("next-turn", INBOX_CAPACITY));
}
