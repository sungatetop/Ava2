//! 双分区 Inbox：next-turn（排队成轮，开轮批量领取）/ next-step（step 边界整批领取）
//! + 唤醒标志。
//!
//! 消息路由约定：followup 入 next-turn 并唤醒、steer 入 next-step 并唤醒、
//! inject 入 next-step 不唤醒；claim 即移除（reject 保持移除 = 持久取消）。

use std::collections::VecDeque;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

use tokio::sync::Notify;

use crate::listener::EventSink;
use crate::session::{EventBus, LiveEvent};
use crate::types::AgentMessage;

const INBOX_CAPACITY: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Partition {
    NextTurn,
    NextStep,
}

impl Partition {
    pub fn label(self) -> &'static str {
        match self {
            Self::NextTurn => "next-turn",
            Self::NextStep => "next-step",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum InboxError {
    /// 目标分区已满（label, capacity）。
    Full(&'static str, usize),
}

impl fmt::Display for InboxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Full(label, capacity) => write!(f, "inbox 分区 {label} 已满（容量 {capacity}）"),
        }
    }
}

impl std::error::Error for InboxError {}

struct InboxInner {
    next_turn: VecDeque<AgentMessage>,
    next_step: VecDeque<AgentMessage>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[derive(Clone)]
pub struct Inbox {
    inner: Arc<Mutex<InboxInner>>,
    notify: Arc<Notify>,
    wake_flag: Arc<AtomicBool>,
    bus: EventBus,
    sink: Arc<Mutex<Option<EventSink>>>,
}

impl Inbox {
    pub fn new(bus: EventBus) -> Self {
        Self {
            inner: Arc::new(Mutex::new(InboxInner {
                next_turn: VecDeque::new(),
                next_step: VecDeque::new(),
            })),
            notify: Arc::new(Notify::new()),
            wake_flag: Arc::new(AtomicBool::new(false)),
            bus,
            sink: Arc::new(Mutex::new(None)),
        }
    }

    /// 注入 listener 扇出 sink（Runtime 装配期调用）。
    pub fn set_sink(&self, sink: EventSink) {
        *lock(&self.sink) = Some(sink);
    }

    fn fanout(&self, event: &LiveEvent) {
        // 克隆后调用：listener 会回调 steer/followup/inject，避免持锁重入。
        let sink = lock(&self.sink).clone();
        if let Some(sink) = sink.as_ref() {
            sink(event);
        }
    }

    fn emit(&self, event: LiveEvent) {
        self.bus.send(event.clone());
        self.fanout(&event);
    }

    /// next-turn 分区 + 唤醒：本轮结束后成轮（开轮批量领取全量 next-turn，
    /// 同批投递合并单 turn，见 [`claim_open`]）。
    pub fn followup(&self, message: AgentMessage) -> Result<(), InboxError> {
        self.push(Partition::NextTurn, message)?;
        self.wake_flag.store(true, Ordering::SeqCst);
        self.notify.notify_one();
        Ok(())
    }

    /// next-step 分区 + 唤醒：本轮下一个被接纳的 step 领取。
    pub fn steer(&self, message: AgentMessage) -> Result<(), InboxError> {
        self.push(Partition::NextStep, message)?;
        self.wake_flag.store(true, Ordering::SeqCst);
        self.notify.notify_one();
        Ok(())
    }

    /// next-step 分区不唤醒：等下一个被接纳的 step（不因它开新轮）。
    pub fn inject(&self, message: AgentMessage) -> Result<(), InboxError> {
        self.push(Partition::NextStep, message)
    }

    fn push(&self, partition: Partition, message: AgentMessage) -> Result<(), InboxError> {
        {
            let mut inner = lock(&self.inner);
            let queue = match partition {
                Partition::NextTurn => &mut inner.next_turn,
                Partition::NextStep => &mut inner.next_step,
            };
            if queue.len() >= INBOX_CAPACITY {
                return Err(InboxError::Full(partition.label(), INBOX_CAPACITY));
            }
            queue.push_back(message.clone());
        } // 先释放 inner 锁再扇出：listener 回调 steer/followup 需重取该锁。
        self.emit(LiveEvent::InboxInserted { partition: partition.label(), message });
        Ok(())
    }

    /// 开轮 Claiming：批量领取 next-turn 全量（claim 即移除；空队列返回空表）。
    ///
    /// 单 turn 合并呈现：subagent settlement 同批投递的 relay + notice 不拆多轮。
    /// 以开轮批量领取 next-turn 达成——inbox 内部锁下
    /// 原子消费，无投递窗口竞态。
    pub fn claim_open(&self) -> Vec<AgentMessage> {
        let messages: Vec<AgentMessage> = lock(&self.inner).next_turn.drain(..).collect();
        if !messages.is_empty() {
            self.emit(LiveEvent::InboxClaimed {
                partition: Partition::NextTurn.label(),
                messages: messages.clone(),
            });
        }
        messages
    }

    /// step 边界 Admitting 前：整批领取 next-step（按入队序）。
    pub fn drain_next_step(&self) -> Vec<AgentMessage> {
        let messages: Vec<AgentMessage> = lock(&self.inner).next_step.drain(..).collect();
        if !messages.is_empty() {
            self.emit(LiveEvent::InboxClaimed {
                partition: Partition::NextStep.label(),
                messages: messages.clone(),
            });
        }
        messages
    }

    pub fn has_next_turn(&self) -> bool {
        !lock(&self.inner).next_turn.is_empty()
    }

    pub fn has_next_step(&self) -> bool {
        !lock(&self.inner).next_step.is_empty()
    }

    /// 任一分区有排队的消息（驱动器自续判据）。
    pub fn has_pending(&self) -> bool {
        let inner = lock(&self.inner);
        !inner.next_turn.is_empty() || !inner.next_step.is_empty()
    }

    /// 清空两分区并清唤醒标志（interrupt 语义：连收件箱一并丢弃）。
    ///
    /// 返回被丢弃的消息条数（供调用方日志/审计）。
    pub fn clear(&self) -> usize {
        let removed = {
            let mut inner = lock(&self.inner);
            let count = inner.next_turn.len() + inner.next_step.len();
            inner.next_turn.clear();
            inner.next_step.clear();
            count
        };
        self.wake_flag.store(false, Ordering::SeqCst);
        removed
    }

    /// 取走并清除唤醒标志（驱动器每次开轮前调用）。
    pub fn take_wakeup(&self) -> bool {
        self.wake_flag.swap(false, Ordering::SeqCst)
    }

    /// 等待唤醒（外部事件循环用；demo 主循环用轮询，此 API 供将来挂起）。
    pub async fn notified(&self) {
        self.notify.notified().await;
    }
}
