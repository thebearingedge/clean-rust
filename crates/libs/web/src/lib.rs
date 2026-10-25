//! Shared axum web plumbing: HTTP error bodies, schema-validated extractors,
//! health-probe routing, tracing middleware, and `OpenAPI` helpers.

pub mod api_error;
pub mod cursor_token;
pub mod extractors;
mod graceful_shutdown;
pub mod health;
pub mod middleware;
pub mod openapi;

/// A future that resolves when the process should shut down (Ctrl+C or
/// SIGTERM on Unix).
///
/// `graceful_shutdown` is deliberately private, unlike its sibling modules:
/// consumers need only the future itself, not the platform-specific handler
/// installation it encapsulates, so the public surface stays at the crate
/// root as `web::shutdown_signal`.
pub use graceful_shutdown::shutdown_signal;
