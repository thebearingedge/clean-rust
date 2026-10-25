use std::net::SocketAddr;
use std::time::Duration;

use anyhow::Context;
use axum::serve;
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;

use telemetry::provider::init_tracer_provider;
use todos::MIGRATOR;
use web::{
    health::{self, HealthCheck},
    middleware::with_http_tracing,
    shutdown_signal,
};

use todos_api::{api::create_todos_api, app_state::AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let tracer_provider = init_tracer_provider("todos_api");

    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;

    tracing::info!("connecting to postgres...");

    let pool = PgPoolOptions::new()
        .min_connections(2)
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(3))
        .connect(&database_url)
        .await
        .context("connecting to postgres")?;

    tracing::info!("connected to postgres. running migrations...");

    MIGRATOR.run(&pool).await.context("running migrations")?;

    tracing::info!("migrations up to date");

    let router = with_http_tracing(create_todos_api(AppState::new(pool.clone()))).merge(
        health::router(vec![HealthCheck::with_resource(
            "postgres",
            pool.clone(),
            database::health::ping,
        )]),
    );

    let address: SocketAddr = std::env::var("LISTEN_ADDRESS")
        .unwrap_or("0.0.0.0:3000".to_string())
        .parse()
        .context("getting socket address")?;

    let listener = TcpListener::bind(&address)
        .await
        .context("binding listener")?;

    tracing::info!("listening at {address}");

    let shutdown_server = serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await;

    tracing::info!("server stopped. cleaning up...");

    pool.close().await;

    let shutdown_telemetry = tracer_provider.shutdown();

    shutdown_server?;
    shutdown_telemetry?;

    Ok(())
}
