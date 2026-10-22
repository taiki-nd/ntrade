//! パラメータ探索（グリッドサーチ）とウォークフォワード分析。
//!
//! グリッドサーチは全組み合わせを並列に走らせて目的関数で並べるだけなので、過剰最適化しやすい。
//! 採用前にウォークフォワード（学習期間で選んだパラメータを、その直後の未知の期間で評価する）で確かめる。

use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use super::metrics::{self, Metrics};
use super::{run, BacktestConfig, BacktestTrade, MarketData};
use crate::strategy::{self, Params};

/// 1 パラメータの候補値（`fast=5..30:5` / `slow=50,100,200` / `sl_atr=1.5`）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GridAxis {
    pub name: String,
    pub values: Vec<f64>,
}

impl GridAxis {
    pub fn parse(s: &str) -> Result<Self> {
        let (name, spec) = s.split_once('=').ok_or_else(|| anyhow!("grid '{s}' must be name=values"))?;
        let values = if let Some((range, step)) = spec.split_once(':') {
            let (a, b) = range.split_once("..").ok_or_else(|| anyhow!("grid '{s}': range must be start..end:step"))?;
            let (a, b, step): (f64, f64, f64) = (a.trim().parse()?, b.trim().parse()?, step.trim().parse()?);
            if step <= 0.0 || b < a {
                bail!("grid '{s}': need start <= end and step > 0");
            }
            let n = ((b - a) / step + 1e-9).floor() as usize;
            (0..=n).map(|i| ((a + step * i as f64) * 1e9).round() / 1e9).collect()
        } else {
            spec.split(',').map(|v| v.trim().parse::<f64>().map_err(|e| anyhow!("grid '{s}': {e}"))).collect::<Result<Vec<_>>>()?
        };
        if values.is_empty() {
            bail!("grid '{s}' has no values");
        }
        Ok(Self { name: name.trim().to_string(), values })
    }
}

/// 全組み合わせ（`base` の値に重ねる）
pub fn combinations(base: &Params, axes: &[GridAxis]) -> Vec<Params> {
    let mut out = vec![base.clone()];
    for axis in axes {
        out = out
            .into_iter()
            .flat_map(|p| {
                axis.values.iter().map(move |v| {
                    let mut q = p.clone();
                    q.insert(axis.name.clone(), *v);
                    q
                })
            })
            .collect();
    }
    out
}

/// 何を最大化するか
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Objective {
    NetProfit,
    ProfitFactor,
    Sharpe,
    RecoveryFactor,
    Expectancy,
}

impl Objective {
    pub fn parse(s: &str) -> Result<Self> {
        Ok(match s.trim().to_lowercase().as_str() {
            "net" | "net_profit" | "profit" => Self::NetProfit,
            "pf" | "profit_factor" => Self::ProfitFactor,
            "sharpe" => Self::Sharpe,
            "rf" | "recovery" | "recovery_factor" => Self::RecoveryFactor,
            "exp" | "expectancy" => Self::Expectancy,
            other => bail!("unknown objective '{other}' (net / pf / sharpe / recovery / expectancy)"),
        })
    }

