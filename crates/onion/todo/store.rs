//! todo 投影：durable `todo/write` 事件折叠。
//!
//! 核心语义：
//! - `todo/write` 整表 last-write-wins（最近一条生效）；
//! - `turn/start` 清空（新回合开始前不保留上一回合清单；`turn/end` 保留，
//!   让已完成清单在回合内可见）；
//! - 首次写入前为 None。
//!
//! onion 教学实现：从 durable 事件流折叠「当前 todo 清单」投影；仅被宿主
//! （CLI 等）用于展示，不进模型上下文（工具结果本身含整表，模型侧已可见）。

use std::sync::Arc;

use crate::session::SessionLog;

use super::types::TodoSnapshot;

/// todo 投影快照（None = 尚无 todo/write 写入或本回合尚未写入）。
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct TodoProjection {
    pub todos: Option<TodoSnapshot>,
}

/// 从 durable 事件流折叠 todo 投影（last-write-wins + turn/start 清空）。
pub fn fold_todo(events: &[crate::session::SessionEvent]) -> TodoProjection {
    let mut projection = TodoProjection::default();
    for event in events {
        match event {
            crate::session::SessionEvent::TodoWrite { todos } => {
                projection.todos = Some(TodoSnapshot { todos: todos.clone() });
            }
            crate::session::SessionEvent::TurnStart { .. } => {
                projection.todos = None;
            }
            _ => {}
        }
    }
    projection
}

/// 追加一条 `todo/write` durable 事件（整表替换）。
pub fn append_todo_write(session: &SessionLog, todos: Vec<super::types::TodoItem>) {
    session.append(crate::session::SessionEvent::TodoWrite { todos });
}

/// 接线辅助：构造时从 session 投影初始 todo 清单。
pub fn initial_projection(session: &Arc<SessionLog>) -> TodoProjection {
    fold_todo(&session.snapshot())
}
