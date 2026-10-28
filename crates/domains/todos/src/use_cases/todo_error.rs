//! Errors raised by the todos use cases.

use thiserror::Error;

use storage::StorageError;

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

    /// The idempotency key was already used for a different request.
    #[error("idempotency key was already used for a different request")]
    IdempotencyKeyReused,

    /// The idempotency key is in use by a request that has not finished yet.
    #[error("idempotency key is in flight")]
    IdempotencyKeyInFlight,

    /// The repository could not read or write todos.
    #[error(transparent)]
    Storage(#[from] StorageError),
}
