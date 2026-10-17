use async_trait::async_trait;
use sqlx::{self, PgPool};
use tracing::instrument;

use database::db_error::DbContext;
use storage::storage_error::StorageError;

use crate::models::{NewTodoModel, TodoModel};
use crate::repositories::postgres::todo_row::TodoRow;
use crate::use_cases::repositories::TodosRepository as Interface;

#[derive(Clone, Debug)]
pub struct TodosRepository {
    service: PgPool,
}

impl TodosRepository {
    pub fn new(service: PgPool) -> Self {
        Self { service }
    }
}

#[async_trait]
impl Interface for TodosRepository {
    #[instrument(skip_all)]
    async fn add(&self, todo: &NewTodoModel) -> Result<TodoModel, StorageError> {
        sqlx::query_as::<_, TodoRow>(
            "
            insert into todos (
                task,
                completed_at
            )
            values ($1, $2)
            returning
                todo_id,
                task,
                completed_at,
                updated_at
            ",
        )
        .bind(&todo.task)
        .bind(todo.completed_at)
        .fetch_one(&self.service)
        .await
        .map(TodoModel::from)
        .db_context("inserting todo")
        .map_err(StorageError::from)
    }
}
