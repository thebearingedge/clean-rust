mod common;
mod controllers;
mod delivery;
mod models;
mod repositories;
mod use_cases;

use std::net::SocketAddr;

use axum::serve;
use sqlx::postgres::PgPoolOptions;
use tokio::{net::TcpListener, signal};
use tower_http::trace::TraceLayer;

use crate::{
    common::telemetry::init_telemetry, controllers::app_state::AppState,
    delivery::todos_api::create_todos_api,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let telemetry = init_telemetry();

    tracing::info!("connecting to postgres...");

    let pool = PgPoolOptions::new()
        .min_connections(2)
        .max_connections(10)
        .connect(
            std::env::var("DATABASE_URL")
                .unwrap_or("postgres://postgres:password@localhost/postgres".to_string())
                .as_str(),
        )
        .await?;

    tracing::info!("connected to postgres");

    let router = create_todos_api(AppState::new(pool.clone())).layer(TraceLayer::new_for_http());

    let address: SocketAddr = std::env::var("LISTEN_ADDRESS")
        .unwrap_or("0.0.0.0:3000".to_string())
        .parse()?;

    let listener = TcpListener::bind(&address).await?;

    tracing::info!("listening at {}", address);

    let shutdown = serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await;

    tracing::info!("server stopped, cleaning up");

    pool.close().await;
    telemetry.shutdown()?;

    shutdown?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("shutdown signal received, draining connections...");
}
