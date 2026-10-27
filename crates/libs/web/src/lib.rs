//! Shared axum web plumbing: HTTP error bodies, schema-validated extractors,
//! health-probe routing, tracing middleware, `OpenAPI` helpers, and — under
//! the `test-support` feature — oneshot HTTP test helpers.

/// HTTP error-response body types ([`crate::api_error::ErrorBody`],
/// [`crate::api_error::ValidationErrorBody`]) and the
/// [`crate::api_error::ApiError`] response wrapper.
pub mod api_error;

/// Encoding and decoding of opaque, base64url pagination cursor tokens.
pub mod cursor_token;

/// JSON fallback handlers and the [`crate::fallback::with_json_fallbacks`]
/// router helper for axum's empty 404 and 405 responses.
pub mod fallback;

/// JSON-Schema-validated extractors: [`crate::extractors::ValidatedJson`],
/// [`crate::extractors::ValidatedQuery`], and
/// [`crate::extractors::ValidatedPath`].
pub mod extractors;
mod graceful_shutdown;

/// Liveness (`/livez`) and readiness (`/readyz`) probe router.
pub mod health;

/// OpenTelemetry-convention request tracing middleware.
pub mod middleware;

/// `OpenAPI` document handler and standard error-response transforms.
pub mod openapi;

/// Request-ID middleware: generation, propagation, sanitization, and
/// task-local access to the current request's ID.
pub mod request_id;

/// Oneshot HTTP test helpers for exercising routers and handlers
/// ([`crate::test_support::get`], [`crate::test_support::post_json`],
/// [`crate::test_support::put_json`], [`crate::test_support::post_body`]) —
/// compiled only when the `test-support` feature is enabled.
#[cfg(feature = "test-support")]
pub mod test_support;

/// A future that resolves when the process should shut down (Ctrl+C or
/// SIGTERM on Unix).
///
/// `graceful_shutdown` is deliberately private, unlike its sibling modules:
/// consumers need only the future itself, not the platform-specific handler
/// installation it encapsulates, so the public surface stays at the crate
/// root as `web::shutdown_signal`.
pub use graceful_shutdown::shutdown_signal;
