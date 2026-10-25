//! Repository port for persisting and querying todos, implemented by
//! infrastructure adapters.

use std::pin::Pin;

use async_trait::async_trait;
use futures::{Stream, TryStreamExt};
use uuid::Uuid;

use storage::StorageError;
use utils::pagination::{Cursor, Sort, SortOrder};

use crate::models::todo::{
    NewTodoModel, TodoCursor, TodoFilter, TodoModel, TodoSortKey, TodoUpdateModel,
};

/// Paged result stream from [`TodosRepository::stream_by_filter`], yielding
/// matching todos in the requested sort order or a [`StorageError`] when
/// they cannot be read.
pub type TodoStream<'a> = Pin<Box<dyn Stream<Item = Result<TodoModel, StorageError>> + Send + 'a>>;

/// Persistence port for todos, backed by an infrastructure adapter.
///
/// Implementations own the mapping between these domain models and their
/// backing store and must uphold the contract documented on each method.
#[async_trait]
pub trait TodosRepository: std::fmt::Debug + Send + Sync {
    /// Persists a new todo and returns it with its [`TodoModel::todo_id`] and
    /// timestamps assigned.
    ///
    /// # Errors
    ///
    /// Returns a [`StorageError`] if the todo cannot be persisted.
    async fn add(&self, todo: &NewTodoModel) -> Result<TodoModel, StorageError>;

    /// Streams todos matching `filter` in `sort` order, starting just after
    /// the [`TodoCursor`] position and yielding at most `cursor.limit` items
    /// when a limit is set. An empty stream means nothing matches.
    ///
    /// The [`TodoCursor`] tuple must correspond to `sort.key` (see
    /// [`TodoCursor`]).
    ///
    /// # Errors
    ///
    /// Yields a [`StorageError`] if the todos cannot be read.
    fn stream_by_filter(
        &self,
        filter: &TodoFilter,
        sort: Sort<TodoSortKey>,
        cursor: &Cursor<TodoCursor>,
    ) -> TodoStream<'_>;

    /// Returns the todo with `todo_id`, or [`Option::None`] when no todo has
    /// that id.
    ///
    /// # Errors
    ///
    /// Returns a [`StorageError`] if the todo cannot be read.
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

        self.stream_by_filter(&filter, sort, &cursor)
            .try_next()
            .await
    }

    /// Replaces the mutable fields of the todo with `todo_id`. Returns the
    /// updated todo, or `None` when no todo has that id.
    ///
    /// # Errors
    ///
    /// Returns a [`StorageError`] if the update cannot be persisted.
    async fn update(
        &self,
        todo_id: Uuid,
        todo: &TodoUpdateModel,
    ) -> Result<Option<TodoModel>, StorageError>;

    /// Deletes the todo with `todo_id`, returning `true` when a row was
    /// deleted and `false` when no todo has that id.
    ///
    /// # Errors
    ///
    /// Returns a [`StorageError`] if the delete cannot be persisted.
    async fn delete(&self, todo_id: Uuid) -> Result<bool, StorageError>;

    /// Collects the result of [`stream_by_filter`](Self::stream_by_filter)
    /// into todos matching `filter` in `sort` order.
    ///
    /// # Errors
    ///
    /// Returns a [`StorageError`] if the todos cannot be read.
    async fn list_by_filter(
        &self,
        filter: &TodoFilter,
        sort: Sort<TodoSortKey>,
        cursor: &Cursor<TodoCursor>,
    ) -> Result<Vec<TodoModel>, StorageError> {
        self.stream_by_filter(filter, sort, cursor)
            .try_collect::<Vec<TodoModel>>()
            .await
    }
}
