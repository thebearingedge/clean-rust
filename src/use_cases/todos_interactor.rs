use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;
use tracing::instrument;

use crate::{
    common::errors::DbError, models::todo::TodoModel,
    repositories::todos_repository::TodosRepository,
};

#[derive(Debug, Error)]
pub enum TodoError {
    #[error("task must not be empty")]
    EmptyTask,

    #[error(transparent)]
    Db(#[from] DbError),
}

impl IntoResponse for TodoError {
    fn into_response(self) -> Response {
        match self {
            TodoError::EmptyTask => (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": TodoError::EmptyTask.to_string() })),
            )
                .into_response(),
            TodoError::Db(e) => e.into_response(),
        }
    }
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
        if !todo.has_valid_task() {
            return Err(TodoError::EmptyTask);
        }
        Ok(self.repository.add_todo(todo).await?)
    }
}
