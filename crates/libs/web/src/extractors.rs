use std::fmt::Write;

use aide::OperationInput;
use axum::{
    Json,
    extract::{
        FromRequest, FromRequestParts, Path, Query, Request, path::ErrorKind,
        rejection::PathRejection,
    },
    http::{StatusCode, request::Parts},
};
use jsonschema::Validator;
use schemars::JsonSchema;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use serde_path_to_error::{Path as ErrorPath, Segment};

use crate::api_error::{ApiError, ValidationDetail, ValidationErrorBody};

/// Supplies the JSON-Schema [`Validator`] that the [`ValidatedJson`],
/// [`ValidatedQuery`], and [`ValidatedPath`] extractors enforce for type `T`.
/// Implemented by the app state; the validators typically come from
/// `schemars::schema_for!` of the request types.
pub trait HasSchema<T> {
    /// The JSON-Schema validator to enforce for `T`.
    fn validator_for(&self) -> &Validator;
}

const SCHEMA_VIOLATION: &str = "schema violation";

/// Location formatter for JSON request bodies: the schema's JSON Pointer is
/// already the client-facing location, so it is passed through verbatim.
fn raw_pointer(pointer: &str) -> String {
    pointer.to_owned()
}

/// A JSON request body validated against the JSON schema supplied by the
/// app state ([`HasSchema`]).
///
/// On failure the rejection is a 422 [`ValidationErrorBody`]. Each
/// [`ValidationDetail::path`] is a JSON Pointer into the request body
/// (e.g. `/task`); see [`ValidationDetail`] in `api_error.rs` for the full
/// location convention shared by all `Validated*` extractors.
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

        validate_instance(app_state.validator_for(), &data, raw_pointer)?;

        let t: T = serde_json::from_value(data).map_err(|e| {
            ApiError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                &ValidationErrorBody {
                    error: format!("deserialization failed: {e}"),
                    request_id: None,
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

fn parameter_error(path: String, message: String) -> ApiError {
    ApiError::new(
        StatusCode::UNPROCESSABLE_ENTITY,
        &ValidationErrorBody {
            error: SCHEMA_VIOLATION.into(),
            request_id: None,
            details: Some(vec![ValidationDetail { path, message }]),
        },
    )
}

fn parse_query<T: DeserializeOwned>(raw: &str) -> Result<T, ApiError> {
    let deserializer = serde_html_form::Deserializer::new(form_urlencoded::parse(raw.as_bytes()));
    serde_path_to_error::deserialize(deserializer).map_err(|e| {
        let pointer = pointer_from(e.path());
        parameter_error(query_location(&pointer), e.inner().to_string())
    })
}

fn query_location(pointer: &str) -> String {
    let mut segments = pointer.split('/').skip(1);
    match segments.next() {
        None => "?".to_owned(),
        Some(name) => {
            let indexes = segments.fold(String::new(), |mut out, i| {
                let _ = write!(out, "[{i}]");
                out
            });
            format!("?{name}{indexes}=")
        }
    }
}

/// A URL query string deserialized into `T` and validated against the JSON
/// schema supplied by the app state ([`HasSchema`]).
///
/// On failure the rejection is a 422 [`ValidationErrorBody`] whose
/// [`ValidationDetail::path`] names the offending query parameter as
/// `?name=`, with a zero-based index appended for repeated parameters
/// (`?name[1]=`) and `?` for a missing or wholly malformed query string; see
/// [`ValidationDetail`] in `api_error.rs` for the full location convention.
pub struct ValidatedQuery<T>(pub T);

impl<S, T> FromRequestParts<S> for ValidatedQuery<T>
where
    S: Send + Sync + HasSchema<T>,
    T: DeserializeOwned + Serialize,
{
    type Rejection = ApiError;

    #[allow(clippy::unused_async_trait_impl)]
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let value = parse_query::<T>(parts.uri.query().unwrap_or_default())?;
        validate_typed(state.validator_for(), &value, query_location)?;
        Ok(Self(value))
    }
}

impl<T: JsonSchema> aide::OperationInput for ValidatedQuery<T> {
    fn operation_input(
        ctx: &mut aide::generate::GenContext,
        operation: &mut aide::openapi::Operation,
    ) {
        <Query<T> as aide::OperationInput>::operation_input(ctx, operation);
    }
}

fn path_error(rejection: PathRejection) -> ApiError {
    match rejection {
        PathRejection::FailedToDeserializePathParams(e) => match e.kind() {
            ErrorKind::ParseErrorAtKey {
                key,
                value,
                expected_type,
            } => parameter_error(
                path_location(&format!("/{key}")),
                format!("cannot parse `{value}` as {expected_type}"),
            ),
            ErrorKind::DeserializeError { key, message, .. } => {
                parameter_error(path_location(&format!("/{key}")), message.clone())
            }
            _ => parameter_error(path_location(""), e.body_text()),
        },
        other => ApiError::message(other.status(), &other.body_text()),
    }
}

fn path_location(pointer: &str) -> String {
    pointer
        .strip_prefix('/')
        .map_or_else(String::new, |name| format!("{{{name}}}"))
}

/// A path parameter set deserialized into `T` and validated against the JSON
/// schema supplied by the app state ([`HasSchema`]).
///
/// On failure the rejection is a 422 [`ValidationErrorBody`] whose
/// [`ValidationDetail::path`] names the offending parameter as `{name}`,
/// matching the route's placeholder syntax; see [`ValidationDetail`] in
/// `api_error.rs` for the full location convention.
pub struct ValidatedPath<T>(pub T);

impl<S, T> FromRequestParts<S> for ValidatedPath<T>
where
    S: Send + Sync + HasSchema<T>,
    T: DeserializeOwned + Serialize + Send,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path(value) = Path::<T>::from_request_parts(parts, state)
            .await
            .map_err(path_error)?;
        validate_typed(state.validator_for(), &value, path_location)?;
        Ok(Self(value))
    }
}

