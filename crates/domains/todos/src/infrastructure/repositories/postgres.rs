mod pg_todos_repository;
mod pg_unit_of_work;

pub use pg_todos_repository::PgTodosRepository;
pub use pg_unit_of_work::PgTodosUnitOfWork;

/// Embedded sqlx migrations for the `todos` domain.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();
