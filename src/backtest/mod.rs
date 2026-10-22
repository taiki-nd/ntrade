//! バックテスト: 本番と同じ戦略・同じリスクチェックを過去の足で動かし、口座の推移と成績を出す。
//!
//! 進め方（1 銘柄）:
//! 1. 執行用の足（1 分足があればそれ）を 1 本ずつ進める
//! 2. 戦略の時間足が確定した時刻になったら、その時点で確定している足だけを戦略に渡す（未来は見せない）
//! 3. 新規注文はリスクチェック（`guard::evaluate`）と発注量の計算を通し、シミュレーターに出す。
//!    成行は次の足の始値で約定する（本番でも足の確定を待ってから発注するので、その遅れを再現する）
//! 4. 予約注文の約定・SL/TP・スワップ・手数料はシミュレーター（`sim`）が判定する

pub mod config;
pub mod data;
pub mod fetch;
pub mod metrics;
pub mod optimize;
pub mod sim;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::ctrader::BarPeriod;
use crate::guard::{self, GuardConfig, GuardContext, MarketView};
use crate::strategy::{self, Action, Entry, OrderIntent, StrategyContext, StrategySpec};
pub use data::{Conversion, DataRequest, MarketData};
pub use metrics::Metrics;
pub use sim::{BacktestTrade, Costs, EquityPoint, ExitReason};
use sim::{SimBroker, SimOrder, SimOrderKind};

/// 1 回のバックテストの条件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BacktestConfig {
    pub pair: String,
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub strategy: StrategySpec,
    pub guard: GuardConfig,
    pub costs: Costs,
    pub initial_balance: f64,
    pub account_currency: String,
}

