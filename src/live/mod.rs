//! 本番の戦略実行: 足が確定するたびに、デプロイした戦略に確定足を渡し、出てきた操作を執行する。
//!
//! バックテストと同じ戦略（`strategy::build`）と同じリスクチェック（`guard::evaluate`）を通す。
//! 違うのは、足を cTrader から取ること・現在値とスプレッドを気配値から取ること・口座の状態が実際の建玉であること。
//! 操作は 1 件ずつシグナルログ（`signals` テーブル）に残す。ドライランのデプロイは発注せず記録だけする。

use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use tracing::{info, warn};

use crate::ctrader::{BarPeriod, CandleBar, CTraderService};
use crate::executor::{signal_comment, MarketOrderRequest, PendingOrderRequest};
use crate::guard::{self, AtrSet, GuardConfig, GuardContext, MarketView};
use crate::indicators::closed_bars;
use crate::server::state::AppState;
use crate::server::types::{CloseReason, Position, SignalLog, SignalStatus, WorkingOrder};
use crate::settings::Deployment;
use crate::strategy::{self, Action, OrderIntent, OrderView, PositionView, Side, StrategyContext};

/// デプロイごとの直近の実行状況（画面表示用。再起動で消える）
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentStatus {
    pub id: String,
    /// 直近に判断した時刻
    pub last_run: Option<String>,
    /// 直近に判断に使った足の確定時刻
    pub last_bar: Option<String>,
    /// 直近の判断で出た操作の数
    pub last_actions: usize,
    pub last_error: Option<String>,
}

/// `tick`（分に切り捨て）が、この時間足の足が確定する時刻か（UTC 0:00 起点で区切る）
pub fn bar_close_at(period: BarPeriod, tick: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let minute = tick.timestamp().div_euclid(60) * 60;
    let step = period.duration().num_seconds();
    (minute % step == 0).then(|| DateTime::from_timestamp(minute, 0)).flatten()
}

