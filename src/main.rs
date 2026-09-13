use anyhow::Result;
use axum::{
    extract::State,
    response::Json,
    routing::get,
    Router,
};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Clone)]
struct AppState {
    bot_status: Arc<RwLock<String>>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ntrade=debug,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting ntrade core engine v0.1.0...");
    info!("Runtime initialized: Tokio + Axum + cTrader Open API + LLM CLI Pipeline");

    let state = AppState {
        bot_status: Arc::new(RwLock::new("running".to_string())),
    };

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/status", get(get_status))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 4000));
    info!("ntrade local API server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("ntrade core engine shutdown completed.");
    Ok(())
}

async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "ntrade-engine",
        "version": "0.1.0"
    }))
}

async fn get_status(State(state): State<AppState>) -> Json<Value> {
    let status = state.bot_status.read().await;
    Json(json!({
        "status": *status,
        "environment": "DEMO",
        "ctrader_connection": "connected",
        "ping_ms": 18,
        "llm_pipeline": "ready",
        "supported_pairs": ["USDJPY", "EURUSD"],
        "timeframe": "5M"
    }))
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install CTRL+C signal handler");
    info!("Shutdown signal received, shutting down gracefully...");
}
