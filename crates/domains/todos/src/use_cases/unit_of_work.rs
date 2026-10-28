//! Unit-of-work ports for the todos use cases, implemented by
//! infrastructure adapters.

use async_trait::async_trait;

use storage::StorageError;

use database::IdempotencyRepository;

use crate::use_cases::repositories::TodosRepository;

/// Transaction-scoping port that begins a unit of work whose repositories
/// share one underlying transaction.
///
/// Dropping the returned unit without calling
/// [`TodosUnit::commit`](crate::use_cases::unit_of_work::TodosUnit::commit)
/// rolls everything back.
#[async_trait]
pub trait TodosUnitOfWork: std::fmt::Debug + Send + Sync {
    /// Begins a transaction and returns the unit whose repository handles
    /// share it.
    ///
    /// # Errors
    ///
    /// Returns a [`StorageError`] if the transaction cannot be started.
    async fn begin(&self) -> Result<Box<dyn TodosUnit>, StorageError>;
}

/// One begun transaction, accessed through cheap per-call repository
/// handles.
///
/// Every call to [`TodosUnit::todos`] or [`TodosUnit::idempotency`] returns
/// a fresh handle over the same underlying transaction, so writes through
/// one handle are visible to reads through any other handle on the same
/// unit. Dropping the unit without calling
/// [`TodosUnit::commit`](crate::use_cases::unit_of_work::TodosUnit::commit)
/// rolls everything back.
#[async_trait]
pub trait TodosUnit: Send {
    /// Returns a todos repository handle over this unit's transaction.
    fn todos(&self) -> Box<dyn TodosRepository>;

    /// Returns an idempotency-keys repository handle over this unit's
    /// transaction.
    fn idempotency(&self) -> Box<dyn IdempotencyRepository>;

    /// Commits the transaction; dropping the unit without calling `commit`
    /// rolls everything back.
    ///
    /// # Errors
    ///
    /// Returns a [`StorageError`] if the commit cannot be persisted.
    async fn commit(self: Box<Self>) -> Result<(), StorageError>;
}
