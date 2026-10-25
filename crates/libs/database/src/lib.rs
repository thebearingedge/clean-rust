//! Database plumbing shared by infrastructure adapters: the [`DbError`]
//! error type with its classification predicates and the [`DbContext`]
//! extension trait that wraps `sqlx` results with operation context, plus a
//! readiness probe.

/// The database error type and its classification predicates.
pub mod db_error;

/// A Postgres connectivity probe.
pub mod health;

pub use db_error::{DbContext, DbError};
