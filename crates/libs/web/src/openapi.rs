//! `OpenAPI` helpers: the `/openapi.json` handler and the standard
//! error-response transforms that every operation should apply
//! (`standard_get_errors`/`standard_post_errors`/`standard_put_errors`).

use std::{future::Ready, sync::Arc};

use aide::{openapi::OpenApi, transform::TransformOperation};
use axum::{Extension, Json};

use crate::api_error::{ErrorBody, ValidationErrorBody};

/// Serves the generated `OpenAPI` document as JSON.
///
/// Mount it on the `/openapi.json` route; it requires the
/// `Extension<Arc<OpenApi>>` layer installed by `ApiRouter::finish_api`.
pub fn openapi_json(Extension(api): Extension<Arc<OpenApi>>) -> Ready<Json<Arc<OpenApi>>> {
    std::future::ready(Json(api))
}

/// The standard error responses every operation can produce, regardless of
/// method: 400 (bad request), 422 (deserialization/schema violation), 500
/// (internal), and 503 (unavailable).
fn standard_errors(op: TransformOperation<'_>) -> TransformOperation<'_> {
    op.response::<400, Json<ErrorBody>>()
        .response::<422, Json<ValidationErrorBody>>()
        .response::<500, Json<ErrorBody>>()
        .response::<503, Json<ErrorBody>>()
}

/// Documents the standard error responses of a GET operation (400/422/500/503).
pub fn standard_get_errors(op: TransformOperation<'_>) -> TransformOperation<'_> {
    standard_errors(op)
}

/// Documents the standard error responses of a POST operation: the GET set
/// plus 415 (unsupported media type).
pub fn standard_post_errors(op: TransformOperation<'_>) -> TransformOperation<'_> {
    standard_errors(op).response::<415, Json<ErrorBody>>()
}

/// Documents the standard error responses of a PUT operation: the GET
/// set plus 415 (unsupported media type).
pub fn standard_put_errors(op: TransformOperation<'_>) -> TransformOperation<'_> {
    standard_errors(op).response::<415, Json<ErrorBody>>()
}
