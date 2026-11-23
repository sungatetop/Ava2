//! ScheduleService：Host 独立存储 CRUD（create/list/catalog/delete/update/history）。
//!
//! 任务存 Host 独立 `tasks` 表，绑定原
//! Session，独立于 Session 事件流；create/update/delete 写 Host 行（不产生 session
//! 事件）；`subagent_session` 拒绝由调用方（工具层）判定，service 层保留语义边界。
//!
//! 核心语义：
//! - `create`：校验恰好一个 selector，分配全局 `schedule-{uuid}` id，写 active 任务；
//! - `list`：按 session 过滤 active 记录；
//! - `catalog`：全部 active/inactive 任务（含 session 绑定与 status）；
//! - `delete`：删 Host 行（非变更 not-found 也返回）；
//! - `update`：compare-and-update 写 Host 行，id 稳定；
//! - `history`：递送历史分页（不激活 Session）。

use std::sync::Arc;

use super::domain;
use super::history;
use super::storage::HostScheduleStore;
use super::time;
use super::types::{
    empty_delivery_history, DeliveryCursorNotFound, DeliveryHistoryPage, DeliveryRetentionBounds,
    InactivePolicy, ScheduleCreateRequest, ScheduleDeleteResult, ScheduleError, ScheduleId,
    ScheduleRecord, ScheduleTask, ScheduleTimingChange, ScheduleUpdateContent,
    ScheduleUpdateResult, ScheduleView, TaskStatus,
};

/// 会话句柄接线槽：service 经 `Arc<HostScheduleStore>` 读 Host 存储。
#[derive(Clone)]
pub struct ScheduleService {
    store: Arc<HostScheduleStore>,
    retention: DeliveryRetentionBounds,
}

impl ScheduleService {
    /// 以 Host 存储构造 service（打开或创建 tasks 表）。
    pub fn new(store: Arc<HostScheduleStore>) -> Self {
        Self { store, retention: DeliveryRetentionBounds::default() }
    }

    /// 设置递送历史保留边界（窗口天数 / 记录数上限）。
    pub fn with_retention(mut self, bounds: DeliveryRetentionBounds) -> Self {
        self.retention = bounds;
        self
    }

    /// 当前保留边界。
    pub fn retention(&self) -> DeliveryRetentionBounds {
        self.retention
    }

    /// 按 session 列出 active 记录（存储序）。
    pub fn list(&self, session_id: &str) -> Vec<ScheduleRecord> {
        self.store
            .all()
            .into_iter()
            .filter(|task| task.session_id == session_id && task.status == TaskStatus::Active)
            .map(|task| task.record)
            .collect()
    }

    /// 按 session 列出 active 记录的模型视图。
    pub fn list_views(&self, session_id: &str) -> Vec<ScheduleView> {
        let now = crate::session::epoch_millis() as i64;
        self.list(session_id).iter().map(|r| domain::schedule_view(r, now)).collect()
    }

    /// 全部 active/inactive 任务（含 session 绑定 + status）。
    pub fn catalog(&self) -> Vec<ScheduleTask> {
        self.store.all()
    }

    /// 按 id 查找任务（任意 status）。
    pub fn task(&self, id: &str) -> Option<ScheduleTask> {
        self.store.get(id)
    }

    /// 按 id 查找 active 记录（工具 update 读取当前值用）。
    pub fn find(&self, id: &str) -> Option<ScheduleRecord> {
        self.store.get(id).map(|task| task.record)
    }

    /// 创建提醒：校验恰好一个 selector，分配全局 id，写 active 任务。
    pub fn create(
        &self,
        session_id: &str,
        request: ScheduleCreateRequest,
    ) -> Result<ScheduleRecord, ScheduleError> {
        let selectors = [
            request.after_seconds.is_some(),
            request.at.is_some(),
            request.every_seconds.is_some(),
            request.daily.is_some(),
            request.weekly.is_some(),
            request.cron.is_some(),
        ]
        .into_iter()
        .filter(|b| *b)
        .count();
        if selectors != 1 {
            return Err(ScheduleError::InvalidSelector(
                "schedule_create accepts exactly one of after_seconds, at, every_seconds, daily, weekly, or cron."
                    .to_string(),
            ));
        }
        let now = crate::session::epoch_millis() as i64;
        let id = domain::new_schedule_id();
        let record = self.build_record(&id, &request, now)?;
        let task = ScheduleTask {
            session_id: session_id.to_string(),
            record: record.clone(),
            status: TaskStatus::Active,
            last_delivery: None,
            delivery_history: Some(empty_delivery_history()),
            session_mode: None,
        };
        self.store.put(task).map_err(|_| ScheduleError::Internal)?;
        Ok(record)
    }

