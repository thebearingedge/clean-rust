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
use uuid::Uuid;

/// The interesting parts of an HTTP response: the status code, the
/// `Content-Type` header, and the parsed JSON body ([`None`] when the body is
/// not valid JSON).
pub struct ResponseParts {
    pub status: StatusCode,
    pub content_type: String,
    pub body: Option<Value>,
}

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
pub async fn post_json(router: Router, uri: &str, body: &impl Serialize) -> ResponseParts {
    let bytes = serde_json::to_vec(body).expect("JSON request body should serialize");
    post_body(router, uri, Some("application/json"), bytes.into()).await
}

/// Sends a `PUT` request with a JSON-encoded `body` and the
/// `application/json` content type, then collects the interesting parts of
/// the response.
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

/// Base for the deterministic fixture ids: a UUID-v7 layout (version nibble 7,
/// zero timestamp) whose ordering is decided entirely by the low `n` bits.
/// The values match the ids seeded by the `list_todos` fixture.
const UUID_BASE: u128 = 0x0000_0000_0000_7000_8000_0000_0000_0000;

#[must_use]
pub fn create_todo_id(n: u32) -> Uuid {
    Uuid::from_u128(UUID_BASE + u128::from(n))
}

/// Collects the `todoId` of every item in a list response as parsed UUIDs.
#[must_use]
pub fn get_todo_ids(data: &Value) -> Vec<Uuid> {
    data["items"]
        .as_array()
        .expect("list response should contain an items array")
        .iter()
        .map(|item| {
            item["todoId"]
                .as_str()
                .and_then(|id| Uuid::parse_str(id).ok())
                .expect("todoId should be a valid uuid")
        })
        .collect()
}
