//! Errors raised by the todos use cases.

use storage::StorageError;
use thiserror::Error;

/// Errors surfaced by [`TodosInteractor`](crate::use_cases::todos_interactor::TodosInteractor).
#[derive(Debug, Error)]
pub enum TodoError {
    /// The task failed validation (see
    /// [`NewTodoModel::has_valid_task`](crate::models::todo::NewTodoModel::has_valid_task)).
    #[error("task must not be empty")]
    EmptyTask,

    /// No todo exists with the requested id.
    #[error("todo not found")]
    NotFound,

    /// The repository could not read or write todos.
    #[error(transparent)]
    Storage(#[from] StorageError),
}
