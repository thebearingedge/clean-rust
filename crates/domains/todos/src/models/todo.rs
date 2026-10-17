use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug)]
pub struct TodoModel {
    pub todo_id: Uuid,
    pub task: String,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}
