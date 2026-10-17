use std::sync::Arc;

use jsonschema::Validator;
use sqlx::PgPool;

use todos::{TodosInteractor, repositories::postgres::TodosRepository};
use web::HasSchema;

use crate::controller::NewTodoHttp;

#[derive(Clone, Debug)]
pub struct AppState {
    pub interactors: Interactors,
    pub schemas: Schemas,
}

impl AppState {
    pub fn new(postgres: PgPool) -> Self {
        let interactors = Interactors {
            todos: TodosInteractor::new(Arc::new(TodosRepository::new(postgres))),
        };

        let schemas = Schemas {
            add_todo: Validator::new(
                &serde_json::to_value(schemars::schema_for!(NewTodoHttp)).unwrap(),
            )
            .unwrap(),
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
    pub add_todo: Validator,
}

impl HasSchema<NewTodoHttp> for AppState {
    fn validator_for(&self) -> &Validator {
        &self.schemas.add_todo
    }
}
