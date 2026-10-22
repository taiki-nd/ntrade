use axum::{
    extract::{Query, State},
    response::Json,
};
use tracing::debug;

use crate::server::state::AppState;
use crate::server::types::{AccountMetrics, ApiResponse, BalanceHistoryQuery, BalancePoint};

/// GET /api/balance-history?days=30
/// 口座残高の推移（古い順）。cTrader の決済・入出金履歴から取り込んだ点に、現在の残高を末尾に足す。
/// `days` を省略すると全期間
pub async fn get_balance_history(
    State(state): State<AppState>,
    Query(q): Query<BalanceHistoryQuery>,
) -> Json<ApiResponse<Vec<BalancePoint>>> {
    let Some(account_id) = state.ctrader_config.read().await.as_ref().map(|c| c.account_id) else {
        return Json(ApiResponse::ok(Vec::new()));
    };
    let from = q.days.map(|d| chrono::Utc::now().timestamp() - d * 86_400);
    match state.with_db(move |db| db.balance_history(account_id, from)).await {
        Ok(mut points) => {
            // 線を現在まで伸ばす（最後の決済から今までは残高が変わっていない）
            let m = state.metrics.read().await;
            if m.broker_balance.is_some() {
                points.push(BalancePoint {
                    time: crate::storage::fmt_ts(chrono::Utc::now().timestamp()),
                    balance: m.balance,
                });
            }
            Json(ApiResponse::ok(points))
        }
        Err(e) => Json(ApiResponse::err(format!("{e:#}"))),
    }
}

/// GET /api/status
/// ダッシュボード全体のメトリクス & 稼働ステータスを返却
pub async fn get_status(State(state): State<AppState>) -> Json<AccountMetrics> {
    debug!("Handling GET /api/status");

    // cTrader 接続時は 30 秒に 1 回ブローカー残高を同期
    state.sync_broker_account_if_stale(std::time::Duration::from_secs(30)).await;

    let ctrader_opt = {
        let lock = state.ctrader_service.read().await;
        lock.clone()
    };

    let mut metrics = {
        let lock = state.metrics.read().await;
        lock.clone()
    };

    // 保有ポジションの未実現損益を集計
    let positions = {
        let lock = state.positions.read().await;
        lock.clone()
    };
    let total_unrealized_pnl: f64 = positions.iter().map(|p| p.pnl_amount).sum();
    metrics.unrealized_pnl = total_unrealized_pnl;
    metrics.equity = metrics.balance + total_unrealized_pnl;
    metrics.free_margin = metrics.equity - metrics.margin;
    metrics.bot_state = *state.bot_state.read().await;

    // ペアごとの直近 Snapshot のスプレッドを反映
    for (pair, bundle) in state.latest_snapshots.read().await.iter() {
        match bundle.snapshot.spread_pips {
            Some(spread) => metrics.spreads.insert(pair.clone(), spread),
            None => metrics.spreads.remove(pair),
        };
    }

    if let Some(ctrader) = ctrader_opt {
        metrics.connection_status.ctrader = "connected".to_string();
        metrics.connection_status.environment = if ctrader.config().is_live {
            "LIVE".to_string()
        } else {
            "DEMO".to_string()
        };
        metrics.connection_status.account_number = format!(
            "cTrader #{} ({})",
            ctrader.config().account_id,
            if ctrader.config().is_live { "Live" } else { "Demo" }
        );
    }

    // メトリクスを最新値で更新
    {
        let mut lock = state.metrics.write().await;
        *lock = metrics.clone();
    }

    Json(metrics)
}
