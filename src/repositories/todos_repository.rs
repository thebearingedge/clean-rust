use sqlx::{self, PgPool};
use tracing::instrument;

use crate::{
    models::todo::TodoModel,
    repositories::db_error::{DbContext, DbError},
};

#[derive(Clone, Debug)]
pub struct TodosRepository {
    service: PgPool,
}

impl TodosRepository {
    pub fn new(service: PgPool) -> Self {
        Self { service }
    }

    #[instrument(skip_all)]
    pub async fn add(&self, todo: &TodoModel) -> Result<TodoModel, DbError> {
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
        .db_context("inserting todo")
    }
}
