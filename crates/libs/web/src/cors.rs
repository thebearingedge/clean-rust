//! CORS layer composition for browser clients.

use std::time::Duration;

use axum::{
    Router,
    http::{HeaderName, HeaderValue, Method, header},
};
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::request_id::REQUEST_ID_HEADER;

const IDEMPOTENCY_KEY: HeaderName = HeaderName::from_static("idempotency-key");
const TRACEPARENT: HeaderName = HeaderName::from_static("traceparent");
const TRACESTATE: HeaderName = HeaderName::from_static("tracestate");
const CORS_MAX_AGE: Duration = Duration::from_secs(600);

/// Wraps `router` with a [`CorsLayer`] for the given exact origins: requests
/// whose `Origin` is not in `allowed_origins` receive no CORS headers, and the
/// layer never emits a wildcard `Access-Control-Allow-Origin` or reflects an
/// arbitrary request origin.
///
/// Preflight `OPTIONS` requests are answered directly by the layer without
/// reaching `router`; responses to actual requests from allowed origins carry
/// the response headers named by
/// [`Access-Control-Expose-Headers`](https://fetch.spec.whatwg.org/#http-access-control-expose-headers)
/// (`location` and [`crate::request_id::REQUEST_ID_HEADER`]).
///
/// An empty `allowed_origins` leaves the router unchanged (CORS off), so
/// callers without configured origins get no CORS layer at all.
///
/// # Panics
///
/// Panics when `allowed_origins` contains a wildcard (`*`) value, via
/// [`AllowOrigin::list`].
pub fn with_cors(router: Router, allowed_origins: Vec<HeaderValue>) -> Router {
    if allowed_origins.is_empty() {
        return router;
    }
    router.layer(
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(allowed_origins))
            .allow_methods([Method::GET, Method::POST])
            .allow_headers([
                header::CONTENT_TYPE,
                IDEMPOTENCY_KEY,
                TRACEPARENT,
                TRACESTATE,
            ])
            .expose_headers([header::LOCATION, REQUEST_ID_HEADER])
            .max_age(CORS_MAX_AGE),
    )
}

#[cfg(test)]
mod tests {
    use axum::{
        Router,
        body::Body,
        http::{self, HeaderValue, Request, StatusCode},
        response::Response,
        routing::get,
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use super::with_cors;
    use crate::request_id::REQUEST_ID_HEADER;

    const ALLOWED_ORIGIN: &str = "http://localhost:5173";
    const UNLISTED_ORIGIN: &str = "http://localhost:9999";

    fn router() -> Router {
        Router::new().route("/todos", get(|| async { "ok" }))
    }

    fn allowed_origins() -> Vec<HeaderValue> {
        vec![HeaderValue::from_static(ALLOWED_ORIGIN)]
    }

    fn preflight(origin: &str, request_method: &str) -> Request<Body> {
        Request::builder()
            .method(http::Method::OPTIONS)
            .uri("/todos")
            .header(http::header::ORIGIN, origin)
            .header(http::header::ACCESS_CONTROL_REQUEST_METHOD, request_method)
            .header(
                http::header::ACCESS_CONTROL_REQUEST_HEADERS,
                "content-type, idempotency-key",
            )
            .body(Body::empty())
            .unwrap()
    }

    fn request(method: &'static str, uri: &str, origin: &str) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header(http::header::ORIGIN, origin)
            .body(Body::empty())
            .unwrap()
    }

    async fn body(res: Response) -> Vec<u8> {
        res.into_body().collect().await.unwrap().to_bytes().to_vec()
    }

    fn cors_headers(res: &Response) -> Vec<&str> {
        res.headers()
            .keys()
            .filter_map(|name| {
                name.as_str()
                    .starts_with("access-control-")
                    .then_some(name.as_str())
            })
            .collect()
    }

    #[tokio::test]
    async fn empty_origin_list_leaves_the_router_without_cors() {
        let res = with_cors(router(), vec![])
            .oneshot(preflight(ALLOWED_ORIGIN, "POST"))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(cors_headers(&res), [] as [&str; 0]);
    }

    #[tokio::test]
    async fn allowed_origin_preflight_is_answered_with_the_exact_origin() {
        let res = with_cors(router(), allowed_origins())
            .oneshot(preflight(ALLOWED_ORIGIN, "POST"))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get("access-control-allow-origin").unwrap(),
            ALLOWED_ORIGIN
        );
        assert_eq!(
            res.headers().get("access-control-allow-methods").unwrap(),
            "GET,POST"
        );
        assert!(
            res.headers()
                .get("access-control-allow-headers")
                .unwrap()
                .to_str()
                .unwrap()
                .contains("idempotency-key")
        );
        assert_eq!(res.headers().get("access-control-max-age").unwrap(), "600");
    }

    #[tokio::test]
    async fn unlisted_origin_preflight_has_no_allow_origin() {
        let res = with_cors(router(), allowed_origins())
            .oneshot(preflight(UNLISTED_ORIGIN, "POST"))
            .await
            .unwrap();
        assert!(res.headers().get("access-control-allow-origin").is_none());
    }

    #[tokio::test]
    async fn actual_request_from_allowed_origin_carries_expose_headers() {
        let res = with_cors(router(), allowed_origins())
            .oneshot(request("GET", "/todos", ALLOWED_ORIGIN))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get("access-control-allow-origin").unwrap(),
            ALLOWED_ORIGIN
        );
        let expose = res
            .headers()
            .get("access-control-expose-headers")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(expose.contains("location"));
        assert!(expose.contains(REQUEST_ID_HEADER.as_str()));
        assert_eq!(body(res).await, b"ok".to_vec());
    }

    #[tokio::test]
    async fn disallowed_method_preflight_advertises_no_disallowed_methods() {
        let res = with_cors(router(), allowed_origins())
            .oneshot(preflight(ALLOWED_ORIGIN, "DELETE"))
            .await
            .unwrap();
        let methods = res
            .headers()
            .get("access-control-allow-methods")
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(methods, "GET,POST");
        assert!(!methods.contains("DELETE"));
    }
}
