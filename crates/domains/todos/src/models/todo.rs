use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct NewTodoModel {
    pub task: String,
    pub completed_at: Option<DateTime<Utc>>,
}

impl NewTodoModel {
    #[must_use]
    pub fn has_valid_task(&self) -> bool {
        self.task.trim().len() > 1
    }
}

#[derive(Debug)]
pub struct TodoModel {
    pub todo_id: Uuid,
    pub task: String,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Default)]
pub struct TodoFilter {
    pub todo_id: Option<Vec<Uuid>>,
    pub is_completed: Option<bool>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TodoSortKey {
    #[default]
    TodoId,
    UpdatedAt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TodoCursor {
    pub todo_id: Uuid,
    pub updated_at: DateTime<Utc>,
}

impl From<&TodoModel> for TodoCursor {
    fn from(todo: &TodoModel) -> Self {
        Self {
            todo_id: todo.todo_id,
            updated_at: todo.updated_at,
        }
    }
}
