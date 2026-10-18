use aide::OperationInput;
use axum::{
    Json,
    extract::{FromRequest, Request},
    http::StatusCode,
};
use jsonschema::Validator;
use serde::de::DeserializeOwned;

use crate::api_error::{ApiError, ValidationDetail, ValidationErrorBody};

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

        let errors: Vec<_> = app_state
            .validator_for()
            .iter_errors(&data)
            .map(|e| ValidationDetail {
                path: e.instance_path().to_string(),
                message: e.to_string(),
            })
            .collect();

        if !errors.is_empty() {
            return Err(ApiError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                &ValidationErrorBody {
                    error: "schema violation".into(),
                    details: Some(errors),
                },
            ));
        }

        let t: T = serde_json::from_value(data).map_err(|e| {
            ApiError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                &ValidationErrorBody {
                    error: format!("deserialization failed: {e}"),
                    details: None,
                },
            )
        })?;

        Ok(ValidatedJson(t))
    }
}

impl<T> OperationInput for ValidatedJson<T>
where
    T: schemars::JsonSchema,
{
    fn operation_input(
        ctx: &mut aide::generate::GenContext,
        operation: &mut aide::openapi::Operation,
    ) {
        <Json<T> as aide::OperationInput>::operation_input(ctx, operation);
    }
}