    /// 依据单个 selector 构建 record。
    fn build_record(
        &self,
        id: &ScheduleId,
        request: &ScheduleCreateRequest,
        now: i64,
    ) -> Result<ScheduleRecord, ScheduleError> {
        if let Some(at) = &request.at {
            return Ok(ScheduleRecord::At(domain::create_at_schedule_record(
                id.clone(), &request.prompt, at, now, &request.title,
            )?));
        }
        if let Some(after) = request.after_seconds {
            return Ok(ScheduleRecord::After(domain::create_after_schedule_record(
                id.clone(), &request.prompt, after, now, &request.title,
            )?));
        }
        if let Some(every) = request.every_seconds {
            return Ok(ScheduleRecord::Every(domain::create_every_schedule_record(
                id.clone(), &request.prompt, every, now, &request.title,
            )?));
        }
        if let Some(daily) = &request.daily {
            return Ok(ScheduleRecord::Daily(domain::create_daily_schedule_record(
                id.clone(), &request.prompt, daily, now, &request.title,
            )?));
        }
        if let Some(weekly) = &request.weekly {
            return Ok(ScheduleRecord::Weekly(domain::create_weekly_schedule_record(
                id.clone(), &request.prompt, weekly, now, &request.title,
            )?));
        }
        if let Some(cron) = &request.cron {
            return Ok(ScheduleRecord::Cron(domain::create_cron_schedule_record(
                id.clone(), &request.prompt, cron, now, &request.title,
            )?));
        }
        Err(ScheduleError::InvalidSelector(
            "Exactly one reminder selector is required.".to_string(),
        ))
    }

    /// 删除提醒：删 Host 行（非变更 not-found 也返回）。
    pub fn delete(&self, session_id: &str, id: &str) -> ScheduleDeleteResult {
        let current = self.store.get(id);
        let owned = current.as_ref().is_some_and(|task| task.session_id == session_id);
        if !owned {
            return ScheduleDeleteResult::NotFound { id: id.to_string() };
        }
        let _ = self.store.delete(id);
        ScheduleDeleteResult::Deleted { id: id.to_string() }
    }

    /// 更新提醒（compare-and-update）：
    /// - 任务不存在 / 不属于该 session → NotFound；
    /// - status inactive 且策略为 `Reject` → Ended（`Allow` 时照常更新，状态不变）；
    /// - `expected` 与当前活跃记录不一致 → Conflict；
    /// - 时序等价 → 保持原记录（Updated 但未变）；
    /// - 否则以 `now` 重算目标，id 稳定原地替换，写 Host 行。
    pub fn update(
        &self,
        session_id: &str,
        id: &str,
        expected: &ScheduleRecord,
        change: Option<ScheduleTimingChange>,
        content: ScheduleUpdateContent,
        inactive: InactivePolicy,
    ) -> ScheduleUpdateResult {
        let current = match self.store.get(id) {
            Some(task) if task.session_id == session_id => task,
            _ => return ScheduleUpdateResult::NotFound { id: id.to_string() },
        };
        if current.status == TaskStatus::Inactive && inactive == InactivePolicy::Reject {
            return ScheduleUpdateResult::Ended { id: id.to_string() };
        }
        if &current.record != expected {
            return ScheduleUpdateResult::Conflict { id: id.to_string() };
        }
        let now = crate::session::epoch_millis() as i64;
        let record = match self.changed_record(&current.record, change, content, now) {
            Ok(r) => r,
            Err(e) => return ScheduleUpdateResult::Error(e),
        };
        let updated = ScheduleTask { record: record.clone(), ..current };
        let _ = self.store.put(updated);
        ScheduleUpdateResult::Updated { record }
    }

    /// 应用时序/内容变更到当前 record（id 稳定）。
    fn changed_record(
        &self,
        current: &ScheduleRecord,
        change: Option<ScheduleTimingChange>,
        content: ScheduleUpdateContent,
        now: i64,
    ) -> Result<ScheduleRecord, ScheduleError> {
        let title = match content.title {
            Some(t) => domain::schedule_title(&t)?,
            None => current.title().to_string(),
        };
        let prompt = match content.prompt {
            Some(p) => domain::schedule_prompt(p.trim())?,
            None => current.prompt().to_string(),
        };
        let Some(change) = change else {
            if title == current.title() && prompt == current.prompt() {
                return Ok(current.clone());
            }
            return replace_content(current, &title, &prompt);
        };
        if timing_equivalent(current, &change)? {
            if title == current.title() && prompt == current.prompt() {
                return Ok(current.clone());
            }
            return replace_content(current, &title, &prompt);
        }
        let id = current.id().to_string();
        let req = ScheduleCreateRequest {
            title,
            prompt,
            after_seconds: None,
            at: None,
            every_seconds: None,
            daily: None,
            weekly: None,
            cron: None,
        };
        Ok(match change {
            ScheduleTimingChange::At { at } => {
                ScheduleRecord::At(domain::create_at_schedule_record(id, &req.prompt, &at, now, &req.title)?)
            }
            ScheduleTimingChange::Every { every_seconds } => ScheduleRecord::Every(
                domain::create_every_schedule_record(id, &req.prompt, every_seconds, now, &req.title)?,
            ),
            ScheduleTimingChange::Daily { daily } => {
                ScheduleRecord::Daily(domain::create_daily_schedule_record(id, &req.prompt, &daily, now, &req.title)?)
            }
            ScheduleTimingChange::Weekly { weekly } => ScheduleRecord::Weekly(
                domain::create_weekly_schedule_record(id, &req.prompt, &weekly, now, &req.title)?,
            ),
            ScheduleTimingChange::Cron { cron } => {
                ScheduleRecord::Cron(domain::create_cron_schedule_record(id, &req.prompt, &cron, now, &req.title)?)
            }
        })
    }

