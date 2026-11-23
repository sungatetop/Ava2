//! schedule 域类型：durable record、时间选择器、Host 存储任务与错误。
//!
//! - 六种时序规则：`after` / `at`（一次性）与 `every` / `daily` / `weekly` / `cron`（循环）；
//! - 全局唯一 `ScheduleId`（`schedule-{uuid}`）；
//! - Host 独立存储任务 `ScheduleTask`（`tasks` 表行），
//!   绑定原 Session + record + 递送状态/历史，独立于 Session 事件流；
//! - 错误码稳定闭集。

use serde::{Deserialize, Serialize};

/// 全局唯一提醒身份（`schedule-{uuid}`，跨会话稳定）。
pub type ScheduleId = String;

/// 一次性延时提醒（durable）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AfterRecord {
    pub id: ScheduleId,
    pub title: String,
    pub prompt: String,
    pub after_seconds: i64,
    pub scheduled_at: String,
}

/// 一次性绝对时刻提醒（durable）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AtRecord {
    pub id: ScheduleId,
    pub title: String,
    pub prompt: String,
    pub scheduled_at: String,
}

/// 固定间隔循环提醒（durable）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EveryRecord {
    pub id: ScheduleId,
    pub title: String,
    pub prompt: String,
    pub every_seconds: i64,
    pub scheduled_at: String,
}

/// 每日墙钟提醒（durable，IANA 时区）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DailyRecord {
    pub id: ScheduleId,
    pub title: String,
    pub prompt: String,
    pub time: String,
    pub time_zone: String,
    pub scheduled_at: String,
}

/// 每周墙钟提醒（durable，IANA 时区 + ISO 工作日）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WeeklyRecord {
    pub id: ScheduleId,
    pub title: String,
    pub prompt: String,
    pub time: String,
    pub time_zone: String,
    pub weekdays: Vec<u32>,
    pub scheduled_at: String,
}

/// 五段 cron 墙钟提醒（durable，IANA 时区）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CronRecord {
    pub id: ScheduleId,
    pub title: String,
    pub prompt: String,
    pub expression: String,
    pub time_zone: String,
    pub scheduled_at: String,
}

/// 提醒规则（durable record，六种变体）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ScheduleRecord {
    After(AfterRecord),
    At(AtRecord),
    Every(EveryRecord),
    Daily(DailyRecord),
    Weekly(WeeklyRecord),
    Cron(CronRecord),
}

impl ScheduleRecord {
    pub fn id(&self) -> &str {
        match self {
            ScheduleRecord::After(r) => &r.id,
            ScheduleRecord::At(r) => &r.id,
            ScheduleRecord::Every(r) => &r.id,
            ScheduleRecord::Daily(r) => &r.id,
            ScheduleRecord::Weekly(r) => &r.id,
            ScheduleRecord::Cron(r) => &r.id,
        }
    }

    pub fn title(&self) -> &str {
        match self {
            ScheduleRecord::After(r) => &r.title,
            ScheduleRecord::At(r) => &r.title,
            ScheduleRecord::Every(r) => &r.title,
            ScheduleRecord::Daily(r) => &r.title,
            ScheduleRecord::Weekly(r) => &r.title,
            ScheduleRecord::Cron(r) => &r.title,
        }
    }

    pub fn prompt(&self) -> &str {
        match self {
            ScheduleRecord::After(r) => &r.prompt,
            ScheduleRecord::At(r) => &r.prompt,
            ScheduleRecord::Every(r) => &r.prompt,
            ScheduleRecord::Daily(r) => &r.prompt,
            ScheduleRecord::Weekly(r) => &r.prompt,
            ScheduleRecord::Cron(r) => &r.prompt,
        }
    }

    pub fn scheduled_at(&self) -> &str {
        match self {
            ScheduleRecord::After(r) => &r.scheduled_at,
            ScheduleRecord::At(r) => &r.scheduled_at,
            ScheduleRecord::Every(r) => &r.scheduled_at,
            ScheduleRecord::Daily(r) => &r.scheduled_at,
            ScheduleRecord::Weekly(r) => &r.scheduled_at,
            ScheduleRecord::Cron(r) => &r.scheduled_at,
        }
    }
}

/// 当前交付时机状态（面向模型视图）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleState {
    Scheduled,
    Overdue,
}

