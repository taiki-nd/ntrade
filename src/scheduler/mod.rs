//! 常駐スケジューラ: 5分足確定ごとに判断サイクルを回し、ポジション同期と自己反省を行う。
//!
//! 環境変数:
//! - `NTRADE_SCHEDULER=off`      … ループを起動しない
//! - `NTRADE_PAIRS=USDJPY,EURUSD` … 対象ペア（既定 USDJPY）
//! - `NTRADE_BAR_DELAY_SECS=15`  … 足確定からデータ取得までの待ち秒数
//! - `NTRADE_LIVE_ORDERS=1`      … cTrader 接続時に実発注へ切り替え（既定はペーパー）

use chrono::{DateTime, Duration, TimeZone, Utc};
use tracing::{info, warn};

use crate::ctrader::{BarPeriod, CandleBar};
use crate::reflection;
use crate::server::handlers::decide::run_decision_cycle;
use crate::server::paper;
use crate::server::state::AppState;
use crate::server::types::{BotState, CloseReason, LessonLearned, TradeHistory};

pub const LESSON_ADOPT_THRESHOLD: usize = 3;
pub const LESSON_MAX_ACTIVE: usize = 10;

pub fn pairs_from_env() -> Vec<String> {
    std::env::var("NTRADE_PAIRS")
        .ok()
        .map(|s| s.split(',').map(|p| p.trim().to_uppercase()).filter(|p| !p.is_empty()).collect())
        .filter(|v: &Vec<String>| !v.is_empty())
        .unwrap_or_else(|| vec!["USDJPY".to_string()])
}

/// 次の 5M 境界 + 遅延
pub fn next_tick(now: DateTime<Utc>, delay_secs: i64) -> DateTime<Utc> {
    let step = 300;
    let next = (now.timestamp() / step + 1) * step;
    Utc.timestamp_opt(next, 0).single().unwrap_or(now) + Duration::seconds(delay_secs)
}

