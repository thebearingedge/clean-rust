use std::net::SocketAddr;
use std::time::Duration;

use anyhow::Context;
use axum::serve;
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use tower_http::trace::DefaultOnFailure;
use tower_http::trace::TraceLayer;

use telemetry::provider::get_tracer_provider;
use tracing::Level;
use web::{
    middleware::{HttpMakeSpan, HttpOnResponse},
    shutdown_signal,
};

use todos::MIGRATOR;
use todos_api::{api::create_todos_api, app_state::AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let tracer_provider = get_tracer_provider("todos_api");

    tracing::info!("connecting to postgres...");

    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;

    let pool = PgPoolOptions::new()
        .min_connections(2)
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(3))
        .connect(&database_url)
        .await?;

    tracing::info!("connected to postgres. running migrations...");

    MIGRATOR.run(&pool).await?;

    tracing::info!("migrations up to date");

    let router = create_todos_api(AppState::new(pool.clone())).layer(
        TraceLayer::new_for_http()
            .make_span_with(HttpMakeSpan)
            .on_request(())
            .on_response(HttpOnResponse)
            .on_failure(DefaultOnFailure::new().level(Level::ERROR)),
    );

    let address: SocketAddr = std::env::var("LISTEN_ADDRESS")
        .unwrap_or("0.0.0.0:3000".to_string())
        .parse()?;

    let listener = TcpListener::bind(&address).await?;

    tracing::info!("listening at {address}");

    let shutdown = serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await;

    tracing::info!("shutdown signal received, draining connections...");

    pool.close().await;
    tracer_provider.shutdown()?;

    shutdown?;

    Ok(())
}
