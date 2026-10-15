use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::{repositories::db_error::DbError, use_cases::todos_interactor::TodoError};

pub struct ApiError {
    status: StatusCode,
    body: serde_json::Value,
}

impl ApiError {
    pub fn message(status: StatusCode, message: &str) -> Self {
        Self::json(status, serde_json::json!({ "error": message }))
    }

    pub fn json(status: StatusCode, body: serde_json::Value) -> Self {
        Self { status, body }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}

impl From<DbError> for ApiError {
    fn from(e: DbError) -> Self {
        tracing::error!(error = %e, "database error");
        if e.is_unavailable() {
            ApiError::message(StatusCode::SERVICE_UNAVAILABLE, "service unavailable")
        } else {
            ApiError::message(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        }
    }
}

impl From<TodoError> for ApiError {
    fn from(e: TodoError) -> Self {
        match e {
            TodoError::EmptyTask => {
                ApiError::message(StatusCode::BAD_REQUEST, "task must not be empty")
            }
            TodoError::IdNotAllowed => ApiError::message(
                StatusCode::UNPROCESSABLE_ENTITY,
                "new todo may not have an id",
            ),
            TodoError::Db(e) => e.into(),
        }
    }
}