impl<T: JsonSchema> aide::OperationInput for ValidatedPath<T> {
    fn operation_input(
        ctx: &mut aide::generate::GenContext,
        operation: &mut aide::openapi::Operation,
    ) {
        <Path<T> as aide::OperationInput>::operation_input(ctx, operation);
    }
}

fn pointer_from(path: &ErrorPath) -> String {
    path.iter()
        .map(|segment| match segment {
            Segment::Seq { index } => format!("/{index}"),
            Segment::Map { key } => format!("/{key}"),
            Segment::Enum { variant } => format!("/{variant}"),
            Segment::Unknown => "/?".to_owned(),
        })
        .collect()
}

fn validate_instance(
    validator: &Validator,
    instance: &Value,
    locate: fn(&str) -> String,
) -> Result<(), ApiError> {
    let details: Vec<_> = validator
        .iter_errors(instance)
        .map(|e| ValidationDetail {
            path: locate(&e.instance_path().to_string()),
            message: e.to_string(),
        })
        .collect();

    if details.is_empty() {
        return Ok(());
    }

    Err(ApiError::new(
        StatusCode::UNPROCESSABLE_ENTITY,
        &ValidationErrorBody {
            error: SCHEMA_VIOLATION.into(),
            request_id: None,
            details: Some(details),
        },
    ))
}

fn validate_typed<T: Serialize>(
    validator: &Validator,
    value: &T,
    locate: fn(&str) -> String,
) -> Result<(), ApiError> {
    let instance =
        serde_json::to_value(value).expect("parameter structs are plain data and always serialize");
    validate_instance(validator, &instance, locate)
}

