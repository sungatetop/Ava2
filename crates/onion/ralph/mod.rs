//! Ralph 固定循环工具（教学子集）。
//!
//! - 单一不可变目标（`objective`）；
//! - 每轮开启一个全新 structured child（无父会话、无前轮 child 会话）；
//! - 跨轮只传有界结构化报告（文本约定）；
//! - 轮数上限（`maxRounds`，缺省部署上限）。
//!
//! 返回 `{ runId, agentsStarted, result }`，result.status ∈
//! `{ complete, blocked, budget-limited, round-failed }`。

pub mod tools;

pub use tools::RalphTool;