    /// 递送历史分页（不激活 Session）。
    ///
    /// `limit` 须为 1..=100；任务不存在或绑定不符 → not-found；
    /// 游标不存在 → cursor-not-found。
    pub fn history(
        &self,
        session_id: &str,
        id: &str,
        limit: usize,
        before: Option<&str>,
    ) -> Result<DeliveryHistoryPage, DeliveryHistoryError> {
        if !(1..=100).contains(&limit) {
            return Err(DeliveryHistoryError::InvalidLimit);
        }
        let task = match self.store.get(id) {
            Some(task) if task.session_id == session_id => task,
            _ => return Err(DeliveryHistoryError::NotFound),
        };
        history::delivery_history_page(&task, id, limit, before, self.retention)
            .map_err(DeliveryHistoryError::CursorNotFound)
    }
}

/// 仅替换 title/prompt，保持规则与目标（id 稳定）。
fn replace_content(
    record: &ScheduleRecord,
    title: &str,
    prompt: &str,
) -> Result<ScheduleRecord, ScheduleError> {
    let mut r = record.clone();
    match &mut r {
        ScheduleRecord::After(x) => { x.title = title.to_string(); x.prompt = prompt.to_string(); }
        ScheduleRecord::At(x) => { x.title = title.to_string(); x.prompt = prompt.to_string(); }
        ScheduleRecord::Every(x) => { x.title = title.to_string(); x.prompt = prompt.to_string(); }
        ScheduleRecord::Daily(x) => { x.title = title.to_string(); x.prompt = prompt.to_string(); }
        ScheduleRecord::Weekly(x) => { x.title = title.to_string(); x.prompt = prompt.to_string(); }
        ScheduleRecord::Cron(x) => { x.title = title.to_string(); x.prompt = prompt.to_string(); }
    }
    Ok(r)
}

/// 判断时序变更与当前规则是否等价（等价则保持已提交目标）。
fn timing_equivalent(
    current: &ScheduleRecord,
    change: &ScheduleTimingChange,
) -> Result<bool, ScheduleError> {
    match (current, change) {
        (ScheduleRecord::At(_) | ScheduleRecord::After(_), ScheduleTimingChange::At { at }) => {
            let current_target = time::parse_utc_instant(current.scheduled_at());
            let new_target = domain::parse_at_input(at)?;
            Ok(Some(new_target) == current_target)
        }
        (ScheduleRecord::Every(r), ScheduleTimingChange::Every { every_seconds }) => {
            Ok(r.every_seconds == *every_seconds)
        }
        (ScheduleRecord::Daily(r), ScheduleTimingChange::Daily { daily }) => {
            let (time, tz) = domain::parse_daily_input(daily)?;
            Ok(time == r.time && tz == r.time_zone)
        }
        (ScheduleRecord::Weekly(r), ScheduleTimingChange::Weekly { weekly }) => {
            let (time, tz, weekdays) = domain::parse_weekly_input(weekly)?;
            Ok(time == r.time && tz == r.time_zone && weekdays == r.weekdays)
        }
        (ScheduleRecord::Cron(r), ScheduleTimingChange::Cron { cron }) => {
            let (expr, tz) = domain::parse_cron_input(cron)?;
            Ok(expr == r.expression && tz == r.time_zone)
        }
        _ => Ok(false),
    }
}

/// 递送历史查询错误（查询失败分支）。
#[derive(Clone, Debug, PartialEq)]
pub enum DeliveryHistoryError {
    /// limit 非 1..=100 的整数。
    InvalidLimit,
    /// 任务不存在或 session 绑定不符。
    NotFound,
    /// 游标不存在。
    CursorNotFound(DeliveryCursorNotFound),
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
