use axum::{
    extract::{Path, State},
    response::Json,
};

use crate::guard::GuardConfig;
use crate::server::state::AppState;
use crate::server::types::{ApiResponse, RuntimeInfo, WorkingOrder};

/// GET /api/orders … ブローカーに置いてある、まだ約定していない予約注文
pub async fn list_orders(State(state): State<AppState>) -> Json<ApiResponse<Vec<WorkingOrder>>> {
    Json(ApiResponse::ok(state.working_orders.read().await.clone()))
}

/// DELETE /api/orders/{id} … 予約注文をブローカーで取り消す
pub async fn cancel_order(State(state): State<AppState>, Path(id): Path<String>) -> Json<ApiResponse<String>> {
    if !state.working_orders.read().await.iter().any(|o| o.id == id) {
        return Json(ApiResponse::err(format!("order {id} not found")));
    }
    // 戦略の発注・操作と重ねない
    let _guard = state.order_lock.lock().await;
    let sink = state.order_sink.read().await.clone();
    match sink.cancel_pending(&id).await {
        Ok(()) => {
            state.working_orders.write().await.retain(|o| o.id != id);
            state.persist_working_orders().await;
            Json(ApiResponse::ok_msg(id, "予約注文を取り消しました"))
        }
        Err(e) => Json(ApiResponse::err(format!("{e:#}"))),
    }
}

/// GET /api/guard/config
pub async fn guard_config(State(state): State<AppState>) -> Json<ApiResponse<GuardConfig>> {
    Json(ApiResponse::ok(state.guard_config().await))
}

/// GET /api/runtime
/// エンジンの稼働設定（読み取り専用）
pub async fn runtime_info(State(state): State<AppState>) -> Json<ApiResponse<RuntimeInfo>> {
    let settings = state.settings.read().await.clone();
    let ctrader = state.ctrader_config.read().await.clone();
    let info = RuntimeInfo {
        order_mode: "live".into(),
        scheduler_enabled: !std::env::var("NTRADE_SCHEDULER").map(|v| v.trim_matches('"') == "off").unwrap_or(false),
        pairs: settings.pairs,
        bar_delay_secs: settings.bar_delay_secs,
        paper_balance: 0.0,
        guard_config_path: format!("SQLite app_settings.{}", crate::settings::SETTINGS_KEY),
        db_path: state.db_path.to_string_lossy().to_string(),
        env_file_present: std::path::Path::new(".env").exists(),
        ctrader_token_present: ctrader.as_ref().is_some_and(|c| !c.access_token.is_empty()),
        ctrader_refresh_token_present: ctrader.as_ref().is_some_and(|c| c.refresh_token.is_some()),
        ctrader_token_expires_at: ctrader.and_then(|c| c.token_expires_at).map(crate::storage::fmt_ts),
    };
    Json(ApiResponse::ok(info))
}
