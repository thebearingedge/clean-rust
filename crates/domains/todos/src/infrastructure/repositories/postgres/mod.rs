mod todo_row;
mod todos_repository;

pub use todos_repository::TodosRepository;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();
