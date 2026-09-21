use axum::{
    extract::{Path, State},
    response::Json,
};
use chrono::Utc;
use tracing::{info, warn};

use crate::server::state::AppState;
use crate::server::types::{ApiResponse, CloseReason, Position, TradeHistory};

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

    let mut positions_lock = state.positions.write().await;
    let index = match positions_lock.iter().position(|p| p.id == id) {
        Some(idx) => idx,
        None => {
            return Json(ApiResponse {
                success: false,
                data: None,
                message: Some(format!("Position {} not found", id)),
            });
        }
    };

    let target = positions_lock.remove(index);
    drop(positions_lock);
    state.persist_positions().await;
    let now_str = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

    // ブローカー側の決済（FIX 設定時は FIX 経由。保護注文の取り消しも行う）
    {
        let state = state.clone();
        let target = target.clone();
        tokio::spawn(async move {
            if let Err(e) = state.close_broker_position(&target).await {
                warn!("Failed to close position {} at broker: {e:#}", target.id);
            }
        });
    }

    let trade = TradeHistory {
        id: format!("trd-man-{}", Utc::now().timestamp_millis()),
        symbol: target.symbol,
        side: target.side,
        volume_lots: target.volume_lots,
        entry_price: target.entry_price,
        close_price: target.current_price,
        stop_loss: target.stop_loss,
        take_profit: target.take_profit,
        pnl_pips: target.pnl_pips,
        pnl_amount: target.pnl_amount,
        close_reason: CloseReason::Manual,
        open_time: target.open_time,
        close_time: now_str,
        cot_log_id: target.cot_log_id,
    };

    state.record_trades(vec![trade.clone()]).await;

    // 口座メトリクス更新
    {
        let mut metrics_lock = state.metrics.write().await;
        metrics_lock.balance += target.pnl_amount;
        metrics_lock.daily_pnl += target.pnl_amount;
        metrics_lock.total_trades_today += 1;
        if target.pnl_pips > 0.0 {
            metrics_lock.winning_trades_today += 1;
        }
        if metrics_lock.total_trades_today > 0 {
            metrics_lock.win_rate_today =
                (metrics_lock.winning_trades_today as f64 / metrics_lock.total_trades_today as f64) * 100.0;
        }
    }

    Json(ApiResponse::ok_msg(trade, "ポジションを成行決済しました"))
}
