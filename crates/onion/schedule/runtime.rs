//! Host 定时器：扫描到期任务、恢复 Session、投递提醒、提交状态。
//!
//! - 一次性到期 → `status: inactive`；
//! - 循环到期 → 推进 `scheduled_at = next_scheduled_at`（无 next → inactive）；
//! - 递送后追加递送历史（`append_delivery`）并一并提交；
//! - 递送失败仅记录，不自动重试；
//! - 重新计算最近未来义务，`tokio::time` 定时。
//!
//! onion 无独立 Host 进程：`ScheduleRuntime` 经注入的 `resolve` + `followup` 闭包
//! 递送（由 front door 提供），`drive()` 可在装配后由 front door 手动调用（demo
//! 主循环轮询）或经 `spawn` 后台任务驱动。

use std::sync::Arc;

use crate::types::{AgentMessage, MessageSource};

use super::domain;
use super::framing;
use super::history::append_delivery;
use super::storage::HostScheduleStore;
use super::types::{
    DeliveryReceipt, DeliveryRetentionBounds, ScheduleRecord, ScheduleTask, TaskStatus,
};

/// Node 定时器上限的语义锚点（约 24.8 天）；tokio 无此限制，保留常量以约束单次定时不超过该值。
pub const MAX_TIMER_DELAY_MS: u64 = 2_147_483_647;

/// 到期递送的一次性驱动。
///
/// 由 front door 注入 `resolve`（恢复 Session，返回 [`Deliver`] 句柄）。返回
/// 投递的消息 id 列表（供测试断言）。
pub async fn drive_once<S>(
    store: &Arc<HostScheduleStore>,
    retention: DeliveryRetentionBounds,
    resolve: S,
) -> Vec<String>
where
    S: Fn(&str) -> Option<Arc<dyn Deliver>>,
{
    let now = crate::session::epoch_millis() as i64;
    let due = store.all().into_iter().filter(|task| is_due(task, now)).collect::<Vec<_>>();
    let mut delivered = Vec::new();
    for task in due {
        let Some(agent) = resolve(&task.session_id) else { continue };
        if let Ok(id) = deliver_task(store, &task, retention, &*agent).await {
            delivered.push(id);
        }
    }
    delivered
}

/// 任务是否到期（active 且 scheduled_at <= now）。
pub fn is_due(task: &ScheduleTask, now: i64) -> bool {
    task.status == TaskStatus::Active && parse_ms(task.record.scheduled_at()) <= now
}

/// [`ScheduleError`] 文案摘要（未实现 Display；code + 变体消息，供 front door
/// 错误映射复用）。
pub fn schedule_error_text(error: &super::types::ScheduleError) -> String {
    use super::types::ScheduleError;
    match error {
        ScheduleError::InvalidPrompt(message)
        | ScheduleError::InvalidSelector(message)
        | ScheduleError::InvalidRule(message)
        | ScheduleError::InvalidTimeZone(message) => message.clone(),
        ScheduleError::NotFuture => "目标时间必须在未来".to_string(),
        ScheduleError::TimeOutOfRange => "时间超出可表示范围".to_string(),
        ScheduleError::FrequencyTooHigh => "循环频率过高（下限 60s）".to_string(),
        ScheduleError::SubagentSession => "subagent 会话不支持日程".to_string(),
        ScheduleError::Internal => "日程内部错误".to_string(),
    }
}

/// 递送单个到期任务并记账（历史 + 状态推进）。[`drive_once`] 的循环体。
///
/// 投递或记账失败返回 Err（原因字符串），任务保持原状、下轮重试。
pub async fn deliver_task(
    store: &Arc<HostScheduleStore>,
    task: &ScheduleTask,
    retention: DeliveryRetentionBounds,
    agent: &dyn Deliver,
) -> Result<String, String> {
    let now = crate::session::epoch_millis() as i64;
    deliver_task_at(store, task, retention, agent, now).await
}