/// 完整面向模型的提醒视图（record + 时机状态 + 交付模式）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScheduleView {
    #[serde(flatten)]
    pub record: ScheduleRecord,
    pub state: ScheduleState,
    pub delivery_mode: String,
}

/// 模型输入：`at` 绝对目标（RFC3339 字符串或结构化本地日历）。
#[derive(Clone, Debug, PartialEq)]
pub enum AtInput {
    /// 显式偏移 RFC3339 字符串（`YYYY-MM-DDTHH:mm:ss[.fff](Z|±HH:MM)`）。
    Instant(String),
    /// 结构化本地日历（date + time + time_zone）。
    Local { date: String, time: String, time_zone: String },
}

/// 模型输入：`daily` 选择器。
#[derive(Clone, Debug, PartialEq)]
pub struct DailyInput {
    pub time: String,
    pub time_zone: String,
}

/// 模型输入：`weekly` 选择器。
#[derive(Clone, Debug, PartialEq)]
pub struct WeeklyInput {
    pub time: String,
    pub time_zone: String,
    pub weekdays: Vec<u32>,
}

/// 模型输入：`cron` 选择器。
#[derive(Clone, Debug, PartialEq)]
pub struct CronInput {
    pub expression: String,
    pub time_zone: String,
}

/// 创建请求（模型侧）。
#[derive(Clone, Debug, PartialEq)]
pub struct ScheduleCreateRequest {
    pub prompt: String,
    pub title: String,
    pub after_seconds: Option<i64>,
    pub at: Option<AtInput>,
    pub every_seconds: Option<i64>,
    pub daily: Option<DailyInput>,
    pub weekly: Option<WeeklyInput>,
    pub cron: Option<CronInput>,
}

/// 时序变更（update 携带；kind 可与原 record 不同）。
#[derive(Clone, Debug, PartialEq)]
pub enum ScheduleTimingChange {
    At { at: AtInput },
    Every { every_seconds: i64 },
    Daily { daily: DailyInput },
    Weekly { weekly: WeeklyInput },
    Cron { cron: CronInput },
}

/// 更新时对已停用任务的处置策略。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InactivePolicy {
    /// 停用视为已结束，拒绝更新（模型侧工具沿用，避免「改了就复活」）。
    Reject,
    /// 允许更新停用任务：只改内容/时序，停用状态保持不变（产品侧编辑沿用）。
    Allow,
}

/// 更新内容（title/prompt 可选替换）。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScheduleUpdateContent {
    pub title: Option<String>,
    pub prompt: Option<String>,
}

/// 稳定输入错误码。
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum ScheduleError {
    InvalidPrompt(String),
    InvalidSelector(String),
    InvalidRule(String),
    InvalidTimeZone(String),
    NotFuture,
    TimeOutOfRange,
    FrequencyTooHigh,
    SubagentSession,
    Internal,
}

impl ScheduleError {
    pub fn code(&self) -> &'static str {
        match self {
            ScheduleError::InvalidPrompt(_) => "invalid_prompt",
            ScheduleError::InvalidSelector(_) => "invalid_selector",
            ScheduleError::InvalidRule(_) => "invalid_rule",
            ScheduleError::InvalidTimeZone(_) => "invalid_time_zone",
            ScheduleError::NotFuture => "not_future",
            ScheduleError::TimeOutOfRange => "time_out_of_range",
            ScheduleError::FrequencyTooHigh => "frequency_too_high",
            ScheduleError::SubagentSession => "subagent_session",
            ScheduleError::Internal => "internal_error",
        }
    }

    pub fn message(&self) -> &str {
        match self {
            ScheduleError::InvalidPrompt(m) => m,
            ScheduleError::InvalidSelector(m) => m,
            ScheduleError::InvalidRule(m) => m,
            ScheduleError::InvalidTimeZone(m) => m,
            ScheduleError::NotFuture => "The scheduled time must be strictly in the future.",
            ScheduleError::TimeOutOfRange => {
                "The scheduled time must be representable as a four-digit-year RFC 3339 UTC instant."
            }
            ScheduleError::FrequencyTooHigh => "every_seconds must be at least 60.",
            ScheduleError::SubagentSession => "A delegated subagent cannot use reminders.",
            ScheduleError::Internal => "The schedule operation failed.",
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({ "code": self.code(), "message": self.message() })
    }
}

