//! Helpers for exercising axum routers in tests by sending one request at a
//! time through `tower`'s oneshot service adapter.
//!
//! Compiled only when the crate's `test-support` feature is enabled; intended
//! for unit and integration tests of HTTP layers built on axum.

use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode, header},
    response::Response,
};
use http_body_util::BodyExt;
use serde::Serialize;
use serde_json::Value;
use tower::ServiceExt;

/// The interesting parts of an HTTP response: the status code, the
/// `Content-Type` header, and the parsed JSON body (`None` when the body is
/// not valid JSON).
pub struct ResponseParts {
    /// The response status code.
    pub status: StatusCode,
    /// The raw `Content-Type` header value.
    pub content_type: String,
    /// The parsed JSON body, or `None` when the body is not valid JSON.
    pub body: Option<Value>,
}

/// Collects the status, `Content-Type` header, and parsed JSON body from a
/// response.
///
/// # Panics
///
/// Panics when the response has no `Content-Type` header, the header is not
/// UTF-8, or the response body cannot be collected.
pub async fn get_response_parts(res: Response<Body>) -> ResponseParts {
    let status = res.status();
    let content_type = res
        .headers()
        .get(header::CONTENT_TYPE)
        .expect("response should have a content-type header")
        .to_str()
        .expect("content-type header should be UTF-8")
        .to_owned();
    let bytes = res
        .into_body()
        .collect()
        .await
        .expect("response body should be collectable")
        .to_bytes();
    let body = serde_json::from_slice(&bytes).ok();
    ResponseParts {
        status,
        content_type,
        body,
    }
}

/// Sends a `GET` request and collects the interesting parts of the response.
///
/// # Panics
///
/// Panics when the request cannot be built or the router fails to handle it.
pub async fn get(router: Router, uri: &str) -> ResponseParts {
    let request = Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("GET request should be buildable");
    get_response_parts(
        router
            .oneshot(request)
            .await
            .expect("GET request should be handled"),
    )
    .await
}

/// Sends a `POST` request with a JSON-encoded `body` and the
/// `application/json` content type, then collects the interesting parts of the
/// response.
///
/// # Panics
///
/// Panics when `body` cannot be serialized to JSON, or the request cannot be
/// built or the router fails to handle it.
pub async fn post_json(router: Router, uri: &str, body: &impl Serialize) -> ResponseParts {
    let bytes = serde_json::to_vec(body).expect("JSON request body should serialize");
    post_body(router, uri, Some("application/json"), bytes.into()).await
}

/// Sends a `PUT` request with a JSON-encoded `body` and the
/// `application/json` content type, then collects the interesting parts of
/// the response.
///
/// # Panics
///
/// Panics when `body` cannot be serialized to JSON, or the request cannot be
/// built or the router fails to handle it.
pub async fn put_json(router: Router, uri: &str, body: &impl Serialize) -> ResponseParts {
    let bytes = serde_json::to_vec(body).expect("JSON request body should serialize");
    let request = Request::builder()
        .method(Method::PUT)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(bytes))
        .expect("PUT request should be buildable");
    get_response_parts(
        router
            .oneshot(request)
            .await
            .expect("PUT request should be handled"),
    )
    .await
}

/// Sends a `POST` request with a raw `body`, optionally setting the
/// `Content-Type` header, then collects the interesting parts of the response.
///
/// This covers the request shapes [`post_json`] cannot express: a missing
/// content type and a body that is not valid JSON.
///
/// # Panics
///
/// Panics when the request cannot be built or the router fails to handle it.
pub async fn post_body(
    router: Router,
    uri: &str,
    content_type: Option<&str>,
    body: Body,
) -> ResponseParts {
    let mut builder = Request::builder().method(Method::POST).uri(uri);
    if let Some(content_type) = content_type {
        builder = builder.header(header::CONTENT_TYPE, content_type);
    }
    let request = builder
        .body(body)
        .expect("POST request should be buildable");
    get_response_parts(
        router
            .oneshot(request)
            .await
            .expect("POST request should be handled"),
    )
    .await
}

