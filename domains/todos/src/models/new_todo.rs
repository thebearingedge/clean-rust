use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct NewTodoModel {
    pub task: String,
    pub completed_at: Option<DateTime<Utc>>,
}

impl NewTodoModel {
    pub fn has_valid_task(&self) -> bool {
        self.task.trim().len() > 1
    }
}
