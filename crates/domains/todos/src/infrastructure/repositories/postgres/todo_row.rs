use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::models::TodoModel;

#[derive(sqlx::FromRow)]
pub(super) struct TodoRow {
    pub todo_id: Uuid,
    pub task: String,
    pub completed_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

impl From<TodoRow> for TodoModel {
    fn from(row: TodoRow) -> Self {
        Self {
            todo_id: row.todo_id,
            task: row.task,
            completed_at: row.completed_at,
            updated_at: row.updated_at,
        }
    }
}
