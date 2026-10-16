mod models;
mod repositories;
mod use_cases;

pub use models::todo::TodoModel;
pub use repositories::todos_repository::TodosRepository;
pub use use_cases::todos_interactor::{TodoError, TodosInteractor};

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();
