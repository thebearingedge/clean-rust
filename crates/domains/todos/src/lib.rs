//! The todos domain: business logic ([`use_cases`]) behind repository ports
//! ([`use_cases::repositories::TodosRepository`]), with sqlx/sea-query
//! adapters in [`infrastructure`].

mod infrastructure;
pub mod models;
mod use_cases;

// Scoped re-exports so apps can construct the concrete postgres adapter and
// run migrations against it — exactly one public path to each.
pub use infrastructure::repositories::postgres::{MIGRATOR, TodosRepository};
pub use use_cases::{TodoError, todos_interactor::TodosInteractor};
