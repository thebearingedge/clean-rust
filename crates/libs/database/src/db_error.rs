//! Database-layer error type.
//!
//! [`DbError`] carries [`sqlx::Error`] context at the source: it wraps the
//! driver error together with caller-supplied context, plus the
//! [`DbError::is_unavailable`]/[`DbError::is_conflict`] predicates used to
//! reason about the failure. Classification into the transport-agnostic
//! [`storage::StorageError`] taxonomy happens separately, via
//! `From<DbError> for StorageError` at the bottom of this file — this module
//! never constructs `StorageError` variants itself.

use storage::StorageError;
use thiserror::Error;

/// A database operation failure: the underlying [`sqlx::Error`] paired with
/// caller-supplied `context` describing the operation that failed.
///
/// Classify with [`DbError::is_unavailable`] and [`DbError::is_conflict`];
/// conversion into the transport-agnostic [`storage::StorageError`] taxonomy
/// happens via the `From` impl at the bottom of this file.
#[derive(Debug, Error)]
#[error("{context}: {source}")]
pub struct DbError {
    /// Description of the operation that failed; forms the `Display` prefix.
    pub context: String,
    #[source]
    source: sqlx::Error,
}

impl DbError {
    /// Wraps `source` with `context` describing the operation that failed.
    pub fn new(context: impl Into<String>, source: sqlx::Error) -> Self {
        Self {
            context: context.into(),
            source,
        }
    }

    /// Wraps a driver-protocol violation (a [`sqlx::Error::Protocol`]) with
    /// `context` describing the operation that failed and `detail`
    /// describing the violation.
    pub fn protocol(context: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::new(context, sqlx::Error::Protocol(detail.into()))
    }

    /// True when the failure means the database is unreachable or degraded
    /// (pool timed out or closed, IO failure).
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

    /// True when the failure is a unique-constraint violation.
    #[must_use]
    pub fn is_conflict(&self) -> bool {
        self.source
            .as_database_error()
            .is_some_and(|e| e.kind() == sqlx::error::ErrorKind::UniqueViolation)
    }
}

/// Extension trait for `Result<T, sqlx::Error>` that attaches caller
/// context to failures, producing [`DbError`].
pub trait DbContext<T> {
    /// Attaches `context` to a failed result; passes success through.
    ///
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
