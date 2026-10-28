//! The todos domain: business logic driven by [`TodosInteractor`] behind the
//! [`TodosRepository`] port, with sqlx/sea-query adapters in `infrastructure`.

mod infrastructure;
pub mod models;
mod use_cases;

// Scoped re-exports so apps can construct the concrete postgres adapter and
// run migrations against it — exactly one public path to each.
pub use infrastructure::repositories::postgres::{
    MIGRATOR, PgIdempotencyRepository, PgTodosRepository, PgTodosUnitOfWork,
};
pub use models::idempotency_key::IdempotencyKey;
pub use use_cases::{
    TodoError,
    repositories::{Claim, IdempotencyRepository, TodosRepository},
    todos_interactor::{TodoCreation, TodosInteractor},
    unit_of_work::{TodosUnit, TodosUnitOfWork},
};
