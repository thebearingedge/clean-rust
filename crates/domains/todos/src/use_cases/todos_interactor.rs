//! Primary entry point for the todos domain, driving a
//! [`TodosRepository`](crate::use_cases::repositories::TodosRepository)
//! implementation behind the [`TodosInteractor`], with idempotent creation
//! scoped by a [`TodosUnitOfWork`](crate::use_cases::unit_of_work::TodosUnitOfWork)
//! transaction.

use std::sync::Arc;

use futures::TryStreamExt;
use tracing::instrument;
use uuid::Uuid;

use utils::pagination::{Cursor, Page, Sort};

use crate::{
    models::{
        idempotency_key::IdempotencyKey,
        todo::{NewTodoModel, TodoCursor, TodoFilter, TodoModel, TodoSortKey, TodoUpdateModel},
    },
    use_cases::repositories::{Claim, TodosRepository},
    use_cases::todo_error::TodoError,
    use_cases::unit_of_work::TodosUnitOfWork,
};

const DEFAULT_PAGE_SIZE: u64 = 50;

/// The result of an idempotent todo creation: the todo, and whether it was
/// replayed from a previous request with the same key.
#[derive(Debug)]
pub struct TodoCreation {
    /// The todo created by this request, or replayed from the original one.
    pub todo: TodoModel,
    /// `true` when the todo was not created here but returned from the
    /// request the key was first used for.
    pub replayed: bool,
}

#[derive(Clone, Debug)]
/// Coordinates the todos use cases over the injected repository port.
pub struct TodosInteractor {
    repository: Arc<dyn TodosRepository>,
    unit_of_work: Arc<dyn TodosUnitOfWork>,
}

impl TodosInteractor {
    /// The largest page size [`TodosInteractor::list_by_filter`] returns:
    /// larger requested limits are clamped down to this.
    pub const MAX_PAGE_SIZE: u64 = 200;

    /// Creates an interactor backed by the given repository for single-statement
    /// use cases, and the given unit of work for multi-statement ones such as
    /// [`TodosInteractor::add_idempotent`].
    #[must_use]
    pub fn new(
        repository: Arc<dyn TodosRepository>,
        unit_of_work: Arc<dyn TodosUnitOfWork>,
    ) -> Self {
        Self {
            repository,
            unit_of_work,
        }
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

    /// Creates a todo under a client-supplied idempotency key. Retrying the
    /// same request with the same key returns the todo from the original
    /// request (with `replayed: true`) instead of creating a second one;
    /// a key already used for a different request body is rejected, and a
    /// key still held by an unfinished request is rejected until that
    /// request completes.
    ///
    /// A failed creation leaves no key row behind: the idempotency claim,
    /// the todo insert, and the key completion all run in one unit of work,
    /// and any early failure drops the unit without
    /// [`commit`](crate::use_cases::unit_of_work::TodosUnit::commit), which
    /// rolls everything back — so the key can be retried afterwards.
    ///
    /// # Errors
    ///
    /// Returns [`TodoError::EmptyTask`] if the task fails validation (see
    /// [`NewTodoModel::has_valid_task`]), [`TodoError::IdempotencyKeyReused`]
    /// if the key was already completed for a different request body,
    /// [`TodoError::IdempotencyKeyInFlight`] if the key is still held by an
    /// unfinished request, and [`TodoError::Storage`] if the repositories or
    /// the transaction cannot complete their work.
    #[instrument(skip_all)]
    pub async fn add_idempotent(
        &self,
        key: &IdempotencyKey,
        todo: &NewTodoModel,
    ) -> Result<TodoCreation, TodoError> {
        if !todo.has_valid_task() {
            return Err(TodoError::EmptyTask);
        }

        let unit = self.unit_of_work.begin().await?;

        match unit.idempotency().claim(key, &todo.fingerprint()).await? {
            Claim::Fresh => {
                let todo = unit.todos().add(todo).await?;
                unit.idempotency().record(key, todo.todo_id).await?;
                unit.commit().await?;
                Ok(TodoCreation {
                    todo,
                    replayed: false,
                })
            }
            Claim::Replay { entity_id } => {
                let todo = unit
                    .todos()
                    .get_by_id(entity_id)
                    .await?
                    .ok_or(TodoError::NotFound)?;
                unit.commit().await?;
                Ok(TodoCreation {
                    todo,
                    replayed: true,
                })
            }
            Claim::Mismatch => Err(TodoError::IdempotencyKeyReused),
            Claim::InFlight => Err(TodoError::IdempotencyKeyInFlight),
        }
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
            .clamp(1, Self::MAX_PAGE_SIZE);

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
