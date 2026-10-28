mod idempotency_repository;
mod todos_repository;
mod unit_of_work;

pub use idempotency_repository::PgIdempotencyRepository;
pub use todos_repository::PgTodosRepository;
pub use unit_of_work::PgTodosUnitOfWork;

/// Embedded sqlx migrations for the `todos` domain.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();
