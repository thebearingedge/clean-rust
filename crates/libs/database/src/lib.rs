//! Database plumbing shared by infrastructure adapters: the [`DbError`]
//! error type with its classification predicates and the [`DbContext`]
//! extension trait that wraps `sqlx` results with operation context, a
//! readiness probe, and the idempotency-key machinery behind idempotent
//! create flows.

/// The database error type and its classification predicates.
pub mod db_error;

/// A Postgres connectivity probe.
pub mod health;

/// Idempotency keys, the repository port for claiming and completing them,
/// and its Postgres adapter.
pub mod idempotency;

/// The [`Database`] handle with transaction support: [`Conn`] access and
/// the [`UnitOfWork`] owner.
pub mod unit_of_work;

pub use db_error::{DbContext, DbError};
pub use idempotency::{
    Claim, IDEMPOTENCY_MAX_LENGTH, IDEMPOTENCY_MIN_LENGTH, IDEMPOTENCY_VISIBLE_ASCII_PATTERN,
    IdempotencyKey, IdempotencyRepository, PgIdempotencyRepository,
};
pub use unit_of_work::{Conn, Database, UnitOfWork};

/// Migrations for the canonical base schema: the `util` schema, the `string`
/// domain, `util.add_updated_at`, the `idempotency_key_status` type, and the
/// `idempotency_keys` table.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();
