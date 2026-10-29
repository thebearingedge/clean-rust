//! todos-api application binary.

use std::net::SocketAddr;
use std::time::Duration;

use anyhow::Context;
use axum::{Router, serve};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;

use telemetry::provider::init_tracer_provider;
use todos::MIGRATOR;
use todos_api::{api::create_todos_api, app_state::AppState};
use web::{
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

fn build_router(pool: &PgPool, draining: Draining) -> Router {
    with_http_tracing(create_todos_api(AppState::new(pool.clone()))).merge(health::router(
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

    let pool = build_pool(&database_url).await?;

    tracing::info!("connected to postgres. running migrations...");

    MIGRATOR.run(&pool).await.context("running migrations")?;

    tracing::info!("migrations up to date");

    let router = build_router(&pool, draining.clone());

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