/// 「立即执行」变体（onion-server 任务端点）：到期语义等同 [`deliver_task`]；
/// 循环任务尚未到期时按已承诺目标记账（occurrence 取首个未来目标，本视为
/// 消费该 occurrence，`scheduled_at` 推进到下一目标）；一次性任务无守卫，
/// 直接收尾转 inactive。
pub async fn deliver_task_now(
    store: &Arc<HostScheduleStore>,
    task: &ScheduleTask,
    retention: DeliveryRetentionBounds,
    agent: &dyn Deliver,
) -> Result<String, String> {
    let now = crate::session::epoch_millis() as i64;
    let accepted_at = if domain::is_recurring(&task.record) {
        now.max(parse_ms(task.record.scheduled_at()))
    } else {
        now
    };
    deliver_task_at(store, task, retention, agent, accepted_at).await
}

/// 按给定 `accepted_at` 递送并记账（[`deliver_task`] / [`deliver_task_now`] 共同实现）。
async fn deliver_task_at(
    store: &Arc<HostScheduleStore>,
    task: &ScheduleTask,
    retention: DeliveryRetentionBounds,
    agent: &dyn Deliver,
    accepted_at: i64,
) -> Result<String, String> {
    let text = if domain::is_recurring(&task.record) {
        let occurrence = domain::resolve_recurring_occurrence(&task.record, accepted_at)
            .map_err(|error| schedule_error_text(&error))?;
        framing::render_recurring_batch_framing(&[(&task.record, occurrence.occurrence_at)])
    } else {
        framing::render_reminder_framing(&task.record)
    };
    let message_id = agent.followup(&text)?;
    let delivered_at = format_utc_now();
    let receipt = DeliveryReceipt {
        scheduled_at: task.record.scheduled_at().to_string(),
        delivered_at,
        message_id: message_id.clone(),
    };
    let (last_delivery, delivery_history) = append_delivery(task, receipt, retention);
    let updated = if domain::is_recurring(&task.record) {
        let occurrence = domain::resolve_recurring_occurrence(&task.record, accepted_at)
            .map_err(|error| schedule_error_text(&error))?;
        let mut record = task.record.clone();
        let status = if occurrence.next_scheduled_at.is_none() {
            TaskStatus::Inactive
        } else {
            TaskStatus::Active
        };
        set_scheduled_at(
            &mut record,
            occurrence
                .next_scheduled_at
                .unwrap_or(occurrence.occurrence_at),
        );
        ScheduleTask {
            record,
            status,
            last_delivery: Some(last_delivery),
            delivery_history: Some(delivery_history),
            ..task.clone()
        }
    } else {
        ScheduleTask {
            status: TaskStatus::Inactive,
            last_delivery: Some(last_delivery),
            delivery_history: Some(delivery_history),
            ..task.clone()
        }
    };
    store
        .put(updated)
        .map_err(|error| format!("schedule 存储 commit 失败: {error}"))?;
    Ok(message_id)
}

/// 递送目标抽象：front door 提供 Session 投递句柄（onion `Runtime` 或测试桩）。
pub trait Deliver: Send + Sync {
    /// 向该 Session 投递一条 user 提醒消息，返回消息 id。
    fn followup(&self, text: &str) -> Result<String, String>;
}

/// 构造 schedule 来源的 user 提醒消息（带 `MessageSource::Schedule` 标记）。
pub fn scheduled_message(text: &str) -> AgentMessage {
    let mut message = AgentMessage::user(text.to_string());
    message.source = Some(MessageSource::Schedule);
    message
}

/// 解析规范 UTC instant 为毫秒。
fn parse_ms(instant: &str) -> i64 {
    crate::schedule::time::parse_utc_instant(instant).unwrap_or(i64::MAX)
}

/// 当前 UTC 时刻的规范字符串。
pub fn format_utc_now() -> String {
    crate::schedule::time::format_utc(crate::session::epoch_millis() as i64)
}

/// 原地替换 record 的 scheduled_at（保持 kind）。
fn set_scheduled_at(record: &mut ScheduleRecord, scheduled_at: String) {
    match record {
        ScheduleRecord::After(r) => r.scheduled_at = scheduled_at,
        ScheduleRecord::At(r) => r.scheduled_at = scheduled_at,
        ScheduleRecord::Every(r) => r.scheduled_at = scheduled_at,
        ScheduleRecord::Daily(r) => r.scheduled_at = scheduled_at,
        ScheduleRecord::Weekly(r) => r.scheduled_at = scheduled_at,
        ScheduleRecord::Cron(r) => r.scheduled_at = scheduled_at,
    }
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
