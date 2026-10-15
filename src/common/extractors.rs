use axum::{
    Json,
    extract::{FromRequest, Request},
    http::StatusCode,
};
use jsonschema::Validator;
use serde::de::DeserializeOwned;

use crate::controllers::api_error::ApiError;

pub trait HasSchema<T> {
    fn validator_for(&self) -> &Validator;
}

pub struct ValidatedJson<T>(pub T);

impl<S, T> FromRequest<S> for ValidatedJson<T>
where
    S: Send + Sync + HasSchema<T>,
    T: DeserializeOwned,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, app_state: &S) -> Result<Self, Self::Rejection> {
        let Json(data) = Json::<serde_json::Value>::from_request(req, app_state)
            .await
            .map_err(|e| ApiError::message(e.status(), &e.body_text()))?;

        let errors: Vec<_> = app_state.validator_for().iter_errors(&data).map(|e| {
            serde_json::json!({ "path": e.instance_path().to_string(), "message": e.to_string() })
        }).collect();

        if !errors.is_empty() {
            return Err(ApiError::json(
                StatusCode::UNPROCESSABLE_ENTITY,
                serde_json::json!({ "error": "schema violation", "details": errors }),
            ));
        }

        let t: T = serde_json::from_value(data).map_err(|e| {
            ApiError::message(
                StatusCode::UNPROCESSABLE_ENTITY,
                &format!("deserialization failed: {}", e),
            )
        })?;

        Ok(ValidatedJson(t))
    }
}
