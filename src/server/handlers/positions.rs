use axum::{
    extract::{Path, State},
    response::Json,
};
use tracing::{info, warn};

use crate::server::state::AppState;
use crate::server::types::{ApiResponse, Position, TradeHistory};

/// GET /api/positions
/// 現在保有中のポジション一覧を返却
pub async fn get_positions(State(state): State<AppState>) -> Json<ApiResponse<Vec<Position>>> {
    let lock = state.positions.read().await;
    Json(ApiResponse::ok(lock.clone()))
}

/// POST /api/positions/:id/close
/// 単一ポジションの手動成行決済
pub async fn close_position(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<TradeHistory>> {
    info!("Closing position id: {}", id);

    match state.close_position_now(&id).await {
        Ok(trade) => Json(ApiResponse::ok_msg(trade, "ポジションを成行決済しました")),
        Err(e) => {
            warn!("Failed to close position {id} at broker: {e:#}");
            Json(ApiResponse {
                success: false,
                data: None,
                message: Some(format!("ポジション {id} の決済に失敗しました: {e:#}")),
            })
        }
    }
}
