use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};

use storage::storage_error::StorageError;

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    body: serde_json::Value,
}

impl ApiError {
    #[must_use]
    pub fn message(status: StatusCode, message: &str) -> Self {
        Self::json(status, serde_json::json!({ "error": message }))
    }

    #[must_use]
    pub fn json(status: StatusCode, body: serde_json::Value) -> Self {
        Self { status, body }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}

impl From<StorageError> for ApiError {
    fn from(e: StorageError) -> Self {
        match &e {
            StorageError::Unavailable(_) => {
                tracing::error!(error = ?e, "storage unavailable");
                ApiError::message(StatusCode::SERVICE_UNAVAILABLE, "service unavailable")
            }
            StorageError::Conflict(_) => {
                tracing::warn!(error = ?e, "storage conflict");
                ApiError::message(StatusCode::CONFLICT, "conflict")
            }
            StorageError::Other(_) => {
                tracing::error!(error = ?e, "storage error");
                ApiError::message(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
            }
        }
    }
}
