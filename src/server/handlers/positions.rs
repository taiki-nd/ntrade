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
    let now_str = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

    // cTrader への実発注（接続時）
    let ctrader_opt = {
        let lock = state.ctrader_service.read().await;
        lock.clone()
    };
    if let Some(ref ctrader) = ctrader_opt {
        let is_buy_close = target.side != "BUY";
        let volume = crate::ctrader::lots_to_volume(target.volume_lots);
        let c_clone = ctrader.clone();
        let sym = target.symbol.clone();
        tokio::spawn(async move {
            if let Err(e) = c_clone.place_market_order_with_sltp(&sym, is_buy_close, volume, None, None).await {
                warn!("Failed to send close order to cTrader: {:?}", e);
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
        cot_log_id: None,
    };

    // 取引履歴に追加
    {
        let mut trades_lock = state.trades.write().await;
        trades_lock.insert(0, trade.clone());
    }

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
