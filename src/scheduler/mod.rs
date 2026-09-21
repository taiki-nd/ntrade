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
use crate::server::state::AppState;
use crate::server::types::{BotState, CloseReason, LessonLearned, Position, TradeHistory};

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

/// サイクル後: ブローカー同期 → 決済トレードの自己反省
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

    let mut closed: Vec<TradeHistory> = Vec::new();

    // 1. ブローカー残高・ポジションとの同期
    state.sync_broker_account().await;
    if let Some(ctrader) = state.ctrader_service.read().await.clone() {
        match ctrader.get_open_positions().await {
            Ok(broker) => {
                let mut positions = state.positions.write().await;
                let mut remaining = Vec::new();
                for p in positions.drain(..) {
                    if broker.iter().any(|b| b.position_id.to_string() == p.id) {
                        remaining.push(p);
                    } else {
                        // ブローカー側で決済された
                        let is_buy = p.side == "BUY";
                        let sign = if is_buy { 1.0 } else { -1.0 };
                        let close_price = bar.close;
                        let pip = crate::snapshot::measures::get_pip_size(&p.symbol);
                        let pv = crate::snapshot::measures::pip_value_per_lot(&p.symbol);
                        let pnl_pips = ((close_price - p.entry_price) * sign / pip * 10.0).round() / 10.0;
                        let pnl_amount = (pnl_pips * pv * p.volume_lots).round();
                        let reason = if (is_buy && close_price <= p.stop_loss) || (!is_buy && close_price >= p.stop_loss) {
                            CloseReason::StopLoss
                        } else if (is_buy && close_price >= p.take_profit) || (!is_buy && close_price <= p.take_profit) {
                            CloseReason::TakeProfit
                        } else {
                            CloseReason::Manual
                        };
                        closed.push(TradeHistory {
                            id: format!("trd-{}", p.id),
                            symbol: p.symbol,
                            side: p.side,
                            volume_lots: p.volume_lots,
                            entry_price: p.entry_price,
                            close_price,
                            stop_loss: p.stop_loss,
                            take_profit: p.take_profit,
                            pnl_pips,
                            pnl_amount,
                            close_reason: reason,
                            open_time: p.open_time,
                            close_time: bar.timestamp.format("%Y-%m-%d %H:%M:%S").to_string(),
                            cot_log_id: p.cot_log_id,
                        });
                    }
                }
                // ブローカーにあってローカルに無い建玉（発注は通ったが記録に失敗した等）を取り込む
                let orphans: Vec<_> = broker
                    .iter()
                    .filter(|b| !remaining.iter().any(|p| p.id == b.position_id.to_string()))
                    .cloned()
                    .collect();
                for b in orphans {
                    let symbol = b.symbol_name.clone().unwrap_or_else(|| format!("symbol-{}", b.symbol_id));
                    warn!(
                        position_id = b.position_id,
                        symbol = %symbol,
                        "orphan broker position adopted (not tracked locally)"
                    );
                    remaining.push(Position {
                        id: b.position_id.to_string(),
                        symbol,
                        side: if b.is_buy { "BUY".into() } else { "SELL".into() },
                        volume_lots: b.volume_lots,
                        entry_price: b.entry_price.unwrap_or(bar.close),
                        current_price: bar.close,
                        stop_loss: b.stop_loss.unwrap_or_default(),
                        take_profit: b.take_profit.unwrap_or_default(),
                        pnl_pips: 0.0,
                        pnl_amount: 0.0,
                        open_time: b
                            .open_time
                            .unwrap_or(bar.timestamp)
                            .format("%Y-%m-%d %H:%M:%S")
                            .to_string(),
                        invalidation_reason: "ブローカー側から取り込んだ建玉（ntrade の記録に無し）".into(),
                        cot_log_id: None,
                    });
                }

                // 確定足終値で含み損益を更新
                let pip = crate::snapshot::measures::get_pip_size(pair);
                let pv = crate::snapshot::measures::pip_value_per_lot(pair);
                for p in remaining.iter_mut().filter(|p| p.symbol.eq_ignore_ascii_case(pair)) {
                    let is_buy = p.side == "BUY";
                    let sign = if is_buy { 1.0 } else { -1.0 };
                    p.current_price = bar.close;
                    p.pnl_pips = ((bar.close - p.entry_price) * sign / pip * 10.0).round() / 10.0;
                    p.pnl_amount = (p.pnl_pips * pv * p.volume_lots).round();
                }
                *positions = remaining;
                drop(positions);
                state.persist_positions().await;
            }
            Err(e) => warn!("reconcile failed: {e:#}"),
        }
    }

    if !closed.is_empty() {
        state.record_trades(closed.clone()).await;
        let mut metrics = state.metrics.write().await;
        for t in &closed {
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
            info!(id = %t.id, ?t.close_reason, pnl_pips = t.pnl_pips, "position closed on broker");
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
            let cot = match t.cot_log_id.clone() {
                Some(id) => state.with_db(move |db| db.cot_log(&id)).await.unwrap_or_else(|e| {
                    warn!(trade = %t.id, "failed to load CoT log for reflection: {e:#}");
                    None
                }),
                None => None,
            };
            match reflection::reflect(&state.llm, &t, cot.as_ref(), &post_bars).await {
                Ok(r) if !r.decision_was_sound && !r.lesson.trim().is_empty() => {
                    let lesson = LessonLearned {
                        id: format!("les-{}", Utc::now().timestamp_millis()),
                        created_at: Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                        symbol: t.symbol.clone(),
                        rule: r.lesson.clone(),
                        context: r.root_cause.clone(),
                        active: false,
                        trigger_trade_id: Some(t.id.clone()),
                        category: r.category.clone(),
                    };
                    let (symbol, category) = (t.symbol.clone(), r.category.clone());
                    let adopted = state
                        .with_db(move |db| {
                            db.upsert_lesson(&lesson)?;
                            let mut lessons = db.lessons()?;
                            let adopted = reflection::maybe_adopt(&mut lessons, &symbol, &category, LESSON_ADOPT_THRESHOLD, LESSON_MAX_ACTIVE);
                            if let Some(l) = adopted.as_ref().and_then(|id| lessons.iter().find(|l| &l.id == id)) {
                                db.upsert_lesson(l)?;
                            }
                            Ok(adopted)
                        })
                        .await;
                    match adopted {
                        Ok(Some(id)) => info!(lesson = %id, category = %r.category, "lesson adopted (threshold reached)"),
                        Ok(None) => info!(category = %r.category, "lesson candidate stored (inactive)"),
                        Err(e) => warn!(trade = %t.id, "failed to store lesson: {e:#}"),
                    }
                }
                Ok(r) => info!(trade = %t.id, sound = r.decision_was_sound, "reflection: no lesson"),
                Err(e) => warn!(trade = %t.id, "reflection failed: {e:#}"),
            }
        });
    }
    Ok(())
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
