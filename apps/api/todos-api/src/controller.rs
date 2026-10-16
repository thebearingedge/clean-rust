use axum::{Json, extract::State, http::StatusCode};
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::instrument;
use uuid::Uuid;

use crate::{app_error::AppError, app_state::AppState};
use todos::TodoModel;
use web::ValidatedJson;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewTodoHttp {
    #[validate(length(min = 1))]
    task: String,
}

impl From<NewTodoHttp> for TodoModel {
    fn from(http: NewTodoHttp) -> Self {
        TodoModel {
            task: http.task,
            ..TodoModel::default()
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoHttp {
    todo_id: Option<Uuid>,
    task: String,
    updated_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
}

impl From<TodoModel> for TodoHttp {
    fn from(model: TodoModel) -> Self {
        TodoHttp {
            todo_id: model.todo_id,
            task: model.task,
            updated_at: model.updated_at,
            completed_at: model.completed_at,
        }
    }
}

#[instrument(skip_all)]
pub async fn add_todo(
    State(app_state): State<AppState>,
    ValidatedJson(new_todo): ValidatedJson<NewTodoHttp>,
) -> Result<(StatusCode, Json<TodoHttp>), AppError> {
    let model = app_state.interactors.todos.add(&new_todo.into()).await?;
    Ok((StatusCode::CREATED, Json(model.into())))
}
