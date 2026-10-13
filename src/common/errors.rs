use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

#[derive(Debug, Error)]
#[error("{context}: {source}")]
pub struct DbError {
    pub context: String,
    #[source]
    source: sqlx::Error,
}

impl DbError {
    pub fn new(context: impl Into<String>, source: sqlx::Error) -> Self {
        Self {
            context: context.into(),
            source,
        }
    }

    pub fn is_unavailable(&self) -> bool {
        matches!(
            self.source,
            sqlx::Error::PoolTimedOut | sqlx::Error::PoolClosed | sqlx::Error::Io(_)
        )
    }
}

impl IntoResponse for DbError {
    fn into_response(self) -> Response {
        if self.is_unavailable() {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "error": "service unavailable" })),
            )
                .into_response();
        }
        tracing::error!("{}", self);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "internal server error" })),
        )
            .into_response()
    }
}