pub async fn run(state: AppState) {
    let pairs = pairs_from_env();
    let delay: i64 = std::env::var("NTRADE_BAR_DELAY_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(15);
    info!(?pairs, delay, "scheduler started (5M cadence)");

    loop {
        let now = Utc::now();
        let at = next_tick(now, delay);
        let wait = (at - now).to_std().unwrap_or_default();
        tokio::time::sleep(wait).await;

        let bot_state = *state.bot_state.read().await;
        if bot_state != BotState::Running {
            info!(?bot_state, "scheduler: bot not running, skipping cycle");
            continue;
        }
        let connected = state.ctrader_service.read().await.is_some();
        if !connected {
            warn!("scheduler: cTrader not connected, skipping cycle (no live bars)");
            continue;
        }

        for pair in &pairs {
            let _serial = state.decide_lock.lock().await;
            match run_decision_cycle(&state, pair).await {
                Ok(r) => info!(pair, action = ?r.decision.action, guard = r.guard.passed, executed = r.executed, "cycle ok"),
                Err(e) => warn!(pair, "cycle failed: {e:#}"),
            }
            drop(_serial);
            if let Err(e) = after_cycle(&state, pair).await {
                warn!(pair, "post-cycle sync failed: {e:#}");
            }
        }
    }
}

/// サイクル後: ペーパー決済 / ブローカー同期 → 決済トレードの自己反省
pub async fn after_cycle(state: &AppState, pair: &str) -> anyhow::Result<()> {
    let bundle = state.latest_snapshot.read().await.clone();
    let Some(bundle) = bundle else { return Ok(()) };
    let Some(row) = bundle.snapshot.bars_5m.last() else { return Ok(()) };
    let bar = CandleBar {
        timestamp: Utc.timestamp_opt(crate::storage::parse_ts(&row.t)?, 0).single().unwrap_or_else(Utc::now),
        open: row.o,
        high: row.h,
        low: row.l,
        close: row.c,
        volume: 0,
    };

    // 1. ペーパーポジションの決済判定
    let closed: Vec<TradeHistory> = {
        let mut positions = state.positions.write().await;
        let (open, closed) = paper::settle(std::mem::take(&mut *positions), pair, &bar);
        *positions = open;
        closed
    };
    if !closed.is_empty() {
        let mut trades = state.trades.write().await;
        let mut metrics = state.metrics.write().await;
        for t in &closed {
            metrics.balance += t.pnl_amount;
            metrics.daily_pnl += t.pnl_amount;
            metrics.daily_pnl_percent = if metrics.balance > 0.0 { metrics.daily_pnl / metrics.balance * 100.0 } else { 0.0 };
            metrics.total_trades_today += 1;
            if t.pnl_amount > 0.0 {
                metrics.winning_trades_today += 1;
            }
            metrics.win_rate_today = if metrics.total_trades_today > 0 {
                metrics.winning_trades_today as f64 / metrics.total_trades_today as f64 * 100.0
            } else {
                0.0
            };
            trades.insert(0, t.clone());
            info!(id = %t.id, ?t.close_reason, pnl_pips = t.pnl_pips, "paper position closed");
        }
    }

    // 2. ブローカー側ポジションとの同期（実発注時）
    if let Some(ctrader) = state.ctrader_service.read().await.clone() {
        if std::env::var("NTRADE_LIVE_ORDERS").is_ok() {
            match ctrader.get_open_positions().await {
                Ok(broker) => {
                    let mut positions = state.positions.write().await;
                    // ブローカーに無い実ポジションは決済済みとみなして除去
                    let before = positions.len();
                    positions.retain(|p| p.id.starts_with("paper-") || broker.iter().any(|b| b.position_id.to_string() == p.id));
                    if positions.len() != before {
                        info!("removed {} positions closed on broker side", before - positions.len());
                    }
                }
                Err(e) => warn!("reconcile failed: {e:#}"),
            }
        }
    }

    // 3. 自己反省（損切りのみ。判断ログとエントリー後の足を渡す）
    for t in closed.into_iter().filter(|t| matches!(t.close_reason, CloseReason::StopLoss)) {
        let state = state.clone();
        let post_bars: Vec<CandleBar> = {
            let db = state.ctrader_service.read().await.clone();
            match db {
                Some(c) => c.get_trendbars(&t.symbol, BarPeriod::M5, 24).await.unwrap_or_default(),
                None => Vec::new(),
            }
        };
        tokio::spawn(async move {
            let cot = t.cot_log_id.as_ref().and_then(|id| {
                futures_lite_block(state.cot_logs.clone(), id.clone())
            });
            match reflection::reflect(&state.llm, &t, cot.as_ref(), &post_bars).await {
                Ok(r) if !r.decision_was_sound && !r.lesson.trim().is_empty() => {
                    let mut lessons = state.lessons.write().await;
                    lessons.insert(
                        0,
                        LessonLearned {
                            id: format!("les-{}", Utc::now().timestamp_millis()),
                            created_at: Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                            symbol: t.symbol.clone(),
                            rule: r.lesson.clone(),
                            context: r.root_cause.clone(),
                            active: false,
                            trigger_trade_id: Some(t.id.clone()),
                            category: r.category.clone(),
                        },
                    );
                    if let Some(id) = reflection::maybe_adopt(&mut lessons, &t.symbol, &r.category, LESSON_ADOPT_THRESHOLD, LESSON_MAX_ACTIVE) {
                        info!(lesson = %id, category = %r.category, "lesson adopted (threshold reached)");
                    } else {
                        info!(category = %r.category, "lesson candidate stored (inactive)");
                    }
                }
                Ok(r) => info!(trade = %t.id, sound = r.decision_was_sound, "reflection: no lesson"),
                Err(e) => warn!(trade = %t.id, "reflection failed: {e:#}"),
            }
        });
    }
    Ok(())
}

/// CoT ログを ID で同期的に引く小さなヘルパー（spawn 内で使う）
fn futures_lite_block(
    logs: std::sync::Arc<tokio::sync::RwLock<Vec<crate::server::types::CoTLog>>>,
    id: String,
) -> Option<crate::server::types::CoTLog> {
    logs.try_read().ok().and_then(|l| l.iter().find(|c| c.id == id).cloned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_tick_aligns_to_five_minutes() {
        let now = Utc.with_ymd_and_hms(2026, 9, 15, 9, 3, 20).unwrap();
        assert_eq!(next_tick(now, 15), Utc.with_ymd_and_hms(2026, 9, 15, 9, 5, 15).unwrap());
        let on_boundary = Utc.with_ymd_and_hms(2026, 9, 15, 9, 5, 0).unwrap();
        assert_eq!(next_tick(on_boundary, 0), Utc.with_ymd_and_hms(2026, 9, 15, 9, 10, 0).unwrap());
    }
}
