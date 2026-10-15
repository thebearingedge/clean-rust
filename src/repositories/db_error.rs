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

pub trait DbContext<T> {
    fn db_context(self, context: impl Into<String>) -> Result<T, DbError>;
}

impl<T> DbContext<T> for Result<T, sqlx::Error> {
    fn db_context(self, context: impl Into<String>) -> Result<T, DbError> {
        self.map_err(|e| DbError::new(context, e))
    }
}

impl IntoResponse for DbError {
    fn into_response(self) -> Response {
        tracing::error!(error = ?self, "database error");
        let (status, message) = if self.is_unavailable() {
            (StatusCode::SERVICE_UNAVAILABLE, "service unavailable")
        } else {
            (StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        };
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
