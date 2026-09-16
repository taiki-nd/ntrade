use axum::{extract::State, response::Json};
use tracing::debug;

use crate::server::state::AppState;
use crate::server::types::AccountMetrics;

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

    // 直近 Snapshot のスプレッドを反映
    if let Some(bundle) = state.latest_snapshot.read().await.as_ref() {
        let snap = &bundle.snapshot;
        match snap.pair.to_uppercase().as_str() {
            "USDJPY" => metrics.usdjpy_spread = snap.spread_pips,
            "EURUSD" => metrics.eurusd_spread = snap.spread_pips,
            _ => {}
        }
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
