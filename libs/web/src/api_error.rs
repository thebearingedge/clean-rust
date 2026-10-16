use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use tracing;

use database::db_error::DbError;

#[derive(Debug)]
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
        tracing::error!(error = ?e, "database error");
        let (status, message) = if e.is_unavailable() {
            (StatusCode::SERVICE_UNAVAILABLE, "service unavailable")
        } else {
            (StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        };
        Self::message(status, message)
    }
}
