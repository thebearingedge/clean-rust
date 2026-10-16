use std::net::SocketAddr;

use axum::serve;
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;

use telemetry::init_telemetry::get_tracer_provider;
use web::shutdown_signal;

use todos::MIGRATOR;
use todos_api::{api::create_todos_api, app_state::AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let collector = get_tracer_provider();

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

    tracing::info!("connected to postgres. running migrations...");

    MIGRATOR.run(&pool).await?;

    tracing::info!("migrations up to date");

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
    collector.shutdown()?;

    shutdown?;

    Ok(())
}
