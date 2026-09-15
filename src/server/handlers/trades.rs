use axum::{extract::State, response::Json};

use crate::server::state::AppState;
use crate::server::types::{ApiResponse, TradeHistory};

/// GET /api/trades
/// 約定・決済履歴一覧を返却
pub async fn get_trades(State(state): State<AppState>) -> Json<ApiResponse<Vec<TradeHistory>>> {
    let lock = state.trades.read().await;
    Json(ApiResponse::ok(lock.clone()))
}
