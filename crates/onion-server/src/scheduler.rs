//! 全局定时任务调度器：后台循环驱动 onion schedule 递送（onion 无独立 Host
//! 进程，front door 职责由本模块承担）。
//!
//! 10s tick 两阶段：
//! 1. **物化**：`independent` 模式的到期任务（`session_id` 为空）新建会话并
//!    回填 id（循环任务每次触发都新建，下一轮再次为空）；
//! 2. **递送**：预恢复到期任务的绑定会话（`registry.open` 为 async，而
//!    `drive_once` 的 resolve 是同步闭包，故先开再查表），`drive_once` 投递
//!    并记账（一次性 → inactive；循环 → 推进 scheduled_at）。
//!
//! 会话打开失败（磁盘日志缺失/恢复异常）本轮跳过，下轮重试。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use onion::schedule::runtime::{self, Deliver};
use onion::schedule::storage::HostScheduleStore;
use onion::schedule::types::ScheduleTask;

use crate::session::{ServerSession, SessionRegistry, spawn_turn};

/// tick 周期（对齐 ava-app 调度 tick 量级；1s 粒度时序由 record 语义保证）。
const TICK: Duration = Duration::from_secs(10);

/// [`ServerSession`] 的递送句柄：投递 schedule 消息并唤醒轮次 driver。
/// 实现 onion `Deliver`（同步投递：inbox 入队 + spawn turn）。
pub(crate) struct SessionDeliver(pub(crate) Arc<ServerSession>);

impl Deliver for SessionDeliver {
    fn followup(&self, text: &str) -> Result<String, String> {
        let message = runtime::scheduled_message(text);
        self.0
            .handle
            .runtime
            .inbox()
            .followup(message)
            .map_err(|error| error.to_string())?;
        spawn_turn(&self.0.handle);
        // inbox.followup 无回执；消息 id 用进程内毫秒时间戳保证唯一可追溯。
        Ok(format!("schedule-{}", onion::session::epoch_millis()))
    }
}

/// 启动后台调度循环（进程生命周期；无停止句柄——onion-server 无优雅停机语义）。
pub(crate) fn spawn(registry: SessionRegistry) {
    let store = registry.schedule_store();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(TICK);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            if let Err(error) = drive_tick(&registry, &store).await {
                eprintln!("[onion-server] schedule 调度轮失败: {error}");
            }
        }
    });
}

/// 单轮调度：物化 independent 任务 → 预开目标会话 → drive_once 递送。
async fn drive_tick(
    registry: &SessionRegistry,
    store: &Arc<HostScheduleStore>,
) -> Result<(), String> {
    let now = onion::session::epoch_millis() as i64;
    // 1) 独立会话物化：到期且未绑定会话 → 新建（interactive，可被用户接手）。
    for task in store.all() {
        if !runtime::is_due(&task, now) {
            continue;
        }
        if task.session_mode.as_deref() != Some("independent") || !task.session_id.is_empty() {
            continue;
        }
        let session = registry
            .create(None, None, true, Some(registry.workspace_root().display().to_string()))
            .await
            .map_err(|error| error.to_string())?;
        let updated = ScheduleTask { session_id: session.id.clone(), ..task };
        store.put(updated).map_err(|error| error.to_string())?;
    }
    // 2) 预开到期任务的目标会话（single 绑定会话 / 已物化的 independent）。
    let now = onion::session::epoch_millis() as i64;
    let mut sessions: HashMap<String, Arc<dyn Deliver>> = HashMap::new();
    for task in store.all() {
        if !runtime::is_due(&task, now) || task.session_id.is_empty() {
            continue;
        }
        if sessions.contains_key(&task.session_id) {
            continue;
        }
        if let Ok(Some(session)) = registry.open(&task.session_id, true).await {
            sessions.insert(task.session_id.clone(), Arc::new(SessionDeliver(session)));
        }
    }
    // 3) 递送（resolve 同步查预开表）。
    runtime::drive_once(store, onion::schedule::types::DeliveryRetentionBounds::default(), |id| {
        sessions.get(id).cloned()
    })
    .await;
    Ok(())
}
