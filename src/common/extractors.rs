use axum::{
    Json,
    body::Bytes,
    extract::{FromRequest, Request},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use jsonschema::Validator;
use serde::de::DeserializeOwned;

pub trait HasSchema<T> {
    fn validator_for(&self) -> &Validator;
}

pub struct ValidatedJson<T>(pub T);

impl<S, T> FromRequest<S> for ValidatedJson<T>
where
    S: Send + Sync + HasSchema<T>,
    T: DeserializeOwned,
{
    type Rejection = Response;

    async fn from_request(req: Request, app_state: &S) -> Result<Self, Self::Rejection> {
        let body = Bytes::from_request(req, app_state)
            .await
            .map_err(IntoResponse::into_response)?;

        let data: serde_json::Value = serde_json::from_slice(&body).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": format!("invalid JSON: {e}") })),
            )
                .into_response()
        })?;

        let errors: Vec<_> = app_state.validator_for().iter_errors(&data).map(|e| {
            serde_json::json!({ "path": e.instance_path().to_string(), "message": e.to_string() })
        }).collect();

        if !errors.is_empty() {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({ "error": "schema violation", "details": errors })),
            )
                .into_response());
        }

        let t: T = serde_json::from_value(data).map_err(|e| {
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({ "error": format!("deserialization failed: {e}") })),
            )
                .into_response()
        })?;

        Ok(ValidatedJson(t))
    }
}
