use sqlx::{self, PgPool};
use tracing::instrument;

use crate::{common::errors::DbError, models::todo::TodoModel};

#[derive(Clone, Debug)]
pub struct TodosRepository {
    service: PgPool,
}

impl TodosRepository {
    pub fn new(service: PgPool) -> Self {
        Self { service }
    }

    #[instrument(skip_all)]
    pub async fn add_todo(&self, todo: &TodoModel) -> Result<TodoModel, DbError> {
        sqlx::query_as::<_, TodoModel>(
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
        .map_err(|e| DbError::new("inserting todo", e))
    }
}
