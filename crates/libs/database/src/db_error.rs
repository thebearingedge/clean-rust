//! Database-layer error type.
//!
//! [`DbError`] carries [`sqlx::Error`] context at the source: it wraps the
//! driver error together with caller-supplied context, plus the
//! [`DbError::is_unavailable`]/[`DbError::is_conflict`] predicates used to
//! reason about the failure. Classification into the transport-agnostic
//! [`StorageError`] taxonomy happens separately, via
//! `From<DbError> for StorageError` at the bottom of this file — this module
//! never constructs `StorageError` variants itself.

use storage::StorageError;
use thiserror::Error;

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
        // Deliberate over-classification: any `sqlx::Error::Io(_)` is treated
        // as availability loss (→ 503 downstream), even though an IO error can
        // in principle occur mid-query for non-availability reasons. The cost
        // of an occasional 503 where a 500 would fit is preferred over the cost
        // of classifying a genuinely dead connection as `Other`.
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
