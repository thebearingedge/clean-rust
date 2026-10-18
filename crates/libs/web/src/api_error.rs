use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};

use schemars::JsonSchema;
use serde::Serialize;
use storage::storage_error::StorageError;

#[derive(Serialize, JsonSchema)]
pub struct ErrorBody {
    pub error: String,
}

#[derive(Serialize, JsonSchema)]
pub struct ValidationDetail {
    pub path: String,
    pub message: String,
}

#[derive(Serialize, JsonSchema)]
pub struct ValidationErrorBody {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Vec<ValidationDetail>>,
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    body: serde_json::Value,
}

impl ApiError {
    /// # Panics
    ///
    /// Will panic when `body` cannot be serialized to JSON.
    pub fn new(status: StatusCode, body: &impl Serialize) -> Self {
        let body = serde_json::to_value(body)
            .expect("error bodies are plain structs and always serialize");
        Self { status, body }
    }

    #[must_use]
    pub fn message(status: StatusCode, message: &str) -> Self {
        Self::new(
            status,
            &ErrorBody {
                error: message.to_owned(),
            },
        )
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
