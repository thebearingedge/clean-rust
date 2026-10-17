use std::sync::Arc;

use thiserror::Error;

use storage::storage_error::StorageError;

use crate::{
    models::{NewTodoModel, TodoModel},
    use_cases::repositories::TodosRepository,
};

#[derive(Debug, Error)]
pub enum TodoError {
    #[error("task must not be empty")]
    EmptyTask,

    #[error(transparent)]
    Storage(#[from] StorageError),
}

#[derive(Clone, Debug)]
pub struct TodosInteractor {
    repository: Arc<dyn TodosRepository>,
}

impl TodosInteractor {
    pub fn new(repository: Arc<dyn TodosRepository>) -> Self {
        Self { repository }
    }

    /// # Errors
    ///
    /// Returns [`TodoError::EmptyTask`] if the task fails validation (see
    /// [`NewTodoModel::has_valid_task`]), and [`TodoError::Storage`] if the
    /// repository cannot persist the todo.
    pub async fn add(&self, todo: &NewTodoModel) -> Result<TodoModel, TodoError> {
        if !todo.has_valid_task() {
            return Err(TodoError::EmptyTask);
        }
        Ok(self.repository.add(todo).await?)
    }
}
