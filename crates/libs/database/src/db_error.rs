use thiserror::Error;

use storage::storage_error::StorageError;

#[derive(Debug, Error)]
#[error("{context}: {source}")]
pub struct DbError {
    pub context: String,
    #[source]
    source: sqlx::Error,
}

impl DbError {
    pub fn new(context: impl Into<String>, source: sqlx::Error) -> Self {
        Self {
            context: context.into(),
            source,
        }
    }

    #[must_use]
    pub fn is_unavailable(&self) -> bool {
        matches!(
            self.source,
            sqlx::Error::PoolTimedOut | sqlx::Error::PoolClosed | sqlx::Error::Io(_)
        )
    }

    #[must_use]
    pub fn is_conflict(&self) -> bool {
        self.source
            .as_database_error()
            .is_some_and(|e| e.kind() == sqlx::error::ErrorKind::UniqueViolation)
    }
}

pub trait DbContext<T> {
    /// # Errors
    ///
    /// Wraps a [`sqlx::Error`] with additional context.
    fn db_context(self, context: impl Into<String>) -> Result<T, DbError>;
}

impl<T> DbContext<T> for Result<T, sqlx::Error> {
    fn db_context(self, context: impl Into<String>) -> Result<T, DbError> {
        self.map_err(|e| DbError::new(context, e))
    }
}

impl From<DbError> for StorageError {
    fn from(e: DbError) -> Self {
        if e.is_unavailable() {
            Self::Unavailable(Box::new(e))
        } else if e.is_conflict() {
            Self::Conflict(Box::new(e))
        } else {
            Self::Other(Box::new(e))
        }
    }
}
