//! Business rules for the todos domain: the [`repositories::TodosRepository`]
//! port, the [`todo_error::TodoError`] error set, and the
//! [`todos_interactor::TodosInteractor`] that drives them.

pub mod repositories;
pub mod todo_error;
pub mod todos_interactor;

pub use todo_error::TodoError;
