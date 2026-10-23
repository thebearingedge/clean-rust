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

#[derive(Debug, Error)]
pub enum CursorError {
    #[error("cursor is malformed")]
    Malformed,
    #[error("cursor does not match the requested sort")]
    SortMismatch,
}

/// # Errors
///
/// Returns a [`CursorError`] if `token` is malformed.
pub fn decode<T: DeserializeOwned>(token: &str) -> Result<T, CursorError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(token)
        .map_err(|_| CursorError::Malformed)?;
    serde_json::from_slice(&bytes).map_err(|_| CursorError::Malformed)
}
