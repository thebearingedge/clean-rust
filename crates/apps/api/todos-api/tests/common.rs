//! Shared helpers for the todos-api integration test binaries. Keep this
//! module to helpers every test binary uses — endpoint-specific helpers
//! belong in the test file that uses them, so no dead-code allows are
//! needed here.

use axum::Router;
use sqlx::PgPool;

use todos_api::{api::create_todos_api, app_state::AppState};
use web::middleware::with_http_tracing;

/// Builds the todos API router against the test database.
pub fn todos_api(postgres: PgPool) -> Router {
    with_http_tracing(create_todos_api(AppState::new(postgres)))
}
