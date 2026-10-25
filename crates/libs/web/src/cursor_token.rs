use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;

/// # Panics
///
/// Will panic if payload cannot be serialized.
pub fn encode<T: Serialize>(payload: &T) -> String {
    let json = serde_json::to_vec(payload)
        .expect("cursor payloads are plain structs and always serialize");
    URL_SAFE_NO_PAD.encode(json)
}

/// HTTP policy (enforced by `impl From<CursorError> for ApiError`):
/// * [`CursorError::Malformed`] — the token fails to decode → `400 Bad Request`.
/// * [`CursorError::SortMismatch`] — the token decodes but fails validation →
///   `422 Unprocessable Entity`.
#[derive(Debug, Error)]
pub enum CursorError {
    /// The token could not be decoded as base64 or as JSON.
    #[error("cursor is malformed")]
    Malformed,

    /// The token decoded but does not match the sort requested by the
    /// current query.
    #[error("cursor does not match the requested sort")]
    SortMismatch,
}

/// # Errors
///
/// Returns a [`CursorError`] if `token` is malformed. The underlying
/// base64/JSON error is logged at debug level; it is intentionally not part
/// of the public error type.
pub fn decode<T: DeserializeOwned>(token: &str) -> Result<T, CursorError> {
    let bytes = URL_SAFE_NO_PAD.decode(token).map_err(|e| {
        tracing::debug!(error = %e, "cursor token is not valid base64");
        CursorError::Malformed
    })?;
    serde_json::from_slice(&bytes).map_err(|e| {
        tracing::debug!(error = %e, "cursor token is not valid JSON");
        CursorError::Malformed
    })
}
