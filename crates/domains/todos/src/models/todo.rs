use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Input for creating a new todo.
#[derive(Debug, Clone)]
pub struct NewTodoModel {
    /// Task description; valid when non-empty after trimming (see
    /// [`Self::has_valid_task`]).
    pub task: String,
    /// When the todo was completed, or [`Option::None`] while it is open.
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
    /// Replacement task description; valid when non-empty after trimming
    /// (see [`Self::has_valid_task`]).
    pub task: String,
    /// Desired completion state: `true` stamps the todo's completion time,
    /// `false` clears it.
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
    /// Unique identifier of the todo.
    pub todo_id: Uuid,
    /// Task description.
    pub task: String,
    /// When the todo was completed, or [`Option::None`] while it is open.
    pub completed_at: Option<DateTime<Utc>>,
    /// When the todo was last created or updated.
    pub updated_at: DateTime<Utc>,
}

/// Filter for listing todos. All fields are optional conjunctive filters;
/// `todo_id` is "any-of" semantics — when present, only todos whose id is in
/// the list are returned (an empty list matches nothing).
#[derive(Debug, Default)]
pub struct TodoFilter {
    /// Restricts results to todos whose id is in the list; [`Option::None`]
    /// matches any id.
    pub todo_id: Option<Vec<Uuid>>,
    /// Restricts results to todos in this completion state; [`Option::None`]
    /// matches both open and completed todos.
    pub is_completed: Option<bool>,
}

/// Sort key for listing todos.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TodoSortKey {
    /// Order by todo id — the order in which todos were created.
    #[default]
    TodoId,
    /// Order by the time each todo was last updated.
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
    /// Id of the last todo on the previous page.
    pub todo_id: Uuid,
    /// Update time of the last todo on the previous page.
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
