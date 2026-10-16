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
            TodoError::IdNotAllowed => Self(ApiError::message(
                StatusCode::UNPROCESSABLE_ENTITY,
                "new todo may not have an id",
            )),
            TodoError::Db(e) => Self(e.into()),
        }
    }
}
