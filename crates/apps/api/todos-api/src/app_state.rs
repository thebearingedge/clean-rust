use std::sync::Arc;

use jsonschema::Validator;
use schemars::JsonSchema;
use sqlx::PgPool;

use crate::controller::{ListTodosQuery, NewTodoHttp, TodoPath, TodoUpdateHttp};
use database::Database;
use todos::{PgTodosRepository, PgTodosUnitOfWork, TodosInteractor};
use web::extractors::HasSchema;

/// The axum app state shared by every handler: domain interactors and the
/// per-request-DTO schema validators, both constructed from a Postgres pool
/// by [`AppState::new`].
#[derive(Clone, Debug)]
pub struct AppState {
    /// The use-case interactors the handlers call into.
    pub interactors: Interactors,
    /// The JSON-schema validators the validated extractors look up through
    /// the [`HasSchema`] impls on this type.
    pub schemas: Schemas,
}

impl AppState {
    /// Creates the state from the Postgres pool: the todos repository
    /// adapter and the todos unit-of-work adapter are built on the pool and
    /// wrapped in a [`TodosInteractor`], and one schema validator is derived
    /// per request DTO ([`NewTodoHttp`], [`TodoUpdateHttp`], [`TodoPath`],
    /// [`ListTodosQuery`]).
    ///
    /// # Panics
    ///
    /// Panics if a derived schema fails to serialize or compile as a
    /// validator, which cannot happen for schemars-derived schemas.
    #[must_use]
    pub fn new(postgres: PgPool) -> Self {
        let database = Database::Pool(postgres);

        let interactors = Interactors {
            todos: TodosInteractor::new(
                Arc::new(PgTodosRepository::new(database.clone())),
                Arc::new(PgTodosUnitOfWork::new(database)),
            ),
        };

        let schemas = Schemas {
            add_todo: validator::<NewTodoHttp>(),
            update_todo: validator::<TodoUpdateHttp>(),
            get_todo: validator::<TodoPath>(),
            list_todos: validator::<ListTodosQuery>(),
        };

        Self {
            interactors,
            schemas,
        }
    }
}

/// The domain half of the two-part app state: the use-case interactors the
/// handlers call into, backed by concrete repository adapters constructed in
/// [`AppState::new`].
#[derive(Clone, Debug)]
pub struct Interactors {
    /// The todos domain interactor all handlers call into.
    pub todos: TodosInteractor,
}

/// The request-validation half of the two-part app state: one JSON-schema
/// [`Validator`] per request DTO, keyed by the [`HasSchema`] impls on
/// [`AppState`] so the validated extractors can look them up.
#[derive(Clone, Debug)]
pub struct Schemas {
    add_todo: Validator,
    update_todo: Validator,
    get_todo: Validator,
    list_todos: Validator,
}

impl HasSchema<NewTodoHttp> for AppState {
    fn validator_for(&self) -> &Validator {
        &self.schemas.add_todo
    }
}

impl HasSchema<TodoUpdateHttp> for AppState {
    fn validator_for(&self) -> &Validator {
        &self.schemas.update_todo
    }
}

impl HasSchema<TodoPath> for AppState {
    fn validator_for(&self) -> &Validator {
        &self.schemas.get_todo
    }
}

impl HasSchema<ListTodosQuery> for AppState {
    fn validator_for(&self) -> &Validator {
        &self.schemas.list_todos
    }
}

fn validator<T: JsonSchema>() -> Validator {
    let schema =
        serde_json::to_value(schemars::schema_for!(T)).expect("derived schemas always serialize");
    jsonschema::validator_for(&schema).expect("derived schemas are valid")
}
