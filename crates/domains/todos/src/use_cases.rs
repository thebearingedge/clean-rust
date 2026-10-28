//! Business rules for the todos domain: the [`repositories::TodosRepository`]
//! port, the [`todo_error::TodoError`] error set, the
//! [`todos_interactor::TodosInteractor`] that drives them, and the
//! [`unit_of_work::TodosUnitOfWork`] transaction port.

pub mod repositories;
pub mod todo_error;
pub mod todos_interactor;
pub mod unit_of_work;

pub use todo_error::TodoError;
