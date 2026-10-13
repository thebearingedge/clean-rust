use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Default, FromRow)]
pub struct TodoModel {
    pub todo_id: Option<Uuid>,
    pub task: String,
    pub updated_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl TodoModel {
    pub fn has_valid_task(&self) -> bool {
        self.task.trim().len() > 1
    }
}
