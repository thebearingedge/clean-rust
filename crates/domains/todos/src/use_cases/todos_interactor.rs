use std::sync::Arc;

use futures::TryStreamExt;
use thiserror::Error;
use uuid::Uuid;

use storage::storage_error::StorageError;
use tracing::instrument;
use utils::pagination::{Cursor, Page, Sort};

use crate::{
    models::todo::{NewTodoModel, TodoCursor, TodoFilter, TodoModel, TodoSortKey},
    use_cases::repositories::TodosRepository,
};

#[derive(Debug, Error)]
pub enum TodoError {
    #[error("task must not be empty")]
    EmptyTask,

    #[error("todo not found")]
    NotFound,

    #[error(transparent)]
    Storage(#[from] StorageError),
}

const DEFAULT_PAGE_SIZE: u64 = 50;
const MAX_PAGE_SIZE: u64 = 200;

#[derive(Clone, Debug)]
pub struct TodosInteractor {
    repository: Arc<dyn TodosRepository>,
}

impl TodosInteractor {
    pub fn new(repository: Arc<dyn TodosRepository>) -> Self {
        Self { repository }
    }

    /// # Errors
    ///
    /// Returns [`TodoError::EmptyTask`] if the task fails validation (see
    /// [`NewTodoModel::has_valid_task`]), and [`TodoError::Storage`] if the
    /// repository cannot persist the todo.
    #[instrument(skip_all)]
    pub async fn add(&self, todo: &NewTodoModel) -> Result<TodoModel, TodoError> {
        if !todo.has_valid_task() {
            return Err(TodoError::EmptyTask);
        }

        Ok(self.repository.add(todo).await?)
    }

    #[instrument(skip_all)]
    pub async fn get_by_id(&self, todo_id: Uuid) -> Result<TodoModel, TodoError> {
        if todo_id.get_version_num() != 7 {
            return Err(TodoError::NotFound);
        }

        self.repository
            .get_by_id(todo_id)
            .await?
            .ok_or(TodoError::NotFound)
    }

    /// # Errors
    ///
    /// Returns [`TodoError::Storage`] if the repository cannot be read.
    #[instrument(skip_all)]
    pub async fn list_by_filter(
        &self,
        filter: &TodoFilter,
        sort: &Sort<TodoSortKey>,
        cursor: &Cursor<TodoCursor>,
    ) -> Result<Page<TodoModel, TodoCursor>, TodoError> {
        let limit = cursor
            .limit
            .unwrap_or(DEFAULT_PAGE_SIZE)
            .clamp(1, MAX_PAGE_SIZE);

        let window = Cursor {
            after: cursor.after,
            limit: Some(limit + 1),
        };

        let mut items: Vec<TodoModel> = self
            .repository
            .stream_by_filter(filter, sort, &window)
            .try_collect()
            .await?;

        let keep = usize::try_from(limit).unwrap_or(usize::MAX);

        let next = if items.len() > keep {
            items.truncate(keep);
            items.last().map(TodoCursor::from)
        } else {
            None
        };

        Ok(Page { items, next })
    }
}