/// Extracts the `next`-page cursor from a list response, if it has one.
#[must_use]
pub fn next_cursor(parts: &ResponseParts) -> Option<String> {
    parts
        .body
        .as_ref()
        .and_then(|body| body["next"].as_str())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::{
        ResponseParts, get, get_response_parts, next_cursor, post_body, post_json, put_json,
    };
    use axum::{
        Json, Router,
        body::{Body, Bytes},
        http::{HeaderMap, StatusCode, header},
        routing::{get as get_route, post as post_route},
    };
    use serde_json::{Value, json};
    use serde_json_assert::assert_json_include;

    #[allow(clippy::unused_async)]
    async fn echo(headers: HeaderMap, body: Bytes) -> Json<Value> {
        let content_type = headers
            .get(header::CONTENT_TYPE)
            .map(|value| value.to_str().expect("content-type header should be UTF-8"));
        let parsed = serde_json::from_slice::<Value>(&body).ok();
        Json(json!({
            "content_type": content_type,
            "body": parsed,
        }))
    }

    fn router() -> Router {
        Router::new()
            .route(
                "/ping",
                get_route(|| async { Json(json!({ "next": "cursor-1" })) }),
            )
            .route("/text", get_route(|| async { "ok" }))
            .route("/echo", post_route(echo).put(echo))
    }

    #[tokio::test]
    async fn get_collects_status_content_type_and_json_body() {
        let parts = get(router(), "/ping").await;
        assert_eq!(parts.status, StatusCode::OK);
        assert_eq!(parts.content_type, "application/json");
        assert_eq!(next_cursor(&parts), Some("cursor-1".to_owned()));
        let _body = parts.body.expect("ping response should be json");
    }

    #[tokio::test]
    async fn get_parses_a_non_json_body_as_none() {
        let parts = get(router(), "/text").await;
        assert_eq!(parts.status, StatusCode::OK);
        assert!(parts.content_type.starts_with("text/plain"));
        assert_eq!(parts.body, None);
        assert_eq!(next_cursor(&parts), None);
    }

    #[tokio::test]
    async fn post_json_sends_the_body_as_json() {
        let parts = post_json(router(), "/echo", &json!({ "hello": "world" })).await;
        assert_eq!(parts.status, StatusCode::OK);
        assert_eq!(parts.content_type, "application/json");
        let body = parts.body.expect("echo response should be json");
        assert_json_include!(actual: body, expected: json!({
            "content_type": "application/json",
            "body": { "hello": "world" },
        }));
    }

    #[tokio::test]
    async fn put_json_sends_the_body_as_json() {
        let parts = put_json(router(), "/echo", &json!({ "hello": "world" })).await;
        assert_eq!(parts.status, StatusCode::OK);
        assert_eq!(parts.content_type, "application/json");
        let body = parts.body.expect("echo response should be json");
        assert_json_include!(actual: body, expected: json!({
            "content_type": "application/json",
            "body": { "hello": "world" },
        }));
    }

    #[tokio::test]
    async fn post_body_sets_the_given_content_type() {
        let parts = post_body(
            router(),
            "/echo",
            Some("text/plain"),
            Body::from("not json"),
        )
        .await;
        assert_eq!(parts.status, StatusCode::OK);
        let body = parts.body.expect("echo response should be json");
        assert_json_include!(actual: body, expected: json!({
            "content_type": "text/plain",
            "body": null,
        }));
    }

    #[tokio::test]
    async fn post_body_omits_the_content_type_header() {
        let parts = post_body(
            router(),
            "/echo",
            None,
            Body::from(json!({ "hello": "world" }).to_string()),
        )
        .await;
        assert_eq!(parts.status, StatusCode::OK);
        let body = parts.body.expect("echo response should be json");
        assert_json_include!(actual: body, expected: json!({
            "content_type": null,
            "body": { "hello": "world" },
        }));
    }

    #[tokio::test]
    async fn get_response_parts_collects_from_a_raw_response() {
        use axum::response::IntoResponse;

        let res = (StatusCode::CREATED, Json(json!({ "id": 7 }))).into_response();
        let parts: ResponseParts = get_response_parts(res).await;
        assert_eq!(parts.status, StatusCode::CREATED);
        assert_eq!(parts.content_type, "application/json");
        let body = parts.body.expect("response should be json");
        assert_json_include!(actual: body, expected: json!({ "id": 7 }));
    }
}
