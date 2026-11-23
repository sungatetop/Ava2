//! schedule 模块：durable 提醒。
//!
//! - `types.rs`：域类型（record / 选择器 / Host 任务 / 递送历史 / 错误闭集）；
//! - `time.rs`：时间/时区基元（RFC3339 + IANA + 墙钟运算）；
//! - `cron.rs`：五段 Vixie cron 解析 / 规范化 / 日期匹配；
//! - `domain.rs`：record 创建校验、全局 id、时序决策、视图；
//! - `storage.rs`：Host 独立 `tasks` 表 JSONL 落盘；
//! - `history.rs`：递送历史收据追加剪枝 + 游标分页；
//! - `framing.rs`：一次性 / 循环批次帧渲染；
//! - `service.rs`：Host 存储 CRUD（create/list/update/delete + catalog/history）；
//! - `runtime.rs`：Host 定时器 + 到期递送 + 状态 commit；
//! - `tools.rs`：`schedule_create` / `schedule_list` / `schedule_update` /
//!   `schedule_delete` 4 工具。
//!
//! 完整 schedule 能力：六种时序规则（after/at/every/daily/weekly/
//! cron）、IANA 时区 + Temporal 语义（DST 重叠取 earlier、间隙拒绝）、Vixie cron
//! 方言（DOM/DOW AND-OR）、recurring 决策（latest due occurrence + next target）、
//! Host 独立存储（任务比 Session 活得久）。

pub mod cron;
pub mod domain;
pub mod framing;
pub mod history;
pub mod runtime;
pub mod service;
pub mod storage;
pub mod time;
pub mod tools;
pub mod types;
