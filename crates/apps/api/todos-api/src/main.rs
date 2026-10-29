//! todos-api application binary.

use std::net::SocketAddr;
use std::time::Duration;

use anyhow::{Context, ensure};
use axum::{Router, http::HeaderValue, serve};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;

use telemetry::provider::init_tracer_provider;
use todos::MIGRATOR;
use todos_api::{api::create_todos_api, app_state::AppState};
use web::{
    cors::with_cors,
    graceful_shutdown::{Draining, graceful_shutdown},
    health::{self, HealthCheck},
    middleware::with_http_tracing,
};

const MAX_CONNECTIONS: u32 = 10;
const MIN_CONNECTIONS: u32 = 2;
const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(3);
const READINESS_TIMEOUT: Duration = Duration::from_secs(2);

async fn build_pool(database_url: &str) -> anyhow::Result<PgPool> {
    tracing::info!("connecting to postgres...");

    PgPoolOptions::new()
        .min_connections(MIN_CONNECTIONS)
        .max_connections(MAX_CONNECTIONS)
        .acquire_timeout(ACQUIRE_TIMEOUT)
        .connect(database_url)
        .await
        .context("connecting to postgres")
}

/// Parses a comma-separated `CORS_ALLOWED_ORIGINS` value into exact-origin
/// header values. Entries are trimmed, empty entries are dropped, and empty
/// input yields an empty list (CORS off).
///
/// # Errors
///
/// Returns an error when an entry ends with a slash or is not a valid header
/// value.
fn parse_allowed_origins(raw: &str) -> anyhow::Result<Vec<HeaderValue>> {
    raw.split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .map(|origin| {
            ensure!(
                !origin.ends_with('/'),
                "CORS origin must not end with a slash: {origin}"
            );
            HeaderValue::from_str(origin).with_context(|| format!("invalid CORS origin: {origin}"))
        })
        .collect()
}

fn build_router(pool: &PgPool, draining: Draining, allowed_origins: Vec<HeaderValue>) -> Router {
    with_cors(
        with_http_tracing(create_todos_api(AppState::new(pool.clone()))),
        allowed_origins,
    )
    .merge(health::router(
        vec![HealthCheck::with_resource(
            "postgres",
            pool.clone(),
            database::health::ping,
        )],
        READINESS_TIMEOUT,
        draining,
    ))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let tracer_provider = init_tracer_provider("todos_api");

    let drain_delay = std::env::var("SHUTDOWN_DRAIN_SECONDS")
        .ok()
        .map_or(Duration::ZERO, |raw| {
            if let Ok(secs) = raw.parse::<u64>() {
                Duration::from_secs(secs)
            } else {
                tracing::warn!(raw, "unparsable SHUTDOWN_DRAIN_SECONDS, defaulting to 0");
                Duration::ZERO
            }
        });

    let draining = Draining::default();

    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;

    let allowed_origins =
        parse_allowed_origins(&std::env::var("CORS_ALLOWED_ORIGINS").unwrap_or_default())
            .context("invalid CORS_ALLOWED_ORIGINS")?;

    let pool = build_pool(&database_url).await?;

    tracing::info!("connected to postgres. running migrations...");

    MIGRATOR.run(&pool).await.context("running migrations")?;

    tracing::info!("migrations up to date");

    let router = build_router(&pool, draining.clone(), allowed_origins);

    let address: SocketAddr = std::env::var("LISTEN_ADDRESS")
        .unwrap_or("0.0.0.0:3000".to_string())
        .parse()
        .context("getting socket address")?;

    let listener = TcpListener::bind(&address)
        .await
        .context("binding listener")?;

    tracing::info!("listening at {address}");

    let shutdown_server = serve(listener, router)
        .with_graceful_shutdown(graceful_shutdown(draining, drain_delay))
        .await;

    tracing::info!("server stopped. cleaning up...");

    pool.close().await;

    let shutdown_telemetry = tracer_provider.shutdown();

    shutdown_server?;
    shutdown_telemetry?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::parse_allowed_origins;

    #[test]
    fn empty_raw_yields_no_origins() {
        assert_eq!(
            parse_allowed_origins("").unwrap(),
            Vec::<HeaderValue>::new()
        );
    }

    #[test]
    fn entries_are_trimmed() {
        let origins =
            parse_allowed_origins(" http://localhost:5173 , http://localhost:3000 ").unwrap();
        assert_eq!(
            origins,
            vec![
                HeaderValue::from_static("http://localhost:5173"),
                HeaderValue::from_static("http://localhost:3000"),
            ]
        );
    }

    #[test]
    fn trailing_slash_is_rejected_with_the_origin_named() {
        let err = parse_allowed_origins("http://localhost:5173/").unwrap_err();
        assert!(err.to_string().contains("http://localhost:5173"));
    }

    #[test]
    fn invalid_header_value_is_rejected() {
        let err = parse_allowed_origins("http://bad\u{1}origin.com").unwrap_err();
        assert!(err.to_string().contains("invalid CORS origin"));
    }

    #[test]
    fn empty_entries_between_commas_are_dropped() {
        let origins =
            parse_allowed_origins("http://localhost:5173,,http://localhost:3000").unwrap();
        assert_eq!(
            origins,
            vec![
                HeaderValue::from_static("http://localhost:5173"),
                HeaderValue::from_static("http://localhost:3000"),
            ]
        );
    }
}
