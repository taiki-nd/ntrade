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
/// 緊急全決済 & 予約注文の全取消 & ボット停止
pub async fn emergency_stop(
    State(state): State<AppState>,
) -> Json<ApiResponse<String>> {
    info!("EXECUTING EMERGENCY STOP: Cancelling working orders and closing all open positions...");

    // 予約注文を残すと、停止後に約定して建玉ができてしまう。取り消せなかった注文は照合で残る
    let orders = state.working_orders.read().await.clone();
    let sink = state.order_sink.read().await.clone();
    let mut cancelled = 0;
    for o in &orders {
        match sink.cancel_pending(&o.id).await {
            Ok(()) => {
                cancelled += 1;
                state.working_orders.write().await.retain(|w| w.id != o.id);
            }
            Err(e) => warn!(order = %o.id, "Failed to cancel working order at broker: {e:#}"),
        }
    }
    state.persist_working_orders().await;
    let orders_note = if orders.is_empty() { String::new() } else { format!("予約注文 {cancelled}/{} 件を取り消し、", orders.len()) };

    let mut positions_lock = state.positions.write().await;
    let mut metrics_lock = state.metrics.write().await;
    let mut bot_state_lock = state.bot_state.write().await;

    // ボット状態を停止に設定
    *bot_state_lock = BotState::Paused;

    if positions_lock.is_empty() {
        return Json(ApiResponse::ok(format!("{orders_note}保有ポジションはありませんでした。ボットを一時停止しました。")));
    }

    let ctrader_opt = {
        let lock = state.ctrader_service.read().await;
        lock.clone()
    };

    let now_str = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let count = positions_lock.len();
    let mut total_pnl = 0.0;
    let mut closed = Vec::with_capacity(count);

    for p in positions_lock.drain(..) {
        total_pnl += p.pnl_amount;

        // ブローカー側の決済
        if ctrader_opt.is_some() {
            let state = state.clone();
            let position = p.clone();
            tokio::spawn(async move {
                if let Err(e) = state.close_broker_position(&position).await {
                    warn!("Failed to close position {} at broker: {e:#}", position.id);
                }
            });
        }

        closed.push(TradeHistory {
            // ID は建玉に対して一意にしておく（照合が実際の約定で同じ行を上書きできるように）
            id: format!("trd-{}", p.id),
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
            signal_id: p.signal_id,
            strategy: p.strategy,
        });
    }

    // 口座メトリクスの更新
    metrics_lock.balance += total_pnl;
    metrics_lock.equity = metrics_lock.balance;
    metrics_lock.margin = 0.0;
    metrics_lock.free_margin = metrics_lock.balance;
    metrics_lock.unrealized_pnl = 0.0;
    metrics_lock.daily_pnl += total_pnl;
    metrics_lock.total_trades_today += count as u32;
    drop((positions_lock, metrics_lock, bot_state_lock));
    state.persist_positions().await;
    state.record_trades(closed).await;

    let msg = format!(
        "緊急全決済を実行しました: {orders_note}{} 件のポジションを成行決済し、自動売買を一時停止しました（確定損益: ¥{:.0}）",
        count, total_pnl
    );
    info!("{}", msg);

    Json(ApiResponse::ok(msg))
}