/// 戦略が出した操作の集計
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalStats {
    /// 新規注文の意図
    pub intents: usize,
    /// リスクチェックを通ってシミュレーターに出した新規注文
    pub placed: usize,
    /// リスクチェックで弾いた理由ごとの件数（1 件で複数のチェックに掛かることがある）
    pub rejected: BTreeMap<String, usize>,
    /// 弾いた新規注文の件数
    pub rejected_total: usize,
    pub closes: usize,
    pub cancels: usize,
    pub modifies: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BacktestResult {
    pub config: BacktestConfig,
    pub exec_period: BarPeriod,
    pub conversion: String,
    /// 戦略を呼んだ回数（確定した戦略の足の本数）
    pub decisions: usize,
    pub signals: SignalStats,
    pub metrics: Metrics,
    pub trades: Vec<BacktestTrade>,
    pub equity: Vec<EquityPoint>,
}

/// バックテストを実行する。`data` は `cfg` の期間を含んでいればよい（期間外の足は助走にだけ使う）
pub fn run(data: &MarketData, cfg: &BacktestConfig) -> Result<BacktestResult> {
    let mut strat = strategy::build(&cfg.strategy)?;
    let lookback = strat.lookback().max(1);
    let dur = data.strategy_period.duration();
    let sb = &data.strategy_bars;
    let mut broker = SimBroker::new(data.pip_size, data.lot_units, &cfg.costs, cfg.initial_balance);
    let mut stats = SignalStats::default();
    let mut equity: Vec<EquityPoint> = Vec::new();
    let mut decisions = 0;

    // 期間内に確定する最初の戦略の足
    let mut si = sb.partition_point(|b| b.timestamp + dur < cfg.from);
    let exec_start = data.exec_bars.partition_point(|b| b.timestamp < cfg.from);
    let exec_end = data.exec_bars.partition_point(|b| b.timestamp < cfg.to);
    let exec = &data.exec_bars[exec_start..exec_end];

    for bar in exec {
        // この足が始まる前に確定した戦略の足で判断する
        while si < sb.len() && sb[si].timestamp + dur <= bar.timestamp {
            let now = sb[si].timestamp + dur;
            if si + 1 >= lookback && now <= cfg.to {
                decide(&mut *strat, data, cfg, &mut broker, &mut stats, si + 1 - lookback..si + 1, now);
                decisions += 1;
                equity.push(EquityPoint { time: now, balance: broker.balance, equity: broker.equity() });
            }
            si += 1;
        }
        broker.process_bar(bar, data.conversion.rate_at(bar.timestamp));
    }
    if let Some(last) = exec.last() {
        let end = last.timestamp + data.exec_period.duration();
        broker.close_all(end);
        equity.push(EquityPoint { time: end, balance: broker.balance, equity: broker.balance });
    }

    let trades = std::mem::take(&mut broker.trades);
    let metrics = metrics::compute(&trades, &equity, cfg.initial_balance);
    Ok(BacktestResult {
        config: cfg.clone(),
        exec_period: data.exec_period,
        conversion: data.conversion.label(),
        decisions,
        signals: stats,
        metrics,
        trades,
        equity,
    })
}

fn decide(
    strat: &mut dyn strategy::Strategy,
    data: &MarketData,
    cfg: &BacktestConfig,
    broker: &mut SimBroker,
    stats: &mut SignalStats,
    window: std::ops::Range<usize>,
    now: DateTime<Utc>,
) {
    let bars = &data.strategy_bars[window];
    let positions = broker.position_views();
    let orders = broker.order_views();
    let ctx = StrategyContext { pair: &data.pair, now, bars, pip_size: data.pip_size, positions: &positions, orders: &orders };
    let actions = strat.on_bar(&ctx);
    for action in actions {
        match action {
            Action::Open(intent) => {
                stats.intents += 1;
                match review(&intent, data, cfg, broker, now, bars.last().map(|b| b.close).unwrap_or_default()) {
                    Ok(order) => {
                        stats.placed += 1;
                        broker.submit(order);
                    }
                    Err(failed) => {
                        stats.rejected_total += 1;
                        for f in failed {
                            *stats.rejected.entry(f).or_insert(0) += 1;
                        }
                    }
                }
            }
            Action::ClosePosition { id, reason } => {
                if broker.request_close(&id, &reason) {
                    stats.closes += 1;
                }
            }
            Action::CancelOrder { id, .. } => {
                if broker.cancel(&id) {
                    stats.cancels += 1;
                }
            }
            Action::ModifyPosition { id, stop_loss, take_profit, .. } => {
                if broker.modify(&id, stop_loss, take_profit) {
                    stats.modifies += 1;
                }
            }
        }
    }
}

/// リスクチェックと発注量の計算。通れば注文、だめなら失敗したチェック名
fn review(
    intent: &OrderIntent,
    data: &MarketData,
    cfg: &BacktestConfig,
    broker: &mut SimBroker,
    now: DateTime<Utc>,
    last_close: f64,
) -> std::result::Result<SimOrder, Vec<String>> {
    let spread = cfg.costs.spread_pips * data.pip_size;
    let market = MarketView {
        pair: &data.pair,
        now,
        price: last_close + spread / 2.0,
        spread_pips: Some(cfg.costs.spread_pips),
        pip_size: data.pip_size,
        atr_pips: data.atr_at(now),
    };
    let (positions, orders) = broker.exposure();
    let ctx = GuardContext {
        open_positions_pair: positions,
        open_positions_total: positions,
        working_orders_pair: orders,
        working_orders_total: orders,
        daily_pnl_pct: broker.daily_pnl_pct(now),
    };
    // 本番ではブローカーが価格を銘柄の桁数に丸めるので、同じく丸めてから見る
    let intent = &round_intent(intent, crate::instrument::price_digits(&data.pair));
    let verdict = guard::evaluate(intent, &market, &ctx, &cfg.guard);
    if !verdict.passed {
        return Err(verdict.failed);
    }
    let entry = guard::expected_entry(intent, &market);
    let sl_pips = (entry - intent.stop_loss).abs() / data.pip_size;
    let pip_value = data.pip_size * data.lot_units * data.conversion.rate_at(now);
    let lots = round_lots(cfg.guard.volume_lots(broker.balance, sl_pips, Some(pip_value)), data.lot_step);
    if lots + 1e-9 < data.min_lots {
        return Err(vec!["VOLUME_BELOW_MIN".into()]);
    }
    let kind = match intent.entry.pending() {
        Some((kind, price)) => SimOrderKind::Pending(kind, price),
        None => SimOrderKind::Market,
    };
    let expires = matches!(kind, SimOrderKind::Pending(..)).then(|| cfg.guard.order_expiry(now, intent.expiry_minutes));
    Ok(SimOrder {
        id: broker.order_id(),
        side: intent.side,
        kind,
        lots,
        stop_loss: intent.stop_loss,
        take_profit: intent.take_profit,
        created: now,
        expires,
        reason: intent.reason.clone(),
    })
}

fn round_intent(intent: &OrderIntent, digits: usize) -> OrderIntent {
    let scale = 10f64.powi(digits as i32);
    let r = |v: f64| (v * scale).round() / scale;
    let entry = match intent.entry {
        Entry::Market => Entry::Market,
        Entry::Limit { price } => Entry::Limit { price: r(price) },
        Entry::Stop { price } => Entry::Stop { price: r(price) },
    };
    OrderIntent { entry, stop_loss: r(intent.stop_loss), take_profit: intent.take_profit.map(r), ..intent.clone() }
}

/// 発注量を刻みに切り捨てる
fn round_lots(lots: f64, step: f64) -> f64 {
    if step <= 0.0 {
        return lots;
    }
    ((lots + 1e-9) / step).floor() * step
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctrader::CandleBar;
    use crate::guard::AtrTimeframe;
    use chrono::{Duration, TimeZone};
    use std::collections::HashMap;

    /// 上げ → 下げ → 上げの 5 分足（EMA クロスが何度か起きる）
    fn wave_bars(start: DateTime<Utc>, n: usize) -> Vec<CandleBar> {
        (0..n)
            .map(|i| {
                let x = i as f64;
                let c = 150.0 + (x / 40.0).sin() * 0.8 + x * 0.0005;
                CandleBar { timestamp: start + Duration::minutes(5 * i as i64), open: c - 0.01, high: c + 0.06, low: c - 0.06, close: c, volume: 1 }
            })
            .collect()
    }

    fn data(start: DateTime<Utc>, n: usize) -> MarketData {
        let bars = wave_bars(start, n);
        MarketData {
            pair: "USDJPY".into(),
            strategy_period: BarPeriod::M5,
            strategy_bars: bars.clone(),
            exec_period: BarPeriod::M5,
            exec_bars: bars,
            atr_bars: HashMap::new(),
            conversion: Conversion::Identity,
            pip_size: 0.01,
            lot_units: 100_000.0,
            min_lots: 0.01,
            lot_step: 0.01,
        }
    }

    fn config(start: DateTime<Utc>, n: usize) -> BacktestConfig {
        BacktestConfig {
            pair: "USDJPY".into(),
            from: start + Duration::minutes(5 * 300),
            to: start + Duration::minutes(5 * n as i64),
            strategy: StrategySpec {
                name: "ema_cross".into(),
                timeframe: BarPeriod::M5,
                params: [("fast".to_string(), 10.0), ("slow".to_string(), 30.0)].into(),
            },
            guard: GuardConfig { min_rr: 0.0, sl_atr_min: 0.0, sl_atr_max: 0.0, max_positions_total: 5, max_positions_per_pair: 5, ..GuardConfig::default() },
            costs: Costs { spread_pips: 0.2, slippage_pips: 0.0, commission_per_lot: 0.0, ..Costs::default() },
            initial_balance: 1_000_000.0,
            account_currency: "JPY".into(),
        }
    }

    #[test]
    fn runs_end_to_end_and_balances_add_up() {
        let start = Utc.with_ymd_and_hms(2026, 9, 1, 0, 0, 0).unwrap();
        let (d, cfg) = (data(start, 3000), config(start, 3000));
        let r = run(&d, &cfg).unwrap();
        assert!(r.metrics.trades > 3, "{:?}", r.signals);
        assert_eq!(r.signals.placed, r.metrics.trades);
        // 期間の前には建てない（助走の足では判断しない）
        assert!(r.trades.iter().all(|t| t.entry_time >= cfg.from));
        // 残高 = 初期資金 + 純損益の合計
        let net: f64 = r.trades.iter().map(|t| t.net).sum();
        assert!((r.metrics.final_balance - (cfg.initial_balance + net)).abs() < 1e-6);
        assert!((r.equity.last().unwrap().balance - r.metrics.final_balance).abs() < 1e-6);
        // 成行は判断した足の次の足の始値で約定する
        let t = &r.trades[0];
        let fill_bar = d.exec_bars.iter().find(|b| b.timestamp == t.entry_time).unwrap();
        let expected = if t.side == strategy::Side::Buy { fill_bar.open + 0.002 } else { fill_bar.open };
        assert!((t.entry_price - expected).abs() < 1e-9);
        assert_eq!(r.decisions, 2700);
    }

    #[test]
    fn identical_inputs_give_identical_results() {
        let start = Utc.with_ymd_and_hms(2026, 9, 1, 0, 0, 0).unwrap();
        let (d, cfg) = (data(start, 2000), config(start, 2000));
        let a = run(&d, &cfg).unwrap();
        let b = run(&d, &cfg).unwrap();
        assert_eq!(a.trades, b.trades);
    }

    #[test]
    fn guard_rejections_are_counted() {
        let start = Utc.with_ymd_and_hms(2026, 9, 1, 0, 0, 0).unwrap();
        let mut cfg = config(start, 2000);
        cfg.costs.spread_pips = 5.0; // 上限 1.0 pips を超える
        let r = run(&data(start, 2000), &cfg).unwrap();
        assert_eq!(r.metrics.trades, 0);
        assert!(r.signals.rejected["SPREAD_TOO_WIDE"] > 0);
        assert_eq!(r.signals.rejected_total, r.signals.intents);
    }

    #[test]
    fn atr_guard_uses_only_closed_bars() {
        let start = Utc.with_ymd_and_hms(2026, 9, 1, 0, 0, 0).unwrap();
        let mut d = data(start, 100);
        d.atr_bars.insert(AtrTimeframe::M5, d.strategy_bars.clone());
        let t = start + Duration::minutes(5 * 50);
        let before = d.atr_at(t);
        // t 以降の足を荒らしても、t 時点の ATR は変わらない
        for b in d.atr_bars.get_mut(&AtrTimeframe::M5).unwrap().iter_mut().filter(|b| b.timestamp >= t) {
            b.high += 10.0;
        }
        assert_eq!(d.atr_at(t), before);
    }

    #[test]
    fn strategy_bars_drive_decisions_while_minute_bars_decide_fills() {
        let start = Utc.with_ymd_and_hms(2026, 9, 1, 0, 0, 0).unwrap();
        let m1: Vec<CandleBar> = (0..5 * 3000)
            .map(|i| {
                let x = i as f64 / 5.0;
                let c = 150.0 + (x / 40.0).sin() * 0.8 + (i % 5) as f64 * 0.003;
                CandleBar { timestamp: start + Duration::minutes(i), open: c - 0.002, high: c + 0.02, low: c - 0.02, close: c, volume: 1 }
            })
            .collect();
        let mut d = data(start, 3000);
        d.strategy_bars = crate::indicators::resample(&m1, BarPeriod::M5);
        d.exec_period = BarPeriod::M1;
        d.exec_bars = m1.clone();
        let r = run(&d, &config(start, 3000)).unwrap();
        assert!(r.metrics.trades > 3);
        for t in r.trades.iter().filter(|t| t.entry_kind == "MARKET") {
            // 5 分足の確定直後の 1 分足の始値で約定する
            assert_eq!(t.entry_time.timestamp() % 300, 0, "{}", t.entry_time);
            let bar = m1.iter().find(|b| b.timestamp == t.entry_time).unwrap();
            let expected = if t.side == strategy::Side::Buy { bar.open + 0.002 } else { bar.open };
            assert!((t.entry_price - expected).abs() < 1e-9);
        }
        // SL・TP は 1 分足の時刻で決まる
        assert!(r.trades.iter().any(|t| t.exit_time.timestamp() % 300 != 0));
    }

    #[test]
    fn lots_are_rounded_down_to_the_step() {
        assert!((round_lots(0.237, 0.01) - 0.23).abs() < 1e-12);
        assert!((round_lots(0.3, 0.01) - 0.3).abs() < 1e-12);
    }
}
