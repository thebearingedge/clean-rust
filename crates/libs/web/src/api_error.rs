//! HTTP error-response types for the web layer.
//!
//! [`ApiError`] is an HTTP response-*body* type, deliberately not
//! `std::error::Error`: it pairs a status code with a serializable body and
//! implements [`IntoResponse`] only. Apps own domain→HTTP mapping via `From`
//! impls in their `app_error.rs` (newtyping `ApiError`); the only mapping that
//! lives here is for errors the web layer itself produces
//! (`From<CursorError>`).

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::cursor_token::CursorError;

/// The standard error body for plain, non-validation error responses.
#[derive(Serialize, JsonSchema)]
pub struct ErrorBody {
    /// A generic, client-safe description of the failure.
    pub error: String,
}

#[derive(Serialize, JsonSchema)]
pub struct ValidationDetail {
    /// Where the problem is. A JSON Pointer such as `/task` for request bodies,
    /// `?name=` for a query parameter (`?` alone for the query string as a whole),
    /// and `{name}` for a path parameter.
    /// A repeated query parameter adds the zero-based position of the bad value,
    /// such as ?id[1]=.
    pub path: String,
    /// A client-safe description of what failed at `path`.
    pub message: String,
}

/// The error body for validation failures, optionally carrying per-field details.
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

/// Two-tier HTTP policy for cursor errors: a *malformed* cursor (base64/JSON
/// decode failure) is a client request error → `400 Bad Request`; an *invalid*
/// cursor (decodes but fails validation, e.g. sort-key mismatch) is a semantic
/// violation of the API schema → `422 Unprocessable Entity`. Both variants are
/// field-less, so `Display` yields only fixed, client-safe messages; internal
/// decode details are logged at debug level in [`cursor_token::decode`], never
/// surfaced here.
impl From<CursorError> for ApiError {
    fn from(e: CursorError) -> Self {
        let status = match e {
            CursorError::Malformed => StatusCode::BAD_REQUEST,
            CursorError::SortMismatch => StatusCode::UNPROCESSABLE_ENTITY,
        };
        ApiError::message(status, &e.to_string())
    }
}
