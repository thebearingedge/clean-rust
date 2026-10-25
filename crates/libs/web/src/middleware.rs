use std::time::Duration;

use axum::{
    Router,
    extract::MatchedPath,
    http::{Request, Response},
};
use tower_http::trace::{DefaultOnFailure, MakeSpan, OnResponse, TraceLayer};
use tracing::{Level, Span};

/// `MakeSpan` implementation following the OpenTelemetry HTTP server
/// semantic conventions: `otel.name` is `"{method} {route}"` (method alone
/// when no route matched), `otel.kind` is `"server"`, and the standard
/// fields (`http.request.method`, `http.route`, `url.path`,
/// `http.response.status_code`) are recorded. The incoming parent context is
/// linked from the request headers via `telemetry::propagation::link_parent`.
#[derive(Clone, Copy)]
pub struct HttpMakeSpan;

impl<B> MakeSpan<B> for HttpMakeSpan {
    fn make_span(&mut self, req: &Request<B>) -> Span {
        let method = req.method().as_str();
        let route = req
            .extensions()
            .get::<MatchedPath>()
            .map(MatchedPath::as_str);
        let name = match route {
            Some(route) => format!("{method} {route}"),
            None => method.to_owned(),
        };
        let span = tracing::info_span!(
            "http.request",
            otel.name = %name,
            otel.kind = "server",
            http.request.method = %method,
            http.route = route,
            url.path = %req.uri().path(),
            http.response.status_code = tracing::field::Empty,
        );

        telemetry::propagation::link_parent(&span, req.headers());
        span
    }
}

/// `OnResponse` implementation that records the response status into the
/// span's `http.response.status_code` field.
#[derive(Clone, Copy)]
pub struct HttpOnResponse;

impl<B> OnResponse<B> for HttpOnResponse {
    fn on_response(self, response: &Response<B>, _latency: Duration, span: &Span) {
        span.record("http.response.status_code", response.status().as_u16());
    }
}

/// Wraps `router` with a [`TraceLayer`] using the OTel-convention span from
/// [`HttpMakeSpan`], recording the status code on response via
/// [`HttpOnResponse`] and logging failures at ERROR level. Requests and
/// responses are not logged.
pub fn with_http_tracing(router: Router) -> Router {
    router.layer(
        TraceLayer::new_for_http()
            .make_span_with(HttpMakeSpan)
            .on_request(())
            .on_response(HttpOnResponse)
            .on_failure(DefaultOnFailure::new().level(Level::ERROR)),
    )
}
