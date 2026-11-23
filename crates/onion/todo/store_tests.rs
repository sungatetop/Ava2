//! `todo/store` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::{fold_todo, TodoProjection};
use crate::session::SessionEvent;
use crate::todo::types::{TodoItem, TodoStatus};

fn item(content: &str, status: TodoStatus) -> TodoItem {
    TodoItem { content: content.to_string(), status }
}

#[test]
fn fold_todo_empty_is_none() {
    let projection = fold_todo(&[]);
    assert_eq!(projection, TodoProjection::default());
    assert_eq!(projection.todos, None);
}

#[test]
fn fold_todo_last_write_wins() {
    let events = vec![
        SessionEvent::TodoWrite { todos: vec![item("a", TodoStatus::Pending)] },
        SessionEvent::TodoWrite { todos: vec![item("b", TodoStatus::Completed)] },
    ];
    let projection = fold_todo(&events);
    let todos = projection.todos.unwrap().todos;
    assert_eq!(todos.len(), 1);
    assert_eq!(todos[0].content, "b");
    assert_eq!(todos[0].status, TodoStatus::Completed);
}

#[test]
fn fold_todo_turn_start_clears() {
    let events = vec![
        SessionEvent::TodoWrite { todos: vec![item("a", TodoStatus::Pending)] },
        SessionEvent::TurnStart { turn: 2 },
    ];
    let projection = fold_todo(&events);
    assert_eq!(projection.todos, None);
}

#[test]
fn fold_todo_write_after_turn_start_restores() {
    let events = vec![
        SessionEvent::TodoWrite { todos: vec![item("a", TodoStatus::Pending)] },
        SessionEvent::TurnStart { turn: 2 },
        SessionEvent::TodoWrite { todos: vec![item("c", TodoStatus::InProgress)] },
    ];
    let projection = fold_todo(&events);
    let todos = projection.todos.unwrap().todos;
    assert_eq!(todos.len(), 1);
    assert_eq!(todos[0].content, "c");
}
