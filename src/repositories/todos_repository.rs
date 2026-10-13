use sqlx;

use crate::{
    common::{database::Database, errors::DbError},
    models::todo::TodoModel,
};

#[derive(Clone)]
pub struct TodosRepository {
    database: Database,
}

impl TodosRepository {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub async fn add_todo(&self, todo: &TodoModel) -> Result<TodoModel, DbError> {
        let mut conn = self.database.acquire().await?;
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
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| DbError::new("inserting todo", e))
    }
}
