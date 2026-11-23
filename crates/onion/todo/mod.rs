//! todo 领域模块：类型 + durable 投影（`todo/write`）。
//!
//! - `types.rs`：`TodoItem` / `TodoStatus` / `TodoSnapshot` / `TodoCounts`；
//! - `store.rs`：`todo/write` 事件折叠投影（last-write-wins + turn/start 清空）；
//! - `tools.rs`：`todo_write` 工具（整表替换 + 校验 + durable 落盘）。

pub mod store;
pub mod tools;
pub mod types;

pub use store::{append_todo_write, fold_todo, TodoProjection};
pub use tools::TodoWriteTool;
pub use types::{TodoCounts, TodoItem, TodoSnapshot, TodoStatus};
