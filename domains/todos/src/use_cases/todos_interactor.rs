use thiserror::Error;
use tracing::instrument;

use database::db_error::DbError;

use crate::{models::todo::TodoModel, repositories::todos_repository::TodosRepository};

#[derive(Debug, Error)]
pub enum TodoError {
    #[error("task must not be empty")]
    EmptyTask,

    #[error("new todo must not have an id")]
    IdNotAllowed,

    #[error(transparent)]
    Db(#[from] DbError),
}

#[derive(Clone, Debug)]
pub struct TodosInteractor {
    repository: TodosRepository,
}

impl TodosInteractor {
    pub fn new(repository: TodosRepository) -> Self {
        Self { repository }
    }

    #[instrument(skip_all)]
    pub async fn add(&self, todo: &TodoModel) -> Result<TodoModel, TodoError> {
        if todo.has_valid_id() {
            return Err(TodoError::IdNotAllowed);
        }
        if !todo.has_valid_task() {
            return Err(TodoError::EmptyTask);
        }
        Ok(self.repository.add(todo).await?)
    }
}
