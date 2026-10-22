//! 常駐スケジューラ: 1 分ごとに、足が確定したデプロイの戦略を並行に回し、終わったらブローカーと照合する。
//!
//! デプロイの時間足（5M なら 5 分ごと、1H なら毎時 0 分）の確定時刻 + 待ち秒数（`bar_delay_secs`）に回す。
//! ブローカーの取引時間外の銘柄は飛ばす（止まった足で判断させない）。
//!
//! デプロイ・待ち秒数は取引設定（`settings::TradingSettings`、画面から変更可）を毎回読み直すので、
//! 変更は再起動なしで次の足から反映される。
//!
//! 環境変数:
//! - `NTRADE_SCHEDULER=off`      … ループを起動しない

use chrono::{DateTime, Duration, TimeZone, Utc};
use tracing::{info, warn};

use crate::live::{self, DeploymentStatus};
use crate::server::state::AppState;
use crate::server::types::BotState;

/// 次の 1 分境界 + 遅延
pub fn next_tick(now: DateTime<Utc>, delay_secs: i64) -> DateTime<Utc> {
    let step = 60;
    let base = now - Duration::seconds(delay_secs);
    let next = (base.timestamp() / step + 1) * step;
    Utc.timestamp_opt(next, 0).single().unwrap_or(now) + Duration::seconds(delay_secs)
}

pub async fn run(state: AppState) {
    {
        let s = state.settings.read().await;
        info!(deployments = s.deployments.iter().filter(|d| d.enabled).count(), delay = s.bar_delay_secs, "scheduler started");
    }

    loop {
        let delay = state.settings.read().await.bar_delay_secs;
        let now = Utc::now();
        let at = next_tick(now, delay);
        tokio::time::sleep((at - now).to_std().unwrap_or_default()).await;

        let settings = state.settings.read().await.clone();
        // この分に足が確定するデプロイ
        let due: Vec<_> = settings
            .deployments
            .iter()
            .filter(|d| d.enabled)
            .filter_map(|d| live::bar_close_at(d.strategy.timeframe, at - Duration::seconds(delay)).map(|t| (d.clone(), t)))
            .collect();
        if due.is_empty() {
            continue;
        }
        let bot_state = *state.bot_state.read().await;
        if bot_state != BotState::Running {
            info!(?bot_state, "scheduler: bot not running, skipping {} deployment(s)", due.len());
            continue;
        }
        let Some(ctrader) = state.ctrader_service.read().await.clone() else {
            warn!("scheduler: cTrader not connected, skipping cycle (no live bars)");
            continue;
        };

        // デプロイごとに並行に回す。発注まわりの排他は live 側（order_lock）で取る
        let _gate = state.cycle_gate.read().await;
        let mut runs = tokio::task::JoinSet::new();
        for (dep, close_time) in due {
            match ctrader.market_status(&dep.pair, Utc::now()).await {
                Ok(m) if !m.open => {
                    tracing::debug!(deployment = %dep.id, pair = %dep.pair, "scheduler: market closed, skipping");
                    continue;
                }
                Ok(_) => {}
                // 仕様が引けないときは回す（足の取得で失敗すればそこで止まる）
                Err(e) => warn!(pair = %dep.pair, "scheduler: market status unknown: {e:#}"),
            }
            let state = state.clone();
            runs.spawn(async move {
                let result = live::run_deployment(&state, &dep, close_time).await;
                let mut status = DeploymentStatus {
                    id: dep.id.clone(),
                    last_run: Some(Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()),
                    last_bar: Some(close_time.format("%Y-%m-%d %H:%M:%S").to_string()),
                    ..Default::default()
                };
                match result {
                    Ok(n) => status.last_actions = n,
                    Err(e) => {
                        warn!(deployment = %dep.id, "strategy run failed: {e:#}");
                        status.last_error = Some(format!("{e:#}"));
                    }
                }
                state.deployment_status.write().await.insert(dep.id.clone(), status);
            });
        }
        while let Some(res) = runs.join_next().await {
            if let Err(e) = res {
                warn!("strategy task panicked: {e}");
            }
        }
        drop(_gate);

        if let Err(e) = state.reconcile_positions().await {
            warn!("post-cycle sync failed: {e:#}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_tick_aligns_to_the_minute_plus_delay() {
        let t = |h, m, s| Utc.with_ymd_and_hms(2026, 9, 15, h, m, s).unwrap();
        assert_eq!(next_tick(t(9, 3, 20), 15), t(9, 4, 15));
        assert_eq!(next_tick(t(9, 3, 10), 15), t(9, 3, 15));
        assert_eq!(next_tick(t(9, 3, 15), 15), t(9, 4, 15));
        assert_eq!(next_tick(t(9, 5, 0), 0), t(9, 6, 0));
    }
}
