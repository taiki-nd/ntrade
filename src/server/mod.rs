pub mod handlers;
pub mod jobs;
pub mod state;
pub mod types;

use axum::{
    routing::{delete, get, post},
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

pub use state::AppState;

/// APIルーターの構築
pub fn create_router(state: AppState) -> Router {
    // CORS許可レイヤー（localhost:3000 からのフェッチを許可）
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // ヘルスチェック
        .route("/health", get(health_check))
        // 1. 全体ステータス & メトリクス
        .route("/api/status", get(handlers::status::get_status))
        .route("/api/balance-history", get(handlers::status::get_balance_history))
        // 2. ボット制御 (稼働/停止/緊急全決済)
        .route("/api/control/state", post(handlers::control::update_state))
        .route("/api/control/emergency-stop", post(handlers::control::emergency_stop))
        // 3. 保有ポジション (一覧/手動決済)
        .route("/api/positions", get(handlers::positions::get_positions))
        .route("/api/positions/{id}/close", post(handlers::positions::close_position))
        // 4. 約定・決済履歴
        .route("/api/trades", get(handlers::trades::get_trades))
        .route("/api/trades/delete", post(handlers::trades::delete_trades))
        .route("/api/trades/{id}", delete(handlers::trades::delete_trade))
        // 5. シグナル（戦略が出した操作の記録）と戦略
        .route("/api/signals", get(handlers::signals::list_signals))
        .route("/api/signals/{id}", get(handlers::signals::get_signal))
        .route("/api/strategies", get(handlers::strategies::list_strategies))
        .route("/api/deployments", get(handlers::strategies::list_deployments))
        // 6. チャート画像
        .route("/api/chart/latest", get(handlers::chart::get_latest_chart))
        // 7. バックテスト
        .route("/api/backtest/runs", get(handlers::backtest::list_runs))
        .route("/api/backtest/runs/{id}", get(handlers::backtest::run_detail).delete(handlers::backtest::delete_run))
        .route("/api/backtest/run", post(handlers::backtest::run))
        .route("/api/backtest/coverage", get(handlers::backtest::coverage))
        .route("/api/backtest/jobs", get(handlers::backtest::list_jobs).post(handlers::backtest::submit_job))
        .route("/api/backtest/jobs/{id}/cancel", post(handlers::backtest::cancel_job))
        // 9. 予約注文とリスク設定
        .route("/api/orders", get(handlers::orders::list_orders))
        .route("/api/orders/{id}", delete(handlers::orders::cancel_order))
        .route("/api/guard/config", get(handlers::orders::guard_config))
        // 10. 取引設定（対象ペア・待ち秒数・リスク設定・戦略）。SQLite に保存し再起動なしで反映
        .route("/api/settings", get(handlers::settings::get_settings).put(handlers::settings::update_settings))
        .route("/api/runtime", get(handlers::orders::runtime_info))
        // 11. 銘柄一覧（ブローカーの全銘柄。資産クラス付き）と銘柄仕様
        .route("/api/symbols", get(handlers::symbols::list_symbols))
        .route("/api/symbols/spec", get(handlers::symbols::symbol_spec))
        // 8. cTrader OAuth 認証連携
        .route("/api/auth/ctrader/url", get(handlers::auth::get_oauth_url))
        .route("/api/auth/ctrader/exchange", post(handlers::auth::exchange_oauth_code))
        .route("/api/auth/ctrader/accounts", get(handlers::auth::get_accounts))
        .route("/api/auth/ctrader/select-account", post(handlers::auth::select_account))
        // レイヤー適用
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health_check() -> axum::response::Json<serde_json::Value> {
    axum::response::Json(serde_json::json!({
        "status": "ok",
        "service": "ntrade-engine",
        "version": "0.1.0"
    }))
}
