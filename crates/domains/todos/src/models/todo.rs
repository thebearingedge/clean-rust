use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct NewTodoModel {
    pub task: String,
    pub completed_at: Option<DateTime<Utc>>,
}

impl NewTodoModel {
    /// A task is valid when it contains at least one non-whitespace
    /// character after trimming.
    #[must_use]
    pub fn has_valid_task(&self) -> bool {
        !self.task.trim().is_empty()
    }
}

/// Full-replace update input for a todo.
#[derive(Debug, Clone)]
pub struct TodoUpdateModel {
    pub task: String,
    pub is_completed: bool,
}

impl TodoUpdateModel {
    /// A task is valid when it contains at least one non-whitespace
    /// character after trimming.
    #[must_use]
    pub fn has_valid_task(&self) -> bool {
        !self.task.trim().is_empty()
    }
}

/// A todo as stored by the repository.
#[derive(Debug)]
pub struct TodoModel {
    pub todo_id: Uuid,
    pub task: String,
    pub completed_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

/// Filter for listing todos. All fields are optional conjunctive filters;
/// `todo_id` is "any-of" semantics — when present, only todos whose id is in
/// the list are returned (an empty list matches nothing).
#[derive(Debug, Default)]
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

/// Opaque pagination cursor position for a todo.
///
/// The `updated_at`/`todo_id` pair is the composite key used when sorting by
/// `TodoSortKey::UpdatedAt`; when sorting by `TodoSortKey::TodoId` only
/// `todo_id` participates in the seek predicate. The tuple must always
/// correspond to the sort key chosen in `stream_by_filter`.
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
