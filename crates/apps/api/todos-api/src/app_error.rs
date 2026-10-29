use aide::OperationOutput;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

use storage::StorageError;
use todos::TodoError;
use web::{api_error::ApiError, cursor_token::CursorError};

/// The app-layer error type: a thin newtype over [`ApiError`] so the app can
/// own the domain→HTTP mappings via `From` impls (e.g. [`TodoError`],
/// [`StorageError`]) without touching the web lib's response machinery, while
/// still implementing aide's [`OperationOutput`] for `OpenAPI` generation.
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

impl From<StorageError> for AppError {
    fn from(e: StorageError) -> Self {
        match &e {
            StorageError::Unavailable(_) => {
                tracing::error!(error = ?e, "storage unavailable");
                Self(ApiError::message(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "service unavailable",
                ))
            }
            StorageError::Conflict(_) => {
                tracing::warn!(error = ?e, "storage conflict");
                Self(ApiError::message(StatusCode::CONFLICT, "conflict"))
            }
            StorageError::Other(_) => {
                tracing::error!(error = ?e, "storage error");
                Self(ApiError::message(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal server error",
                ))
            }
        }
    }
}

impl From<TodoError> for AppError {
    fn from(e: TodoError) -> Self {
        let message = e.to_string();
        match e {
            TodoError::EmptyTask => Self(ApiError::message(StatusCode::BAD_REQUEST, &message)),
            TodoError::NotFound => Self(ApiError::message(StatusCode::NOT_FOUND, &message)),
            TodoError::IdempotencyKeyReused => Self(ApiError::message(
                StatusCode::UNPROCESSABLE_ENTITY,
                &message,
            )),
            TodoError::IdempotencyKeyInFlight => {
                Self(ApiError::message(StatusCode::CONFLICT, &message))
            }
            TodoError::Storage(e) => Self::from(e),
        }
    }
}

impl OperationOutput for AppError {
    type Inner = Self;
}

#[cfg(test)]
mod tests {
    use axum::{http::StatusCode, response::IntoResponse};

    use super::AppError;
    use storage::StorageError;
    use todos::TodoError;

    #[test]
    fn not_found_maps_to_404() {
        let response = AppError::from(TodoError::NotFound).into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn empty_task_maps_to_400() {
        let response = AppError::from(TodoError::EmptyTask).into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn idempotency_key_reused_maps_to_422() {
        let response = AppError::from(TodoError::IdempotencyKeyReused).into_response();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[test]
    fn idempotency_key_in_flight_maps_to_409() {
        let response = AppError::from(TodoError::IdempotencyKeyInFlight).into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[test]
    fn storage_unavailable_maps_to_503() {
        let error = StorageError::Unavailable("database is down".into());
        let response = AppError::from(error).into_response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn storage_conflict_maps_to_409() {
        let error = StorageError::Conflict("unique violation".into());
        let response = AppError::from(error).into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[test]
    fn other_storage_errors_map_to_500() {
        let error = StorageError::Other("something else broke".into());
        let response = AppError::from(error).into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
