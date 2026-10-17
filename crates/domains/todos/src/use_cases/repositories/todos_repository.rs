use async_trait::async_trait;
use storage::storage_error::StorageError;

use crate::models::{NewTodoModel, TodoModel};

#[async_trait]
pub trait TodosRepository: std::fmt::Debug + Send + Sync {
    async fn add(&self, model: &NewTodoModel) -> Result<TodoModel, StorageError>;
}
