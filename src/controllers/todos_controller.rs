use axum::{Json, extract::State, http::StatusCode};
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    common::extractors::ValidatedJson, controllers::app_state::AppState, models::todo::TodoModel,
    use_cases::todos_interactor::TodoError,
};

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all(serialize = "camelCase", deserialize = "snake_case"))]
pub struct NewTodoHttp {
    #[validate(length(min = 1))]
    task: String,
}

impl Into<TodoModel> for NewTodoHttp {
    fn into(self) -> TodoModel {
        TodoModel {
            task: self.task,
            ..TodoModel::default()
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all(serialize = "camelCase", deserialize = "snake_case"))]
pub struct TodoHttp {
    todo_id: Option<Uuid>,
    task: String,
    updated_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
}

impl Into<TodoHttp> for TodoModel {
    fn into(self) -> TodoHttp {
        TodoHttp {
            todo_id: self.todo_id,
            task: self.task,
            updated_at: self.updated_at,
            completed_at: self.completed_at,
        }
    }
}

pub async fn add_todo(
    State(app_state): State<AppState>,
    ValidatedJson(new_todo): ValidatedJson<NewTodoHttp>,
) -> Result<(StatusCode, Json<TodoHttp>), TodoError> {
    let model = app_state.interactors.todos.add(&new_todo.into()).await?;
    Ok((StatusCode::CREATED, Json(model.into())))
}
