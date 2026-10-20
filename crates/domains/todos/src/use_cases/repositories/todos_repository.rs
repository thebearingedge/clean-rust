use std::pin::Pin;

use async_trait::async_trait;
use futures::{Stream, TryStreamExt};
use uuid::Uuid;

use storage::storage_error::StorageError;
use utils::pagination::{Cursor, Sort, SortOrder};

use crate::models::todo::{NewTodoModel, TodoCursor, TodoFilter, TodoModel, TodoSortKey};

pub type TodoStream<'a> = Pin<Box<dyn Stream<Item = Result<TodoModel, StorageError>> + Send + 'a>>;

#[async_trait]
pub trait TodosRepository: std::fmt::Debug + Send + Sync {
    async fn add(&self, todo: &NewTodoModel) -> Result<TodoModel, StorageError>;

    fn stream_by_filter(
        &self,
        filter: &TodoFilter,
        sort: &Sort<TodoSortKey>,
        cursor: &Cursor<TodoCursor>,
    ) -> TodoStream<'_>;

    async fn get_by_id(&self, todo_id: Uuid) -> Result<Option<TodoModel>, StorageError> {
        let filter = TodoFilter {
            todo_id: Some(vec![todo_id]),
            ..TodoFilter::default()
        };
        let sort = Sort {
            key: TodoSortKey::default(),
            order: SortOrder::default(),
        };
        let cursor = Cursor::default();

        self.stream_by_filter(&filter, &sort, &cursor)
            .try_next()
            .await
    }

    async fn list_by_filter(
        &self,
        filter: &TodoFilter,
        sort: &Sort<TodoSortKey>,
        cursor: &Cursor<TodoCursor>,
    ) -> Result<Vec<TodoModel>, StorageError> {
        self.stream_by_filter(filter, sort, cursor)
            .try_collect::<Vec<TodoModel>>()
            .await
    }
}
