use std::sync::Arc;

use jsonschema::Validator;
use schemars::JsonSchema;
use sqlx::PgPool;

use todos::{TodosInteractor, repositories::postgres::TodosRepository};
use web::extractors::HasSchema;

use crate::controller::{ListTodosQuery, NewTodoBody, TodoPath};

#[derive(Clone, Debug)]
pub struct AppState {
    pub interactors: Interactors,
    pub schemas: Schemas,
}

impl AppState {
    #[must_use]
    pub fn new(postgres: PgPool) -> Self {
        let interactors = Interactors {
            todos: TodosInteractor::new(Arc::new(TodosRepository::new(postgres))),
        };

        let schemas = Schemas {
            add_todo: validator::<NewTodoBody>(),
            get_todo: validator::<TodoPath>(),
            list_todos: validator::<ListTodosQuery>(),
        };

        Self {
            interactors,
            schemas,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Interactors {
    pub todos: TodosInteractor,
}

#[derive(Clone, Debug)]
pub struct Schemas {
    add_todo: Validator,
    get_todo: Validator,
    list_todos: Validator,
}

impl HasSchema<NewTodoBody> for AppState {
    fn validator_for(&self) -> &Validator {
        &self.schemas.add_todo
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
