use axum::{extract::State, response::Json};
use chrono::Utc;
use tracing::{info, warn};

use crate::server::state::AppState;
use crate::server::types::{ApiResponse, BotState, CloseReason, TradeHistory, UpdateBotStateRequest};

/// POST /api/control/state
/// ボットの稼働ステータス（running / paused）を更新
pub async fn update_state(
    State(state): State<AppState>,
    Json(payload): Json<UpdateBotStateRequest>,
) -> Json<ApiResponse<BotState>> {
    info!("Updating bot state to: {:?}", payload.state);

    let mut lock = state.bot_state.write().await;
    *lock = payload.state;

    Json(ApiResponse::ok_msg(payload.state, "ボット稼働状態を更新しました"))
}

/// POST /api/control/emergency-stop
/// 緊急全決済 & ボット停止
pub async fn emergency_stop(
    State(state): State<AppState>,
) -> Json<ApiResponse<String>> {
    info!("EXECUTING EMERGENCY STOP: Closing all open positions...");

    let mut positions_lock = state.positions.write().await;
    let mut trades_lock = state.trades.write().await;
    let mut metrics_lock = state.metrics.write().await;
    let mut bot_state_lock = state.bot_state.write().await;

    // ボット状態を停止に設定
    *bot_state_lock = BotState::Paused;

    if positions_lock.is_empty() {
        return Json(ApiResponse::ok("保有ポジションはありませんでした。ボットを一時停止しました。".to_string()));
    }

    let ctrader_opt = {
        let lock = state.ctrader_service.read().await;
        lock.clone()
    };

    let now_str = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let count = positions_lock.len();
    let mut total_pnl = 0.0;

    for p in positions_lock.drain(..) {
        total_pnl += p.pnl_amount;

        let next_idx = trades_lock.len();
        // cTrader 接続があれば実ブローカー決済リクエスト送信
        if let Some(ref ctrader) = ctrader_opt {
            if let Ok(_pos_id) = p.id.replace("pos-", "").parse::<i64>() {
                let volume = crate::ctrader::lots_to_volume(p.volume_lots);
                let sym_clone = p.symbol.clone();
                let is_buy = p.side != "BUY";
                let c_clone = ctrader.clone();
                tokio::spawn(async move {
                    if let Err(e) = c_clone.place_market_order_with_sltp(&sym_clone, is_buy, volume, None, None).await {
                        warn!("Failed to send cTrader close order: {:?}", e);
                    }
                });
            }
        }

        trades_lock.insert(
            0,
            TradeHistory {
                id: format!("trd-emerg-{}-{}", Utc::now().timestamp_millis(), next_idx),
                symbol: p.symbol,
                side: p.side,
                volume_lots: p.volume_lots,
                entry_price: p.entry_price,
                close_price: p.current_price,
                stop_loss: p.stop_loss,
                take_profit: p.take_profit,
                pnl_pips: p.pnl_pips,
                pnl_amount: p.pnl_amount,
                close_reason: CloseReason::Manual,
                open_time: p.open_time,
                close_time: now_str.clone(),
                cot_log_id: None,
            },
        );
    }

    // 口座メトリクスの更新
    metrics_lock.balance += total_pnl;
    metrics_lock.equity = metrics_lock.balance;
    metrics_lock.margin = 0.0;
    metrics_lock.free_margin = metrics_lock.balance;
    metrics_lock.unrealized_pnl = 0.0;
    metrics_lock.daily_pnl += total_pnl;
    metrics_lock.total_trades_today += count as u32;

    let msg = format!(
        "緊急全決済を実行しました: {} 件のポジションを成行決済し、自動売買を一時停止しました（確定損益: ¥{:.0}）",
        count, total_pnl
    );
    info!("{}", msg);

    Json(ApiResponse::ok(msg))
}
