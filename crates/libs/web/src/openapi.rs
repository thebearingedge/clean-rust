use std::sync::Arc;

use aide::{axum::IntoApiResponse, openapi::OpenApi, transform::TransformOperation};
use axum::{Extension, Json};

use crate::api_error::{ErrorBody, ValidationErrorBody};

#[allow(clippy::unused_async)]
pub async fn serve(Extension(api): Extension<Arc<OpenApi>>) -> impl IntoApiResponse {
    Json(api)
}

pub fn standard_post_errors(op: TransformOperation<'_>) -> TransformOperation<'_> {
    op.response::<400, Json<ErrorBody>>()
        .response::<415, Json<ErrorBody>>()
        .response::<422, Json<ValidationErrorBody>>()
        .response::<500, Json<ErrorBody>>()
        .response::<503, Json<ErrorBody>>()
}
