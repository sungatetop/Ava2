//! ask_user 领域模块：阻塞问询 seam。
//!
//! - `types.rs`：`AskUserQuestion` / `AskUserOption` / `AskUserAnswer` 等类型；
//! - `service.rs`：`AskUserService`（挂起-答复 oneshot 通道 + session 接线）；
//! - `tools.rs`：`ask_user_question` 工具（legacy blocking，答复回填 agent loop）。

pub mod service;
pub mod tools;
pub mod types;

pub use service::{answer_to_pending_message, AskUserRequest, AskUserService, TimedOutcome};
pub use tools::AskUserQuestionTool;
pub use types::{AskUserAnswer, AskUserAnswerItem, AskUserOption, AskUserQuestion};
