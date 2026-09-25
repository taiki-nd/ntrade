pub mod handlers;
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
        // 5. LLM思考ログ (CoT)
        .route("/api/cot", get(handlers::cot::get_cot_logs))
        .route("/api/cot/{id}", get(handlers::cot::get_cot_detail))
        // 6. 自己反省ルール (教訓CRUD)
        .route("/api/lessons", get(handlers::lessons::get_lessons))
        .route("/api/lessons", post(handlers::lessons::create_lesson))
        .route("/api/lessons/{id}/toggle", post(handlers::lessons::toggle_lesson))
        .route("/api/lessons/{id}", delete(handlers::lessons::delete_lesson))
        // 7. Snapshot: 時間足別チャート画像配信 / 再生成 / 客観的事実JSON
        .route("/api/chart/latest", get(handlers::chart::get_latest_chart))
        .route("/api/chart/generate", post(handlers::chart::generate_chart))
        .route("/api/snapshot/latest", get(handlers::chart::get_latest_snapshot))
        // 7b. リプレイ結果
        .route("/api/replay/runs", get(handlers::replay::list_runs))
        .route("/api/replay/runs/{id}", get(handlers::replay::run_detail))
        .route("/api/replay/coverage", get(handlers::replay::coverage))
        // 9. 判断サイクル（Snapshot → LLM → ガード → 執行/プラン登録）と条件付きプラン
        .route("/api/decide", post(handlers::decide::decide_now))
        .route("/api/plans", get(handlers::plans::list_plans))
        .route("/api/plans/{id}", delete(handlers::plans::discard_plan))
        .route("/api/guard/config", get(handlers::plans::guard_config))
        .route("/api/runtime", get(handlers::plans::runtime_info))
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
