//! Database plumbing shared by infrastructure adapters: the [`DbError`]
//! error type with its classification predicates and the [`DbContext`]
//! extension trait that wraps `sqlx` results with operation context, plus a
//! readiness probe.

/// The database error type and its classification predicates.
pub mod db_error;

/// A Postgres connectivity probe.
pub mod health;

/// The [`Database`] handle with transaction support: [`Conn`] access and
/// the [`UnitOfWork`] owner.
pub mod unit_of_work;

pub use db_error::{DbContext, DbError};
pub use unit_of_work::{Conn, Database, UnitOfWork};