#[cfg(test)]
mod tests {
    use axum::{
        Json, Router,
        body::Body,
        http::{Request, StatusCode, header},
        response::IntoResponse,
        routing::get,
    };
    use http_body_util::BodyExt;
    use jsonschema::Validator;
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};
    use serde_json::Value;
    use serde_json_assert::assert_json_include;
    use tower::ServiceExt;
    use uuid::Uuid;

    use super::{HasSchema, ValidatedPath, ValidatedQuery};

    #[derive(Deserialize, Serialize, JsonSchema)]
    struct Params {
        #[validate(length(max = 2))]
        id: Option<Vec<u64>>,
        #[validate(range(min = 1))]
        limit: u32,
    }

    #[derive(Deserialize, Serialize, JsonSchema)]
    struct Path {
        id: Uuid,
    }

    #[derive(Clone)]
    struct TestState {
        params: Validator,
        path: Validator,
    }

    impl HasSchema<Params> for TestState {
        fn validator_for(&self) -> &Validator {
            &self.params
        }
    }

    impl HasSchema<Path> for TestState {
        fn validator_for(&self) -> &Validator {
            &self.path
        }
    }

    async fn list(ValidatedQuery(params): ValidatedQuery<Params>) -> impl IntoResponse {
        Json(serde_json::json!({ "limit": params.limit, "id": params.id }))
    }

    async fn show(ValidatedPath(path): ValidatedPath<Path>) -> impl IntoResponse {
        Json(serde_json::json!({ "id": path.id }))
    }

    fn router() -> Router {
        let params_schema = serde_json::to_value(schemars::schema_for!(Params)).unwrap();
        let params_validator = jsonschema::validator_for(&params_schema).unwrap();
        let path_schema = serde_json::to_value(schemars::schema_for!(Path)).unwrap();
        let path_validator = jsonschema::validator_for(&path_schema).unwrap();
        Router::new()
            .route("/list", get(list))
            .route("/items/{id}", get(show))
            .with_state(TestState {
                params: params_validator,
                path: path_validator,
            })
    }

    async fn get_json_response_parts(uri: &str) -> (StatusCode, String, Value) {
        let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
        let res = router().oneshot(req).await.unwrap();
        let status = res.status();
        let content_type = res
            .headers()
            .get(header::CONTENT_TYPE)
            .map(|v| v.to_str().unwrap().to_owned())
            .unwrap_or_default();
        let body = res.into_body().collect().await.unwrap().to_bytes();
        let data = serde_json::from_slice(&body).unwrap();
        (status, content_type, data)
    }

    #[tokio::test]
    async fn a_bad_path_parameter_is_a_422_with_details() {
        let (status, content_type, data) = get_json_response_parts("/items/abc").await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(content_type, "application/json");
        assert_json_include!(actual: data, expected: serde_json::json!({
            "error": "schema violation",
            "details": [{ "path": "{id}" }]
        }));
    }

    #[tokio::test]
    async fn a_valid_path_parameter_is_extracted() {
        let id = Uuid::now_v7();
        let (status, content_type, data) = get_json_response_parts(&format!("/items/{id}")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type, "application/json");
        assert_json_include!(actual: data, expected: serde_json::json!({ "id": id.to_string() }));
    }

    #[tokio::test]
    async fn a_bad_query_string_is_a_422_with_details() {
        let (status, content_type, data) = get_json_response_parts("/list?limit=abc").await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(content_type, "application/json");
        assert_json_include!(actual: data, expected: serde_json::json!({
            "error": "schema violation",
            "details": [{ "path": "?limit=" }]
        }));
    }

    #[tokio::test]
    async fn a_query_violation_is_a_422_with_details() {
        let (status, content_type, data) = get_json_response_parts("/list?limit=0").await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(content_type, "application/json");
        assert_json_include!(actual: data, expected: serde_json::json!({
            "error": "schema violation",
            "details": [{ "path": "?limit=" }]
        }));
    }

    #[tokio::test]
    async fn a_repeated_scalar_is_a_422_with_details() {
        let (status, content_type, data) = get_json_response_parts("/list?limit=1&limit=2").await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(content_type, "application/json");
        assert_json_include!(actual: data, expected: serde_json::json!({
            "error": "schema violation",
            "details": [{ "path": "?limit=" }]
        }));
    }

    #[tokio::test]
    async fn too_many_query_params_is_a_422_with_details() {
        let (status, content_type, data) =
            get_json_response_parts("/list?limit=1&id=1&id=2&id=3").await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(content_type, "application/json");
        assert_json_include!(actual: data, expected: serde_json::json!({
            "error": "schema violation",
            "details": [{ "path": "?id=" }]
        }));
    }

    #[tokio::test]
    async fn a_specific_invalid_query_param_is_a_422_with_details() {
        let (status, content_type, data) =
            get_json_response_parts("/list?limit=1&id=1&id=foo").await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(content_type, "application/json");
        assert_json_include!(actual: data, expected: serde_json::json!({
            "error": "schema violation",
            "details": [{ "path": "?id[1]=" }]
        }));
    }

    #[tokio::test]
    async fn a_missing_query_is_a_422_with_details() {
        let (status, content_type, data) = get_json_response_parts("/list").await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(content_type, "application/json");
        assert_json_include!(actual: data, expected: serde_json::json!({
            "error": "schema violation",
            "details": [{ "path": "?" }]
        }));
    }

    #[tokio::test]
    async fn valid_query_parameter_passes_through() {
        let (status, content_type, data) = get_json_response_parts("/list?limit=5").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type, "application/json");
        assert_json_include!(actual: data, expected: serde_json::json!({
            "id": null,
            "limit": 5,
        }));
    }
}
