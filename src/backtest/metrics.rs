//! 成績指標: 取引と資産の推移から、勝率・PF・最大ドローダウン・シャープレシオなどを求める。

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::sim::{BacktestTrade, EquityPoint};
use crate::strategy::Side;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonthlyStat {
    /// "YYYY-MM"
    pub month: String,
    pub trades: usize,
    pub net: f64,
    pub pips: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metrics {
    pub initial_balance: f64,
    pub final_balance: f64,
    pub trades: usize,
    pub wins: usize,
    pub losses: usize,
    /// 勝率（%）。勝ち = 純損益が正
    pub win_rate: f64,
    pub net_profit: f64,
    pub gross_profit: f64,
    pub gross_loss: f64,
    /// 総利益 / 総損失。損失が無ければ None
    pub profit_factor: Option<f64>,
    /// 1 取引あたりの平均純損益
    pub expectancy: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    /// 平均利益 / 平均損失
    pub payoff_ratio: Option<f64>,
    pub total_pips: f64,
    pub avg_pips: f64,
    /// 最大ドローダウン（口座通貨・含み損益込みの資産で測る）
    pub max_drawdown: f64,
    /// 最大ドローダウン（その時点の資産の最高値に対する %）
    pub max_drawdown_pct: f64,
    /// 純損益 / 初期資金（%）
    pub return_pct: f64,
    /// 日次リターンのシャープレシオ（年率換算、252 日）。2 日分以上の資産推移が無ければ None
    pub sharpe: Option<f64>,
    /// 純損益 / 最大ドローダウン
    pub recovery_factor: Option<f64>,
    pub max_consecutive_wins: usize,
    pub max_consecutive_losses: usize,
    pub avg_hold_minutes: f64,
    /// 手数料の合計（支払い = 正）
    pub commission: f64,
    /// スワップの合計（受け取り = 正）
    pub swap: f64,
    pub long_trades: usize,
    pub long_win_rate: f64,
    pub short_trades: usize,
    pub short_win_rate: f64,
    /// 決済理由ごとの件数
    pub exit_reasons: BTreeMap<String, usize>,
    pub monthly: Vec<MonthlyStat>,
}

fn pct(n: usize, d: usize) -> f64 {
    if d == 0 {
        0.0
    } else {
        n as f64 / d as f64 * 100.0
    }
}

/// 取引と資産の推移から指標を求める。`equity` が空なら取引の決済時点で資産を組み立てる
pub fn compute(trades: &[BacktestTrade], equity: &[EquityPoint], initial_balance: f64) -> Metrics {
    let built;
    let equity = if equity.is_empty() {
        built = equity_from_trades(trades, initial_balance);
        &built[..]
    } else {
        equity
    };

    let n = trades.len();
    let wins: Vec<&BacktestTrade> = trades.iter().filter(|t| t.net > 0.0).collect();
    let losses: Vec<&BacktestTrade> = trades.iter().filter(|t| t.net <= 0.0).collect();
    let gross_profit: f64 = wins.iter().map(|t| t.net).sum();
    let gross_loss: f64 = -losses.iter().map(|t| t.net).sum::<f64>();
    let net_profit = gross_profit - gross_loss;
    let avg_win = if wins.is_empty() { 0.0 } else { gross_profit / wins.len() as f64 };
    let avg_loss = if losses.is_empty() { 0.0 } else { gross_loss / losses.len() as f64 };
    let total_pips: f64 = trades.iter().map(|t| t.pnl_pips).sum();

    let (mut streak_w, mut streak_l, mut max_w, mut max_l) = (0, 0, 0, 0);
    for t in trades {
        if t.net > 0.0 {
            streak_w += 1;
            streak_l = 0;
        } else {
            streak_l += 1;
            streak_w = 0;
        }
        max_w = max_w.max(streak_w);
        max_l = max_l.max(streak_l);
    }

    let (max_dd, max_dd_pct) = drawdown(equity);
    let side_stats = |side: Side| {
        let all: Vec<&BacktestTrade> = trades.iter().filter(|t| t.side == side).collect();
        let w = all.iter().filter(|t| t.net > 0.0).count();
        (all.len(), pct(w, all.len()))
    };
    let (long_trades, long_win_rate) = side_stats(Side::Buy);
    let (short_trades, short_win_rate) = side_stats(Side::Sell);

    let mut exit_reasons = BTreeMap::new();
    for t in trades {
        *exit_reasons.entry(t.exit_reason.as_str().to_string()).or_insert(0) += 1;
    }

    let mut monthly: BTreeMap<String, MonthlyStat> = BTreeMap::new();
    for t in trades {
        let key = format!("{:04}-{:02}", t.exit_time.year(), t.exit_time.month());
        let m = monthly.entry(key.clone()).or_insert_with(|| MonthlyStat { month: key, ..Default::default() });
        m.trades += 1;
        m.net += t.net;
        m.pips += t.pnl_pips;
    }

    Metrics {
        initial_balance,
        final_balance: initial_balance + net_profit,
        trades: n,
        wins: wins.len(),
        losses: losses.len(),
        win_rate: pct(wins.len(), n),
        net_profit,
        gross_profit,
        gross_loss,
        profit_factor: (gross_loss > 0.0).then(|| gross_profit / gross_loss),
        expectancy: if n == 0 { 0.0 } else { net_profit / n as f64 },
        avg_win,
        avg_loss,
        payoff_ratio: (avg_loss > 0.0 && !wins.is_empty()).then(|| avg_win / avg_loss),
        total_pips,
        avg_pips: if n == 0 { 0.0 } else { total_pips / n as f64 },
        max_drawdown: max_dd,
        max_drawdown_pct: max_dd_pct,
        return_pct: if initial_balance > 0.0 { net_profit / initial_balance * 100.0 } else { 0.0 },
        sharpe: sharpe(equity),
        recovery_factor: (max_dd > 0.0).then(|| net_profit / max_dd),
        max_consecutive_wins: max_w,
        max_consecutive_losses: max_l,
        avg_hold_minutes: if n == 0 {
            0.0
        } else {
            trades.iter().map(|t| (t.exit_time - t.entry_time).num_seconds() as f64 / 60.0).sum::<f64>() / n as f64
        },
        commission: trades.iter().map(|t| t.commission).sum(),
        swap: trades.iter().map(|t| t.swap).sum(),
        long_trades,
        long_win_rate,
        short_trades,
        short_win_rate,
        exit_reasons,
        monthly: monthly.into_values().collect(),
    }
}

/// 決済のたびに残高が動いたとみなした資産の推移（含み損益は入らない）
pub fn equity_from_trades(trades: &[BacktestTrade], initial_balance: f64) -> Vec<EquityPoint> {
    let mut sorted: Vec<&BacktestTrade> = trades.iter().collect();
    sorted.sort_by_key(|t| t.exit_time);
    let mut balance = initial_balance;
    let mut out = Vec::with_capacity(sorted.len() + 1);
    if let Some(first) = sorted.first() {
        out.push(EquityPoint { time: first.entry_time, balance, equity: balance });
    }
    for t in sorted {
        balance += t.net;
        out.push(EquityPoint { time: t.exit_time, balance, equity: balance });
    }
    out
}

/// 最大ドローダウン（金額, %）
fn drawdown(equity: &[EquityPoint]) -> (f64, f64) {
    let (mut peak, mut max_dd, mut max_pct) = (f64::MIN, 0.0_f64, 0.0_f64);
    for p in equity {
        peak = peak.max(p.equity);
        let dd = peak - p.equity;
        max_dd = max_dd.max(dd);
        if peak > 0.0 {
            max_pct = max_pct.max(dd / peak * 100.0);
        }
    }
    (max_dd, max_pct)
}

/// 日ごとの最後の資産から日次リターンを作り、年率換算のシャープレシオを求める
fn sharpe(equity: &[EquityPoint]) -> Option<f64> {
    let mut daily: BTreeMap<NaiveDate, f64> = BTreeMap::new();
    for p in equity {
        daily.insert(p.time.date_naive(), p.equity);
    }
    let values: Vec<f64> = daily.into_values().collect();
    let returns: Vec<f64> = values.windows(2).filter(|w| w[0] > 0.0).map(|w| w[1] / w[0] - 1.0).collect();
    if returns.len() < 2 {
        return None;
    }
    let mean = returns.iter().sum::<f64>() / returns.len() as f64;
    let var = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (returns.len() - 1) as f64;
    let sd = var.sqrt();
    (sd > 0.0).then(|| mean / sd * 252f64.sqrt())
}

/// 保存用に資産の推移を間引く（最大 `max_points` 点。最高値・最安値の点は残す）
pub fn downsample(equity: &[EquityPoint], max_points: usize) -> Vec<EquityPoint> {
    if equity.len() <= max_points || max_points < 4 {
        return equity.to_vec();
    }
    let bucket = equity.len().div_ceil(max_points / 2);
    let mut out = Vec::with_capacity(max_points + 2);
    for chunk in equity.chunks(bucket) {
        let lo = chunk.iter().min_by(|a, b| a.equity.total_cmp(&b.equity)).copied();
        let hi = chunk.iter().max_by(|a, b| a.equity.total_cmp(&b.equity)).copied();
        let (Some(lo), Some(hi)) = (lo, hi) else { continue };
        if lo.time <= hi.time {
            out.extend([lo, hi]);
        } else {
            out.extend([hi, lo]);
        }
    }
    out.dedup_by_key(|p| p.time);
    if let (Some(last), Some(end)) = (out.last(), equity.last()) {
        if last.time != end.time {
            out.push(*end);
        }
    }
    out
}

/// 期間内の日数（年率換算などの表示用）
pub fn span_days(from: DateTime<Utc>, to: DateTime<Utc>) -> f64 {
    (to - from).num_seconds() as f64 / 86_400.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backtest::sim::ExitReason;
    use chrono::{Duration, TimeZone};

    fn trade(day: i64, net: f64, side: Side) -> BacktestTrade {
        let t = Utc.with_ymd_and_hms(2026, 9, 1, 9, 0, 0).unwrap() + Duration::days(day);
        BacktestTrade {
            id: format!("t{day}"),
            side,
            volume_lots: 0.1,
            entry_kind: "MARKET".into(),
            entry_time: t,
            entry_price: 150.0,
            exit_time: t + Duration::hours(2),
            exit_price: 150.0,
            stop_loss: None,
            take_profit: None,
            pnl_pips: net / 100.0,
            gross: net,
            commission: 10.0,
            swap: 0.0,
            net,
            exit_reason: if net > 0.0 { ExitReason::TakeProfit } else { ExitReason::StopLoss },
            entry_reason: String::new(),
            exit_note: String::new(),
            mae_pips: 0.0,
            mfe_pips: 0.0,
        }
    }

    #[test]
    fn basic_metrics_from_trades() {
        let trades = vec![trade(0, 300.0, Side::Buy), trade(1, -100.0, Side::Sell), trade(2, -100.0, Side::Buy), trade(3, 200.0, Side::Buy)];
        let m = compute(&trades, &[], 10_000.0);
        assert_eq!((m.trades, m.wins, m.losses), (4, 2, 2));
        assert_eq!(m.win_rate, 50.0);
        assert_eq!(m.net_profit, 300.0);
        assert_eq!(m.profit_factor, Some(2.5));
        assert_eq!(m.expectancy, 75.0);
        assert_eq!(m.payoff_ratio, Some(2.5));
        // 10,300 → 10,200 → 10,100 の 200 が最大ドローダウン
        assert_eq!(m.max_drawdown, 200.0);
        assert_eq!(m.max_consecutive_losses, 2);
        assert_eq!(m.final_balance, 10_300.0);
        assert_eq!((m.long_trades, m.short_trades), (3, 1));
        assert_eq!(m.exit_reasons["STOP_LOSS"], 2);
        assert_eq!(m.monthly.len(), 1);
        assert_eq!(m.commission, 40.0);
        assert!(m.sharpe.is_some());
        assert_eq!(m.avg_hold_minutes, 120.0);
    }

    #[test]
    fn no_trades_yields_zeros_not_nan() {
        let m = compute(&[], &[], 10_000.0);
        assert_eq!(m.trades, 0);
        assert_eq!(m.profit_factor, None);
        assert_eq!(m.win_rate, 0.0);
        assert!(m.sharpe.is_none());
    }

    #[test]
    fn downsample_keeps_extremes_and_the_end() {
        let t0 = Utc.with_ymd_and_hms(2026, 9, 1, 0, 0, 0).unwrap();
        let eq: Vec<EquityPoint> = (0..1000)
            .map(|i| {
                let v = if i == 500 { 5.0 } else { 100.0 + i as f64 };
                EquityPoint { time: t0 + Duration::minutes(i), balance: v, equity: v }
            })
            .collect();
        let d = downsample(&eq, 100);
        assert!(d.len() <= 102);
        assert!(d.iter().any(|p| p.equity == 5.0));
        assert_eq!(d.last().unwrap().time, eq.last().unwrap().time);
        assert_eq!(drawdown(&d).0, drawdown(&eq).0);
    }
}
