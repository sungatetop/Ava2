//! per-Lead 事务排序 + 事件提交发布。
//!
//! 同一 Lead 的所有「读-校验-写」操作经 `transact` 串行化，保证事件顺序与任务
//! revision 连续性。`append` 只做提交发布（同步 notify activity 等）；实际 durable
//! 写入由 service 层注入的 `AppendFn` 完成（本模块不直接依赖 SessionLog）。

use std::future::Future;
use std::sync::Arc;

use tokio::sync::{watch, Mutex};

use crate::agent_team::projection;
use crate::agent_team::projection::TeamEvent;
use crate::agent_team::types::TeamState;

/// 追加一个团队事件的函数（由 service 注入：写 SessionLog JSONL + 触发投影）。
pub type AppendFn = Arc<dyn Fn(TeamEvent) -> Result<(), String> + Send + Sync>;

/// 团队变更通知（waitForChange 唤醒）。
///
/// 用 `watch::Sender<u64>` 做版本号递增广播：`notify()` 递增版本唤醒所有 waiter；
/// `subscribe()` 返回当前版本 + 接收端；`close()` 关闭 channel 唤醒并终止所有 waiter。
/// 版本号用独立 `AtomicU64` 计数（避免在 send 中 borrow watch 值导致时序问题）。
#[derive(Clone)]
pub struct Notify {
    tx: watch::Sender<u64>,
    version: Arc<std::sync::atomic::AtomicU64>,
}

impl Notify {
    pub fn new() -> Self {
        let (tx, _rx) = watch::channel(0u64);
        Self {
            tx,
            version: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }
    /// 订阅版本号变化（返回当前版本 + 接收端）。
    pub fn subscribe(&self) -> (u64, watch::Receiver<u64>) {
        let v = self.version.load(std::sync::atomic::Ordering::SeqCst);
        (v, self.tx.subscribe())
    }
    /// 广播唤醒（递增版本）。
    pub fn notify(&self) {
        let v = self.version.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        let _ = self.tx.send(v);
    }
    /// 关闭：终止所有 waiter（watch 关闭后 recv 返回 Err）。
    pub fn close(&self) {
        self.version.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.tx.send_replace(u64::MAX);
    }
}

impl Default for Notify {
    fn default() -> Self {
        Self::new()
    }
}

/// per-Lead 事务队列（按 Lead 各自串行排队提交）。
pub struct Journal {
    append: Mutex<AppendFn>,
    notify: Notify,
    tail: Mutex<Option<PinBoxFuture>>,
    state: Mutex<TeamState>,
}

type PinBoxFuture = std::pin::Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

impl Journal {
    pub fn new(append: AppendFn, notify: Notify) -> Self {
        Self {
            append: Mutex::new(append),
            notify,
            tail: Mutex::new(None),
            state: Mutex::new(projection::empty_team_state()),
        }
    }

    /// 以真实 append 替换构造时的 no-op（装配完成后由 service `wire()` 调用）。
    pub async fn set_append(&self, append: AppendFn) {
        *self.append.lock().await = append;
    }

    /// 同步变体：供同步装配路径（`assemble_runtime`）在无并发持有者时安全替换。
    /// 用 `try_lock`（与 `Runtime::set_model_target_sync` 同理）——装配发生在
    /// tokio 驱动线程上，`blocking_lock` 会 panic；装配期锁必空闲。
    pub fn set_append_sync(&self, append: AppendFn) -> std::io::Result<()> {
        let mut guard = self
            .append
            .try_lock()
            .map_err(|_| std::io::Error::other("journal append 锁被并发持有：装配期不应存在其它持有者"))?;
        *guard = append;
        Ok(())
    }

    /// 原子读当前投影状态的快照。
    pub async fn snapshot(&self) -> TeamState {
        self.state.lock().await.clone()
    }

    /// 在 per-Lead 串行队列中执行一个读写操作。
    ///
    /// `op` 接收当前状态快照，返回「要 append 的事件列表 + 可选结果」。事件按序
    /// append（提交发布），随后投影增量应用，最后 notify。
    pub async fn transact<T, F>(&self, op: F) -> Result<T, String>
    where
        T: Send + 'static,
        F: FnOnce(TeamState) -> Result<(Vec<TeamEvent>, T), String> + Send + 'static,
    {
        // 串行化：等待前一个 tail 完成。
        let prior = self.tail.lock().await.take();
        if let Some(prior) = prior {
            prior.await;
        }

        let (events, result) = {
            let state = self.state.lock().await.clone();
            op(state)?
        };

        {
            // 先取 append（含 await 点）再锁 state，避免持 MutexGuard 跨 await。
            let append = self.append.lock().await.clone();
            let mut guard = self.state.lock().await;
            for event in &events {
                append(event.clone())?;
                projection::apply_event(&mut guard, event).map_err(|e| e.to_string())?;
            }
        }
        if !events.is_empty() {
            self.notify.notify();
        }

        // 登记新 tail（空操作，保持链），供下一个 transact 等待。
        *self.tail.lock().await = Some(Box::pin(async {}));

        Ok(result)
    }
}

#[cfg(test)]
#[path = "journal_tests.rs"]
mod tests;
