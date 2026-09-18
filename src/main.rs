use anyhow::Result;
use std::net::SocketAddr;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use ntrade::server::{create_router, AppState};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ntrade=debug,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("============================================================");
    info!("  Starting ntrade core engine v0.1.0 (Step 5 Integrated)");
    info!("  Runtime: Tokio + Axum + cTrader Open API + LLM Engine");
    info!("============================================================");

    // アプリケーション状態の初期化
    let state = AppState::new();

    // バックグラウンドで cTrader への初期接続を試行
    let state_for_init = state.clone();
    tokio::spawn(async move {
        state_for_init.init_ctrader_connection().await;
    });

    // cTrader トークンの期限監視・自動更新（SQLite に保存）と切断時の再接続
    tokio::spawn(state.clone().run_token_refresh_loop());

    // 5分足確定ごとの常駐スケジューラ（NTRADE_SCHEDULER=off で無効化）
    if std::env::var("NTRADE_SCHEDULER").map(|v| v == "off").unwrap_or(false) {
        info!("Scheduler disabled (NTRADE_SCHEDULER=off)");
    } else {
        let state_for_loop = state.clone();
        tokio::spawn(async move {
            ntrade::scheduler::run(state_for_loop).await;
        });
    }

    // Axum API サーバーの起動
    let app = create_router(state);
    let addr = SocketAddr::from(([127, 0, 0, 1], 4000));
    info!("ntrade local API server listening on http://{}", addr);
    info!("Endpoints available:");
    info!("  - GET  /health");
    info!("  - GET  /api/status");
    info!("  - POST /api/control/state");
    info!("  - POST /api/control/emergency-stop");
    info!("  - GET  /api/positions");
    info!("  - POST /api/positions/:id/close");
    info!("  - GET  /api/trades");
    info!("  - GET  /api/cot");
    info!("  - GET  /api/lessons");
    info!("  - GET  /api/chart/latest?tf=4H|1H|15M|5M");
    info!("  - GET  /api/snapshot/latest");
    info!("  - POST /api/decide");
    info!("  - GET  /api/plans, DELETE /api/plans/:id");
    info!("  - GET  /api/guard/config");
    info!("  - GET  /api/replay/runs, /api/replay/runs/:id, /api/replay/coverage");
    info!("  - GET  /api/auth/ctrader/url");
    info!("  - POST /api/auth/ctrader/exchange");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("ntrade core engine shutdown completed.");
    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install CTRL+C signal handler");
    info!("Shutdown signal received, shutting down gracefully...");
}
