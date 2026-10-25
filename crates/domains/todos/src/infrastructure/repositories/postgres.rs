mod todos_repository;

pub use todos_repository::TodosRepository;

/// Embedded sqlx migrations for the `todos` domain.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();
