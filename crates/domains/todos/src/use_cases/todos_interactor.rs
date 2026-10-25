//! Primary entry point for the todos domain, driving a
//! [`TodosRepository`](crate::use_cases::repositories::TodosRepository)
//! implementation behind the [`TodosInteractor`].

use std::sync::Arc;

use futures::TryStreamExt;
use tracing::instrument;
use uuid::Uuid;

use utils::pagination::{Cursor, Page, Sort};

use crate::{
    models::todo::{NewTodoModel, TodoCursor, TodoFilter, TodoModel, TodoSortKey, TodoUpdateModel},
    use_cases::repositories::TodosRepository,
    use_cases::todo_error::TodoError,
};

const DEFAULT_PAGE_SIZE: u64 = 50;
const MAX_PAGE_SIZE: u64 = 200;

#[derive(Clone, Debug)]
/// Coordinates the todos use cases over the injected repository port.
pub struct TodosInteractor {
    repository: Arc<dyn TodosRepository>,
}

impl TodosInteractor {
    /// Creates an interactor backed by the given repository.
    #[must_use]
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

    /// # Errors
    ///
    /// Returns [`TodoError::EmptyTask`] if the task fails validation (see
    /// [`TodoUpdateModel::has_valid_task`]), [`TodoError::NotFound`] if the
    /// todo does not exist, and [`TodoError::Storage`] if the repository
    /// cannot persist the update.
    #[instrument(skip_all)]
    pub async fn update(
        &self,
        todo_id: Uuid,
        todo: &TodoUpdateModel,
    ) -> Result<TodoModel, TodoError> {
        if !todo.has_valid_task() {
            return Err(TodoError::EmptyTask);
        }

        self.repository
            .update(todo_id, todo)
            .await?
            .ok_or(TodoError::NotFound)
    }

    /// # Errors
    ///
    /// Returns [`TodoError::NotFound`] if the todo does not exist, or
    /// [`TodoError::Storage`] if the repository cannot be read. Lookup is
    /// delegated entirely to the repository; malformed or non-existent IDs
    /// surface as [`TodoError::NotFound`].
    #[instrument(skip_all)]
    pub async fn get_by_id(&self, todo_id: Uuid) -> Result<TodoModel, TodoError> {
        self.repository
            .get_by_id(todo_id)
            .await?
            .ok_or(TodoError::NotFound)
    }

    /// # Errors
    ///
    /// Returns [`TodoError::NotFound`] if the todo does not exist, or
    /// [`TodoError::Storage`] if the repository cannot persist the delete.
    #[instrument(skip_all)]
    pub async fn remove(&self, todo_id: Uuid) -> Result<(), TodoError> {
        if self.repository.delete(todo_id).await? {
            Ok(())
        } else {
            Err(TodoError::NotFound)
        }
    }

    /// # Errors
    ///
    /// Returns [`TodoError::Storage`] if the repository cannot be read.
    #[instrument(skip_all)]
    pub async fn list_by_filter(
        &self,
        filter: &TodoFilter,
        sort: Sort<TodoSortKey>,
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
