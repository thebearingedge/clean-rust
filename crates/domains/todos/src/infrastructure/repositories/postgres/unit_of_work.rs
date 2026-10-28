use async_trait::async_trait;

use database::{Database, IdempotencyRepository, PgIdempotencyRepository, UnitOfWork};
use storage::StorageError;

use crate::infrastructure::repositories::postgres::PgTodosRepository;
use crate::use_cases::repositories::TodosRepository;
use crate::use_cases::unit_of_work::{TodosUnit, TodosUnitOfWork};

/// Postgres implementation of the
/// [`TodosUnitOfWork`](crate::use_cases::unit_of_work::TodosUnitOfWork) port.
#[derive(Clone, Debug)]
pub struct PgTodosUnitOfWork {
    database: Database,
}

impl PgTodosUnitOfWork {
    /// Creates a unit of work that begins transactions on the given
    /// [`Database`] connection source.
    #[must_use]
    pub fn new(database: Database) -> Self {
        Self { database }
    }
}

#[async_trait]
impl TodosUnitOfWork for PgTodosUnitOfWork {
    async fn begin(&self) -> Result<Box<dyn TodosUnit>, StorageError> {
        let unit = self.database.begin().await.map_err(StorageError::from)?;
        Ok(Box::new(PgTodosUnit { unit }))
    }
}

struct PgTodosUnit {
    unit: UnitOfWork,
}

#[async_trait]
impl TodosUnit for PgTodosUnit {
    fn todos(&self) -> Box<dyn TodosRepository> {
        Box::new(PgTodosRepository::new(self.unit.database()))
    }

    fn idempotency(&self) -> Box<dyn IdempotencyRepository> {
        Box::new(PgIdempotencyRepository::new(self.unit.database()))
    }

    async fn commit(self: Box<Self>) -> Result<(), StorageError> {
        self.unit.commit().await.map_err(StorageError::from)
    }
}
