use std::time::Duration;

use axum::{
    Router,
    extract::MatchedPath,
    http::{Request, Response},
};
use tower_http::trace::{DefaultOnFailure, MakeSpan, OnResponse, TraceLayer};
use tracing::{Level, Span};

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

#[derive(Clone, Copy)]
pub struct HttpOnResponse;

impl<B> OnResponse<B> for HttpOnResponse {
    fn on_response(self, response: &Response<B>, _latency: Duration, span: &Span) {
        span.record("http.response.status_code", response.status().as_u16());
    }
}

pub fn with_http_tracing(router: Router) -> Router {
    router.layer(
        TraceLayer::new_for_http()
            .make_span_with(HttpMakeSpan)
            .on_request(())
            .on_response(HttpOnResponse)
            .on_failure(DefaultOnFailure::new().level(Level::ERROR)),
    )
}
