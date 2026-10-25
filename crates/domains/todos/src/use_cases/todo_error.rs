use storage::StorageError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TodoError {
    #[error("task must not be empty")]
    EmptyTask,

    #[error("todo not found")]
    NotFound,

    #[error(transparent)]
    Storage(#[from] StorageError),
}
