//! todo 领域类型。
//!
//! `TodoItem` 仅含 `content` 与 `status`（无 activeForm）；整表替换语义，
//! 状态枚举 `pending` / `in_progress` / `completed`。

use serde::{Deserialize, Serialize};

/// 单个待办状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

impl TodoStatus {
    /// 状态标签（模型可见枚举值）。
    pub fn label(&self) -> &'static str {
        match self {
            TodoStatus::Pending => "pending",
            TodoStatus::InProgress => "in_progress",
            TodoStatus::Completed => "completed",
        }
    }
}

/// 单个待办项（content + status）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoItem {
    pub content: String,
    pub status: TodoStatus,
}

impl TodoItem {
    pub fn is_completed(&self) -> bool {
        matches!(self.status, TodoStatus::Completed)
    }
}

/// 待办清单整表快照（`todo/write` 事件载荷）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoSnapshot {
    pub todos: Vec<TodoItem>,
}

/// 计数（pending / inProgress / completed）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TodoCounts {
    pub pending: usize,
    pub in_progress: usize,
    pub completed: usize,
}

impl TodoSnapshot {
    pub fn counts(&self) -> TodoCounts {
        let mut counts = TodoCounts { pending: 0, in_progress: 0, completed: 0 };
        for todo in &self.todos {
            match todo.status {
                TodoStatus::Pending => counts.pending += 1,
                TodoStatus::InProgress => counts.in_progress += 1,
                TodoStatus::Completed => counts.completed += 1,
            }
        }
        counts
    }
}