fn ts_string(t: DateTime<Utc>) -> String {
    t.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn parse_time(s: &str) -> Option<DateTime<Utc>> {
    crate::storage::parse_ts(s).ok().and_then(|t| DateTime::from_timestamp(t, 0))
}

/// 照合で持っている建玉を戦略から見た形にする
fn position_view(p: &Position) -> Option<PositionView> {
    Some(PositionView {
        id: p.id.clone(),
        side: Side::parse(&p.side)?,
        volume_lots: p.volume_lots,
        entry_price: p.entry_price,
        stop_loss: Some(p.stop_loss).filter(|v| *v > 0.0),
        take_profit: Some(p.take_profit).filter(|v| *v > 0.0),
        open_time: parse_time(&p.open_time).unwrap_or_else(Utc::now),
    })
}

fn order_view(o: &WorkingOrder) -> Option<OrderView> {
    Some(OrderView {
        id: o.id.clone(),
        side: Side::parse(&o.side)?,
        kind: o.kind,
        price: o.entry_price,
        stop_loss: Some(o.stop_loss).filter(|v| *v > 0.0),
        take_profit: Some(o.take_profit).filter(|v| *v > 0.0),
        created_at: parse_time(&o.created_at).unwrap_or_else(Utc::now),
        expires_at: parse_time(&o.expires_at),
    })
}

/// 1 デプロイ分の判断。`close_time` に確定した足までを戦略に渡す。戻り値は出た操作の数
pub async fn run_deployment(state: &AppState, dep: &Deployment, close_time: DateTime<Utc>) -> Result<usize> {
    let ctrader = state.ctrader_service.read().await.clone().ok_or_else(|| anyhow!("cTrader is not connected"))?;
    let mut strat = strategy::build(&dep.strategy)?;
    let lookback = strat.lookback().max(1);
    let period = dep.strategy.timeframe;

    let bars = ctrader
        .get_trendbars(&dep.pair, period, (lookback + 2) as u32)
        .await
        .with_context(|| format!("failed to fetch {} bars", period.as_str()))?;
    let mut bars = closed_bars(bars, period, Utc::now());
    let Some(last) = bars.last() else { bail!("no closed {} bars", period.as_str()) };
    let last_close = last.timestamp + period.duration();
    if last_close != close_time {
        // 休場中・足の配信遅れ。止まった足で判断しない
        bail!("latest closed bar ends at {last_close}, expected {close_time} (market idle or bar not published yet)");
    }
    if bars.len() < lookback {
        bail!("only {} closed bars; the strategy needs {lookback}", bars.len());
    }
    let bars = bars.split_off(bars.len() - lookback);

    let (positions, orders): (Vec<PositionView>, Vec<OrderView>) = {
        let mine = |s: &Option<String>, sym: &str| s.as_deref() == Some(dep.id.as_str()) && sym.eq_ignore_ascii_case(&dep.pair);
        let p = state.positions.read().await.iter().filter(|p| mine(&p.strategy, &p.symbol)).filter_map(position_view).collect();
        let o = state.working_orders.read().await.iter().filter(|o| mine(&o.strategy, &o.symbol)).filter_map(order_view).collect();
        (p, o)
    };
    let pip_size = crate::instrument::pip_size(&dep.pair);
    let ctx = StrategyContext { pair: &dep.pair, now: close_time, bars: &bars, pip_size, positions: &positions, orders: &orders };
    let actions = strat.on_bar(&ctx);
    if actions.is_empty() {
        return Ok(0);
    }
    info!(deployment = %dep.id, pair = %dep.pair, actions = actions.len(), dry_run = dep.dry_run, "strategy signalled");

    // 発注・操作は他のデプロイと重ねない（建玉数のチェックと発注の間に別の発注が入らないように）
    let _order_guard = state.order_lock.lock().await;
    let cfg = state.guard_config().await;
    let mut atr: Option<AtrSet> = None;
    let base = Utc::now().timestamp_millis();
    for (i, action) in actions.iter().enumerate() {
        let signal_id = format!("sig-{base}-{i}");
        let mut log = SignalLog {
            id: signal_id.clone(),
            timestamp: ts_string(Utc::now()),
            deployment: dep.id.clone(),
            strategy: dep.strategy.label(),
            symbol: dep.pair.clone(),
            action: action.kind().to_string(),
            side: None,
            reason: action.reason().to_string(),
            detail: serde_json::json!({ "action": action, "barClose": ts_string(close_time) }),
            guard_result: None,
            status: SignalStatus::DryRun,
            message: None,
            order_id: None,
            dry_run: dep.dry_run,
        };
        let result = match action {
            Action::Open(intent) => {
                if atr.is_none() {
                    atr = Some(atr_set(&ctrader, &dep.pair, &cfg, pip_size).await);
                }
                open(state, &ctrader, dep, intent, &cfg, atr.clone().unwrap_or_default(), &bars, &signal_id, &mut log).await
            }
            Action::ClosePosition { id, .. } => {
                log.order_id = Some(id.clone());
                if dep.dry_run {
                    Ok(SignalStatus::DryRun)
                } else {
                    state.close_position_as(id, Some(CloseReason::Strategy)).await.map(|t| {
                        log.message = Some(format!("closed at {} ({:+.1} pips, {:+.0})", t.close_price, t.pnl_pips, t.pnl_amount));
                        SignalStatus::Executed
                    })
                }
            }
            Action::CancelOrder { id, .. } => {
                log.order_id = Some(id.clone());
                if dep.dry_run {
                    Ok(SignalStatus::DryRun)
                } else {
                    cancel(state, id).await.map(|_| SignalStatus::Executed)
                }
            }
            Action::ModifyPosition { id, stop_loss, take_profit, .. } => {
                log.order_id = Some(id.clone());
                if dep.dry_run {
                    Ok(SignalStatus::DryRun)
                } else {
                    modify(state, id, *stop_loss, *take_profit).await.map(|_| SignalStatus::Executed)
                }
            }
        };
        log.status = match result {
            Ok(s) => s,
            Err(e) => {
                warn!(deployment = %dep.id, signal = %signal_id, "signal failed: {e:#}");
                log.message = Some(format!("{e:#}"));
                SignalStatus::Failed
            }
        };
        info!(deployment = %dep.id, signal = %signal_id, action = %log.action, status = log.status.as_str(), guard = ?log.guard_result, "signal processed");
        state.record_signal(log).await;
    }
    Ok(actions.len())
}

/// リスクチェックに使う ATR（確定足から）
async fn atr_set(ctrader: &CTraderService, pair: &str, cfg: &GuardConfig, pip_size: f64) -> AtrSet {
    let mut set = AtrSet::default();
    for tf in cfg.atr_timeframes() {
        match ctrader.get_trendbars(pair, tf.period(), 40).await {
            Ok(bars) => set.insert_from_bars(tf, &closed_bars(bars, tf.period(), Utc::now()), pip_size),
            Err(e) => warn!(pair, tf = tf.label(), "failed to fetch bars for ATR: {e:#}"),
        }
    }
    set
}

/// 新規注文: リスクチェック → 発注量 → 証拠金 → 発注
#[allow(clippy::too_many_arguments)]
async fn open(
    state: &AppState,
    ctrader: &CTraderService,
    dep: &Deployment,
    intent: &OrderIntent,
    cfg: &GuardConfig,
    atr: AtrSet,
    bars: &[CandleBar],
    signal_id: &str,
    log: &mut SignalLog,
) -> Result<SignalStatus> {
    log.side = Some(intent.side.as_str().to_string());
    let pip_size = crate::instrument::pip_size(&dep.pair);
    let quote = ctrader.quote(&dep.pair, Duration::seconds(30)).await;
    let price = quote.map(|q| (q.bid + q.ask) / 2.0).or(bars.last().map(|b| b.close)).unwrap_or_default();
    let market = MarketView {
        pair: &dep.pair,
        now: Utc::now(),
        price,
        spread_pips: quote.map(|q| q.spread() / pip_size),
        pip_size,
        atr_pips: atr,
    };
    let ctx = state.guard_context(&dep.pair).await;
    let verdict = guard::evaluate(intent, &market, &ctx, cfg);
    log.guard_result = Some(verdict.summary());
    log.detail["market"] = serde_json::json!({ "price": price, "spreadPips": market.spread_pips, "atrPips": market.atr_pips });
    if !verdict.passed {
        log.message = Some(verdict.failure_details());
        return Ok(SignalStatus::Rejected);
    }
    let entry = guard::expected_entry(intent, &market);
    let sl_pips = (entry - intent.stop_loss).abs() / pip_size;
    let lots = state.order_volume(cfg, &dep.pair, sl_pips).await;
    log.detail["volumeLots"] = serde_json::json!(lots);
    if let Err(reason) = state.margin_check(cfg, &dep.pair, intent.side, lots, entry).await {
        log.message = Some(reason);
        return Ok(SignalStatus::Rejected);
    }
    if dep.dry_run {
        return Ok(SignalStatus::DryRun);
    }

    let comment = signal_comment(signal_id, &dep.id);
    let sink = state.order_sink.read().await.clone();
    let now = Utc::now();
    match intent.entry.pending() {
        None => {
            let reference = match (intent.side, quote) {
                (Side::Buy, Some(q)) => q.ask,
                (Side::Sell, Some(q)) => q.bid,
                _ => price,
            };
            let req = MarketOrderRequest {
                pair: dep.pair.clone(),
                side: intent.side,
                volume_lots: lots,
                reference_price: reference,
                stop_loss: intent.stop_loss,
                take_profit: intent.take_profit,
                comment,
            };
            let fill = sink.place_market(&req).await?;
            log.order_id = Some(fill.position_id.clone());
            log.message = Some(format!("filled {} lot at {}", lots, fill.price));
            state.positions.write().await.push(Position {
                id: fill.position_id,
                symbol: dep.pair.clone(),
                side: intent.side.as_str().to_string(),
                volume_lots: lots,
                entry_price: fill.price,
                current_price: fill.price,
                stop_loss: intent.stop_loss,
                take_profit: intent.take_profit.unwrap_or(0.0),
                pnl_pips: 0.0,
                pnl_amount: 0.0,
                open_time: ts_string(now),
                note: intent.reason.clone(),
                signal_id: Some(signal_id.to_string()),
                strategy: Some(dep.id.clone()),
            });
            state.persist_positions().await;
        }
        Some((kind, price)) => {
            let expires_at = cfg.order_expiry(now, intent.expiry_minutes);
            let req = PendingOrderRequest {
                pair: dep.pair.clone(),
                side: intent.side,
                kind,
                volume_lots: lots,
                entry_price: price,
                stop_loss: intent.stop_loss,
                take_profit: intent.take_profit,
                expires_at,
                comment,
            };
            let id = sink.place_pending(&req).await?;
            log.order_id = Some(id.clone());
            state.working_orders.write().await.push(WorkingOrder {
                id,
                symbol: dep.pair.clone(),
                side: intent.side.as_str().to_string(),
                kind,
                volume_lots: lots,
                entry_price: price,
                stop_loss: intent.stop_loss,
                take_profit: intent.take_profit.unwrap_or(0.0),
                created_at: ts_string(now),
                expires_at: ts_string(expires_at),
                note: intent.reason.clone(),
                signal_id: Some(signal_id.to_string()),
                strategy: Some(dep.id.clone()),
            });
            state.persist_working_orders().await;
        }
    }
    Ok(SignalStatus::Executed)
}

async fn cancel(state: &AppState, id: &str) -> Result<()> {
    let sink = state.order_sink.read().await.clone();
    sink.cancel_pending(id).await?;
    state.working_orders.write().await.retain(|o| o.id != id);
    state.persist_working_orders().await;
    Ok(())
}

async fn modify(state: &AppState, id: &str, stop_loss: Option<f64>, take_profit: Option<f64>) -> Result<()> {
    let current = state.positions.read().await.iter().find(|p| p.id == id).cloned().ok_or_else(|| anyhow!("position {id} not found"))?;
    let sink = state.order_sink.read().await.clone();
    // 変えない項目も今の値で送る（省くと 0 とみなすブローカーがある）
    let keep = |v: f64| Some(v).filter(|v| *v > 0.0);
    sink.amend_position(id, stop_loss.or(keep(current.stop_loss)), take_profit.or(keep(current.take_profit))).await?;
    if let Some(p) = state.positions.write().await.iter_mut().find(|p| p.id == id) {
        if let Some(sl) = stop_loss {
            p.stop_loss = sl;
        }
        if let Some(tp) = take_profit {
            p.take_profit = tp;
        }
    }
    state.persist_positions().await;
    Ok(())
}

impl AppState {
    /// リスクチェック用の口座の状態（全デプロイ・手動の建玉を含む）
    pub async fn guard_context(&self, pair: &str) -> GuardContext {
        let (pos_pair, pos_total) = {
            let p = self.positions.read().await;
            (p.iter().filter(|p| p.symbol.eq_ignore_ascii_case(pair)).count(), p.len())
        };
        let (ord_pair, ord_total) = {
            let o = self.working_orders.read().await;
            (o.iter().filter(|o| o.symbol.eq_ignore_ascii_case(pair)).count(), o.len())
        };
        GuardContext {
            open_positions_pair: pos_pair,
            open_positions_total: pos_total,
            working_orders_pair: ord_pair,
            working_orders_total: ord_total,
            daily_pnl_pct: self.daily_realized().await.1,
        }
    }

    /// 当日（UTC）の確定損益（金額, その日の始めの残高に対する %）
    pub async fn daily_realized(&self) -> (f64, f64) {
        let since = ts_string(Utc::now().date_naive().and_hms_opt(0, 0, 0).map(|t| t.and_utc()).unwrap_or_else(Utc::now));
        let amount = self.with_db(move |db| db.realized_pnl_since(&since)).await.unwrap_or_else(|e| {
            warn!("failed to sum today's realized PnL: {e:#}");
            0.0
        });
        let balance = self.metrics.read().await.balance;
        let start = balance - amount;
        (amount, if start > 0.0 { amount / start * 100.0 } else { 0.0 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn bar_closes_align_to_utc_midnight() {
        let t = |h, m, s| Utc.with_ymd_and_hms(2026, 10, 20, h, m, s).unwrap();
        assert_eq!(bar_close_at(BarPeriod::M5, t(9, 5, 15)), Some(t(9, 5, 0)));
        assert_eq!(bar_close_at(BarPeriod::M5, t(9, 7, 15)), None);
        assert_eq!(bar_close_at(BarPeriod::H1, t(10, 0, 20)), Some(t(10, 0, 0)));
        assert_eq!(bar_close_at(BarPeriod::H1, t(10, 5, 0)), None);
        assert_eq!(bar_close_at(BarPeriod::H4, t(8, 0, 10)), Some(t(8, 0, 0)));
        assert_eq!(bar_close_at(BarPeriod::H4, t(9, 0, 10)), None);
        assert_eq!(bar_close_at(BarPeriod::M1, t(9, 7, 15)), Some(t(9, 7, 0)));
    }

    #[test]
    fn only_tagged_values_become_strategy_views() {
        let p = Position {
            id: "1".into(),
            symbol: "USDJPY_z".into(),
            side: "SELL".into(),
            volume_lots: 0.1,
            entry_price: 150.0,
            current_price: 150.0,
            stop_loss: 150.5,
            take_profit: 0.0,
            pnl_pips: 0.0,
            pnl_amount: 0.0,
            open_time: "2026-10-20 09:00:00".into(),
            note: String::new(),
            signal_id: None,
            strategy: Some("d".into()),
        };
        let v = position_view(&p).unwrap();
        assert_eq!((v.side, v.stop_loss, v.take_profit), (Side::Sell, Some(150.5), None));
        assert!(position_view(&Position { side: "?".into(), ..p }).is_none());
    }
}
