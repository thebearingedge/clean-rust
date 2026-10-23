use aide::OperationOutput;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

use todos::TodoError;
use web::{api_error::ApiError, cursor_token::CursorError};

pub struct AppError(ApiError);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        self.0.into_response()
    }
}

impl From<CursorError> for AppError {
    fn from(e: CursorError) -> Self {
        Self(e.into())
    }
}

impl From<TodoError> for AppError {
    fn from(e: TodoError) -> Self {
        match e {
            TodoError::EmptyTask => Self(ApiError::message(
                StatusCode::BAD_REQUEST,
                "task must not be empty",
            )),
            TodoError::NotFound => Self(ApiError::message(StatusCode::NOT_FOUND, "todo not found")),
            TodoError::Storage(e) => Self(e.into()),
        }
    }
}

impl OperationOutput for AppError {
    type Inner = Self;
}

#[cfg(test)]
mod tests {
    use axum::{http::StatusCode, response::IntoResponse};
    use todos::TodoError;

    use super::AppError;

    #[test]
    fn not_found_maps_to_404() {
        let response = AppError::from(TodoError::NotFound).into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