    /// 取引数が `min_trades` 未満の組み合わせは選ばない（None）
    pub fn score(self, m: &Metrics, min_trades: usize) -> Option<f64> {
        if m.trades < min_trades.max(1) {
            return None;
        }
        Some(match self {
            Self::NetProfit => m.net_profit,
            // 損失ゼロは比が出ないので、勝ち数に応じた大きな値として扱う
            Self::ProfitFactor => m.profit_factor.unwrap_or(if m.net_profit > 0.0 { 100.0 } else { 0.0 }),
            Self::Sharpe => m.sharpe?,
            Self::RecoveryFactor => m.recovery_factor.unwrap_or(if m.net_profit > 0.0 { 100.0 } else { 0.0 }),
            Self::Expectancy => m.expectancy,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SweepRow {
    pub params: Params,
    pub metrics: Metrics,
    pub score: Option<f64>,
}

/// グリッドサーチ。組み合わせごとに `cfg` のパラメータだけを差し替えて走らせ、スコアの高い順に返す。
/// 組めないパラメータ（fast >= slow など）は飛ばす
pub fn sweep(
    data: &MarketData,
    cfg: &BacktestConfig,
    combos: &[Params],
    objective: Objective,
    min_trades: usize,
    threads: usize,
) -> Vec<SweepRow> {
    let next = AtomicUsize::new(0);
    let rows = Mutex::new(Vec::with_capacity(combos.len()));
    std::thread::scope(|s| {
        for _ in 0..threads.max(1).min(combos.len().max(1)) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(params) = combos.get(i) else { break };
                let mut c = cfg.clone();
                c.strategy.params = params.clone();
                if strategy::build(&c.strategy).is_err() {
                    continue;
                }
                if let Ok(r) = run(data, &c) {
                    let score = objective.score(&r.metrics, min_trades);
                    let params = strategy::resolve_params(&c.strategy).unwrap_or_else(|_| params.clone());
                    rows.lock().unwrap_or_else(|e| e.into_inner()).push(SweepRow { params, metrics: r.metrics, score });
                }
            });
        }
    });
    let mut rows = rows.into_inner().unwrap_or_else(|e| e.into_inner());
    rows.sort_by(|a, b| match (b.score, a.score) {
        (Some(x), Some(y)) => x.total_cmp(&y),
        (Some(_), None) => std::cmp::Ordering::Greater,
        (None, Some(_)) => std::cmp::Ordering::Less,
        (None, None) => b.metrics.net_profit.total_cmp(&a.metrics.net_profit),
    });
    rows
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WalkForwardWindow {
    pub train_from: DateTime<Utc>,
    pub train_to: DateTime<Utc>,
    pub test_from: DateTime<Utc>,
    pub test_to: DateTime<Utc>,
    /// 学習期間で選んだパラメータ（どの組み合わせも条件を満たさなければ None で、その期間は取引しない）
    pub params: Option<Params>,
    pub train: Option<Metrics>,
    pub test: Option<Metrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WalkForwardResult {
    pub windows: Vec<WalkForwardWindow>,
    /// 検証期間だけをつないだ成績（未知のデータでの成績）
    pub oos: Metrics,
    pub oos_trades: Vec<BacktestTrade>,
}

/// ウォークフォワード分析: [学習 → 検証] の窓を検証期間ずつずらしながら繰り返す
#[allow(clippy::too_many_arguments)]
pub fn walk_forward(
    data: &MarketData,
    cfg: &BacktestConfig,
    combos: &[Params],
    objective: Objective,
    min_trades: usize,
    train: Duration,
    test: Duration,
    threads: usize,
) -> Result<WalkForwardResult> {
    if train <= Duration::zero() || test <= Duration::zero() {
        bail!("train and test periods must be positive");
    }
    if cfg.from + train + test > cfg.to {
        bail!("period {}..{} is shorter than one train + test window", cfg.from, cfg.to);
    }
    let mut windows = Vec::new();
    let mut oos_trades = Vec::new();
    let mut start = cfg.from;
    while start + train < cfg.to {
        let (train_to, test_to) = (start + train, (start + train + test).min(cfg.to));
        let train_cfg = BacktestConfig { from: start, to: train_to, ..cfg.clone() };
        let rows = sweep(data, &train_cfg, combos, objective, min_trades, threads);
        let best = rows.into_iter().find(|r| r.score.is_some());
        let mut w = WalkForwardWindow {
            train_from: start,
            train_to,
            test_from: train_to,
            test_to,
            params: None,
            train: None,
            test: None,
        };
        if let Some(best) = best {
            let mut test_cfg = BacktestConfig { from: train_to, to: test_to, ..cfg.clone() };
            test_cfg.strategy.params = best.params.clone();
            let r = run(data, &test_cfg)?;
            oos_trades.extend(r.trades);
            w.params = Some(best.params);
            w.train = Some(best.metrics);
            w.test = Some(r.metrics);
        }
        windows.push(w);
        start += test;
    }
    oos_trades.sort_by_key(|t| t.exit_time);
    let oos = metrics::compute(&oos_trades, &[], cfg.initial_balance);
    Ok(WalkForwardResult { windows, oos, oos_trades })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_axes_parse_ranges_and_lists() {
        assert_eq!(GridAxis::parse("fast=5..20:5").unwrap().values, vec![5.0, 10.0, 15.0, 20.0]);
        assert_eq!(GridAxis::parse("sl_atr=1..2:0.5").unwrap().values, vec![1.0, 1.5, 2.0]);
        assert_eq!(GridAxis::parse("slow=50, 100").unwrap().values, vec![50.0, 100.0]);
        assert!(GridAxis::parse("fast").is_err());
        assert!(GridAxis::parse("fast=10..5:1").is_err());

        let axes = vec![GridAxis::parse("fast=5,10").unwrap(), GridAxis::parse("slow=50,100,200").unwrap()];
        let base = Params::from([("tp_rr".to_string(), 2.0)]);
        let combos = combinations(&base, &axes);
        assert_eq!(combos.len(), 6);
        assert!(combos.iter().all(|p| p["tp_rr"] == 2.0));
    }

    #[test]
    fn objective_requires_min_trades() {
        let m = Metrics { trades: 5, net_profit: 100.0, profit_factor: Some(1.5), ..Default::default() };
        assert_eq!(Objective::ProfitFactor.score(&m, 10), None);
        assert_eq!(Objective::ProfitFactor.score(&m, 5), Some(1.5));
        assert_eq!(Objective::parse("pf").unwrap(), Objective::ProfitFactor);
        assert!(Objective::parse("magic").is_err());
    }

    #[test]
    fn sweep_and_walk_forward_run_on_synthetic_data() {
        use crate::backtest::{Conversion, Costs};
        use crate::ctrader::{BarPeriod, CandleBar};
        use crate::guard::GuardConfig;
        use crate::strategy::StrategySpec;
        use chrono::TimeZone;
        let start = Utc.with_ymd_and_hms(2026, 1, 5, 0, 0, 0).unwrap();
        let bars: Vec<CandleBar> = (0..20_000)
            .map(|i| {
                let x = i as f64;
                let c = 150.0 + (x / 50.0).sin() * 0.6 + (x / 333.0).cos() * 0.9;
                CandleBar { timestamp: start + Duration::minutes(5 * i), open: c, high: c + 0.05, low: c - 0.05, close: c, volume: 1 }
            })
            .collect();
        let data = MarketData {
            pair: "USDJPY".into(),
            strategy_period: BarPeriod::M5,
            strategy_bars: bars.clone(),
            exec_period: BarPeriod::M5,
            exec_bars: bars,
            atr_bars: Default::default(),
            conversion: Conversion::Identity,
            pip_size: 0.01,
            lot_units: 100_000.0,
            min_lots: 0.01,
            lot_step: 0.01,
        };
        let cfg = BacktestConfig {
            pair: "USDJPY".into(),
            from: start + Duration::days(2),
            to: start + Duration::days(69),
            strategy: StrategySpec { name: "ema_cross".into(), timeframe: BarPeriod::M5, params: Params::new() },
            guard: GuardConfig { min_rr: 0.0, sl_atr_min: 0.0, sl_atr_max: 0.0, ..GuardConfig::default() },
            costs: Costs::default(),
            initial_balance: 1_000_000.0,
            account_currency: "JPY".into(),
        };
        let combos = combinations(
            &Params::new(),
            &[GridAxis::parse("fast=5,10,60").unwrap(), GridAxis::parse("slow=20,50").unwrap()],
        );
        let rows = sweep(&data, &cfg, &combos, Objective::NetProfit, 1, 4);
        // fast=60 は slow より長いので組めない
        assert_eq!(rows.len(), 4);
        assert!(rows.windows(2).all(|w| w[0].score.unwrap_or(f64::MIN) >= w[1].score.unwrap_or(f64::MIN)));

        let wf = walk_forward(&data, &cfg, &combos, Objective::NetProfit, 1, Duration::days(20), Duration::days(10), 4).unwrap();
        assert!(wf.windows.len() >= 4);
        assert!(wf.oos_trades.iter().all(|t| t.entry_time >= cfg.from + Duration::days(20)));
        assert_eq!(wf.oos.trades, wf.oos_trades.len());
    }
}