/// 删除结果（含非变更 not-found）。
#[derive(Clone, Debug, PartialEq)]
pub enum ScheduleDeleteResult {
    Deleted { id: ScheduleId },
    NotFound { id: ScheduleId },
}

/// 更新结果。
#[derive(Clone, Debug, PartialEq)]
pub enum ScheduleUpdateResult {
    Updated { record: ScheduleRecord },
    NotFound { id: ScheduleId },
    Ended { id: ScheduleId },
    Conflict { id: ScheduleId },
    Error(ScheduleError),
}

/// 循环决策：最新到期 occurrence + 下一未来目标。
#[derive(Clone, Debug, PartialEq)]
pub struct RecurringOccurrence {
    pub occurrence_at: String,
    pub next_scheduled_at: Option<String>,
}

/// durable Host 递送收据。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeliveryReceipt {
    /// 被递送 occurrence 的规范 UTC 目标。
    pub scheduled_at: String,
    /// Session 持久化确认递送后采样的规范 UTC 时刻。
    pub delivered_at: String,
    /// 递送消息身份（循环批次内多任务共享）。
    pub message_id: String,
}

/// 保存的 inbox 递送（含本 Host 记录时写入的不可变 prompt）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeliveryRecord {
    #[serde(flatten)]
    pub receipt: DeliveryReceipt,
    /// 本次 occurrence 发送的 prompt（legacy 收据不可用）。
    pub prompt: Option<String>,
}

/// 每任务的递送历史。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeliveryHistory {
    pub records: Vec<DeliveryRecord>,
    pub earlier_records_unavailable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub earlier_records_pruned: Option<bool>,
}

/// Host 独立存储任务：绑定原 Session，
/// 独立于 Session 事件流，任务比 Session 活得久。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScheduleTask {
    /// 接收本提醒的原 Session。
    pub session_id: String,
    pub record: ScheduleRecord,
    #[serde(default = "default_active")]
    pub status: TaskStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_delivery: Option<DeliveryReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivery_history: Option<DeliveryHistory>,
    /// 任务模式（onion-server 全局任务面扩展；对齐 ava-app `session_mode`）：
    /// `single` = 绑定会话投递（session_id 为目标）；`independent` = 到期由
    /// 调度器新建会话投递（session_id 递送前为空，触发后被回填）。缺省 None
    /// = 会话内 `schedule_create` 工具创建的传统提醒（等价 single）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_mode: Option<String>,
}

/// 任务递送状态（缺省 active）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Active,
    Inactive,
}

fn default_active() -> TaskStatus {
    TaskStatus::Active
}

/// 新任务缺省的递送历史（空记录 + 无早期记录标志）。
pub fn empty_delivery_history() -> DeliveryHistory {
    DeliveryHistory { records: Vec::new(), earlier_records_unavailable: false, earlier_records_pruned: None }
}

/// 递送历史保留边界。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeliveryRetentionBounds {
    /// 保留窗口（天）。
    pub days: i64,
    /// 每任务保留记录数上限。
    pub records: usize,
}

impl Default for DeliveryRetentionBounds {
    fn default() -> Self {
        Self { days: DEFAULT_DELIVERY_HISTORY_DAYS, records: DEFAULT_DELIVERY_HISTORY_RECORDS }
    }
}

/// 递送历史窗口缺省（30 天）。
pub const DEFAULT_DELIVERY_HISTORY_DAYS: i64 = 30;
/// 递送历史记录数缺省（200 条）。
pub const DEFAULT_DELIVERY_HISTORY_RECORDS: usize = 200;

/// 最新优先的递送历史页（查询成功分支）。
#[derive(Clone, Debug, PartialEq)]
pub struct DeliveryHistoryPage {
    pub id: ScheduleId,
    pub records: Vec<DeliveryRecord>,
    pub earlier_records_unavailable: bool,
    pub earlier_records_pruned: bool,
    pub retention: DeliveryRetentionBounds,
    pub next_before: Option<String>,
}

/// 递送历史查询失败（游标未找到）。
#[derive(Clone, Debug, PartialEq)]
pub struct DeliveryCursorNotFound {
    pub id: ScheduleId,
}

// 固定协议常量。
pub const MIN_EVERY_INTERVAL_SECONDS: i64 = 60;
pub const MAX_TITLE_LENGTH: usize = 120;
pub const CRON_SEARCH_HORIZON_YEARS: i64 = 400;
