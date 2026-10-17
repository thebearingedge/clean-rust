use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

use todos::TodoError;
use web::ApiError;

pub struct AppError(ApiError);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        self.0.into_response()
    }
}

impl From<TodoError> for AppError {
    fn from(e: TodoError) -> Self {
        match e {
            TodoError::EmptyTask => Self(ApiError::message(
                StatusCode::BAD_REQUEST,
                "task must not be empty",
            )),
            TodoError::Storage(e) => Self(e.into()),
        }
    }
}
