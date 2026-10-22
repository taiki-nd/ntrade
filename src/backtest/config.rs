//! バックテストの設定ファイル（`config/backtest.toml`）と、実行前の準備（設定の解決・データの読み込み）。
//! CLI（`bin/backtest`）とダッシュボードの API が同じ手順で実行する。

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use super::optimize::{self, GridAxis, Objective, Progress, SweepRow, WalkForwardResult};
use super::{BacktestConfig, Costs, DataRequest, MarketData};
use crate::ctrader::BarPeriod;
use crate::guard::{GuardConfig, DEFAULT_GUARD_CONFIG_PATH};
use crate::settings::{TradingSettings, SETTINGS_KEY};
use crate::storage::Db;
use crate::strategy::{self, Params, StrategySpec};

pub const DEFAULT_BACKTEST_CONFIG_PATH: &str = "config/backtest.toml";

/// リスク設定をどこから取るか
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GuardSource {
    /// 本番と同じ（SQLite に保存した取引設定。未保存なら guard.toml）
    #[default]
    Settings,
    /// config/guard.toml
    File,
}

/// 銘柄ごとに上書きするコスト（書いた項目だけ上書き）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CostOverride {
    pub spread_pips: Option<f64>,
    pub slippage_pips: Option<f64>,
    pub commission_per_lot: Option<f64>,
    pub swap_long_per_lot: Option<f64>,
    pub swap_short_per_lot: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BacktestFileConfig {
    pub account_currency: String,
    pub initial_balance: f64,
    pub guard: GuardSource,
    /// 約定を判定する足（"auto" なら 1 分足があればそれ）
    pub exec_timeframe: String,
    /// 換算用の足が無いときの固定レート（建値通貨 → 口座通貨）
    pub fx_rate: Option<f64>,
    pub costs: Costs,
    /// 銘柄ごとの上書き（キーは大文字小文字を区別しない）
    pub pairs: HashMap<String, CostOverride>,
}

impl Default for BacktestFileConfig {
    fn default() -> Self {
        Self {
            account_currency: "JPY".into(),
            initial_balance: 1_000_000.0,
            guard: GuardSource::Settings,
            exec_timeframe: "auto".into(),
            fx_rate: None,
            costs: Costs::default(),
            pairs: HashMap::new(),
        }
    }
}

impl BacktestFileConfig {
    pub fn load_or_default(path: impl AsRef<Path>) -> Result<Self> {
        match std::fs::read_to_string(path.as_ref()) {
            Ok(text) => toml::from_str(&text).with_context(|| format!("invalid {:?}", path.as_ref())),
            Err(_) => Ok(Self::default()),
        }
    }

    /// 銘柄のコスト（共通の値に銘柄ごとの上書きを重ねる）
    pub fn costs_for(&self, pair: &str) -> Costs {
        let mut c = self.costs.clone();
        if let Some(o) = self.pairs.iter().find(|(k, _)| k.eq_ignore_ascii_case(pair)).map(|(_, v)| v) {
            c.spread_pips = o.spread_pips.unwrap_or(c.spread_pips);
            c.slippage_pips = o.slippage_pips.unwrap_or(c.slippage_pips);
            c.commission_per_lot = o.commission_per_lot.unwrap_or(c.commission_per_lot);
            c.swap_long_per_lot = o.swap_long_per_lot.unwrap_or(c.swap_long_per_lot);
            c.swap_short_per_lot = o.swap_short_per_lot.unwrap_or(c.swap_short_per_lot);
        }
        c
    }

    pub fn exec_period(&self) -> Result<Option<BarPeriod>> {
        match self.exec_timeframe.trim() {
            "" | "auto" => Ok(None),
            s => BarPeriod::parse(s).map(Some).ok_or_else(|| anyhow::anyhow!("unknown exec_timeframe {s}")),
        }
    }

    pub fn guard_config(&self, db: &Db) -> Result<GuardConfig> {
        Ok(match self.guard {
            GuardSource::Settings => match db.load_setting::<TradingSettings>(SETTINGS_KEY)? {
                Some(s) => s.guard,
                None => GuardConfig::load_or_default(DEFAULT_GUARD_CONFIG_PATH),
            },
            GuardSource::File => GuardConfig::load(DEFAULT_GUARD_CONFIG_PATH)?,
        })
    }
}

/// 1 回分の実行の指定（CLI の引数・API のリクエスト）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRequest {
    pub pair: String,
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub strategy: StrategySpec,
    /// 指定したものだけ設定ファイルの値を上書きする
    #[serde(default)]
    pub initial_balance: Option<f64>,
    #[serde(default)]
    pub spread_pips: Option<f64>,
    #[serde(default)]
    pub commission_per_lot: Option<f64>,
    #[serde(default)]
    pub exec_timeframe: Option<BarPeriod>,
    #[serde(default)]
    pub label: String,
}

/// 設定を解決してデータを読み込む。`extra_lookback` は最適化で長い期間のパラメータを試すときの助走
pub fn prepare(db: &Db, file: &BacktestFileConfig, req: &RunRequest, extra_lookback: usize) -> Result<(MarketData, BacktestConfig)> {
    if req.from >= req.to {
        anyhow::bail!("from ({}) must be before to ({})", req.from, req.to);
    }
    let guard = file.guard_config(db)?;
    let mut costs = file.costs_for(&req.pair);
    if let Some(v) = req.spread_pips {
        costs.spread_pips = v;
    }
    if let Some(v) = req.commission_per_lot {
        costs.commission_per_lot = v;
    }
    let lookback = strategy::build(&req.strategy)?.lookback().max(extra_lookback);
    let data = MarketData::load(
        db,
        &DataRequest {
            pair: req.pair.clone(),
            from: req.from,
            to: req.to,
            strategy_period: req.strategy.timeframe,
            lookback,
            exec_period: match req.exec_timeframe {
                Some(p) => Some(p),
                None => file.exec_period()?,
            },
            atr_timeframes: guard.atr_timeframes(),
            account_currency: file.account_currency.clone(),
            fx_rate: file.fx_rate,
        },
    )?;
    let cfg = BacktestConfig {
        pair: data.pair.clone(),
        from: req.from,
        to: req.to,
        strategy: StrategySpec { params: strategy::resolve_params(&req.strategy)?, ..req.strategy.clone() },
        guard,
        costs,
        initial_balance: req.initial_balance.unwrap_or(file.initial_balance),
        account_currency: file.account_currency.clone(),
    };
    Ok((data, cfg))
}

/// 単発の実行結果を保存する
pub fn save_single(db: &mut Db, result: &super::BacktestResult, label: &str) -> Result<i64> {
    let cfg = &result.config;
    let run = crate::storage::NewBacktestRun {
        label,
        kind: "single",
        pair: &cfg.pair,
        strategy: &cfg.strategy.label(),
        from: cfg.from,
        to: cfg.to,
        config: serde_json::json!({
            "config": cfg,
            "execPeriod": result.exec_period,
            "conversion": result.conversion,
            "decisions": result.decisions,
            "signals": result.signals,
        }),
        metrics: serde_json::to_value(&result.metrics)?,
        summary: serde_json::Value::Null,
    };
    db.insert_backtest_run(&run, &result.trades, &super::metrics::downsample(&result.equity, 2000))
}

/// グリッドサーチ・ウォークフォワードの指定（CLI の引数・API のリクエスト）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridRequest {
    #[serde(flatten)]
    pub run: RunRequest,
    /// 探索するパラメータ（`fast=5..30:5` / `slow=50,100` / `sl_atr=1.5`）
    pub grid: Vec<String>,
    pub objective: Objective,
    /// これ未満の取引数の組み合わせは選ばない
    #[serde(default = "default_min_trades")]
    pub min_trades: usize,
}

fn default_min_trades() -> usize {
    30
}

/// ウォークフォワードの期間（日数）
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WalkForwardWindows {
    pub train_days: i64,
    pub test_days: i64,
}

/// 探索する組み合わせ。パラメータ名の誤りは走らせる前に止める
pub fn grid_combos(spec: &StrategySpec, grid: &[String]) -> Result<Vec<Params>> {
    let axes: Vec<GridAxis> = grid.iter().map(|s| GridAxis::parse(s)).collect::<Result<_>>()?;
    if axes.is_empty() {
        anyhow::bail!("探索するパラメータを 1 つ以上指定してください");
    }
    for a in &axes {
        let probe = StrategySpec { params: Params::from([(a.name.clone(), a.values[0])]), ..spec.clone() };
        if let Err(e) = strategy::resolve_params(&probe) {
            if e.to_string().contains("unknown") {
                return Err(e);
            }
        }
    }
    let combos = optimize::combinations(&spec.params, &axes);
    if combos.len() > MAX_COMBINATIONS {
        anyhow::bail!("組み合わせが {} 通りあります（上限 {MAX_COMBINATIONS}）。候補を減らしてください", combos.len());
    }
    Ok(combos)
}

/// 1 回のグリッドサーチで試す組み合わせの上限
pub const MAX_COMBINATIONS: usize = 5000;

/// 組み合わせの中で最も長い助走
pub fn max_lookback(spec: &StrategySpec, combos: &[Params]) -> usize {
    combos
        .iter()
        .filter_map(|p| strategy::build(&StrategySpec { params: p.clone(), ..spec.clone() }).ok())
        .map(|s| s.lookback())
        .max()
        .unwrap_or(0)
}

/// グリッドサーチを実行して保存する
pub struct SweepOutcome {
    pub id: i64,
    pub config: BacktestConfig,
    pub exec_period: BarPeriod,
    pub combinations: usize,
    pub rows: Vec<SweepRow>,
}

pub fn run_sweep(db: &mut Db, file: &BacktestFileConfig, req: &GridRequest, threads: usize, progress: &Progress) -> Result<SweepOutcome> {
    let combos = grid_combos(&req.run.strategy, &req.grid)?;
    progress.total.store(combos.len(), std::sync::atomic::Ordering::Relaxed);
    let (data, cfg) = prepare(db, file, &req.run, max_lookback(&req.run.strategy, &combos))?;
    let rows = optimize::sweep(&data, &cfg, &combos, req.objective, req.min_trades, threads, progress);
    if progress.is_cancelled() {
        anyhow::bail!("cancelled");
    }
    let best = rows.first().map(|r| serde_json::to_value(&r.metrics)).transpose()?.unwrap_or_default();
    let id = db.insert_backtest_run(
        &crate::storage::NewBacktestRun {
            label: &req.run.label,
            kind: "sweep",
            pair: &cfg.pair,
            strategy: &grid_label(&cfg.strategy),
            from: cfg.from,
            to: cfg.to,
            config: serde_json::json!({ "config": cfg, "execPeriod": data.exec_period, "grid": req.grid, "objective": req.objective, "minTrades": req.min_trades }),
            metrics: best,
            summary: serde_json::to_value(&rows)?,
        },
        &[],
        &[],
    )?;
    Ok(SweepOutcome { id, config: cfg, exec_period: data.exec_period, combinations: combos.len(), rows })
}

pub struct WalkForwardOutcome {
    pub id: i64,
    pub config: BacktestConfig,
    pub combinations: usize,
    pub result: WalkForwardResult,
}

/// ウォークフォワードを実行して保存する
pub fn run_walk_forward(
    db: &mut Db,
    file: &BacktestFileConfig,
    req: &GridRequest,
    windows: WalkForwardWindows,
    threads: usize,
    progress: &Progress,
) -> Result<WalkForwardOutcome> {
    let combos = grid_combos(&req.run.strategy, &req.grid)?;
    let (data, cfg) = prepare(db, file, &req.run, max_lookback(&req.run.strategy, &combos))?;
    let result = optimize::walk_forward(
        &data,
        &cfg,
        &combos,
        req.objective,
        req.min_trades,
        Duration::days(windows.train_days),
        Duration::days(windows.test_days),
        threads,
        progress,
    )?;
    let id = db.insert_backtest_run(
        &crate::storage::NewBacktestRun {
            label: &req.run.label,
            kind: "walk_forward",
            pair: &cfg.pair,
            strategy: &grid_label(&cfg.strategy),
            from: cfg.from,
            to: cfg.to,
            config: serde_json::json!({
                "config": cfg, "execPeriod": data.exec_period, "grid": req.grid, "objective": req.objective,
                "minTrades": req.min_trades, "trainDays": windows.train_days, "testDays": windows.test_days
            }),
            metrics: serde_json::to_value(&result.oos)?,
            summary: serde_json::to_value(&result.windows)?,
        },
        &result.oos_trades,
        &super::metrics::equity_from_trades(&result.oos_trades, cfg.initial_balance),
    )?;
    Ok(WalkForwardOutcome { id, config: cfg, combinations: combos.len(), result })
}

/// グリッドサーチ・ウォークフォワードの一覧表示用の戦略名（パラメータは組み合わせごとに違うので出さない）
fn grid_label(spec: &StrategySpec) -> String {
    format!("{} {} [grid]", spec.name, spec.timeframe.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_config_parses_and_overrides_per_pair() {
        let cfg = BacktestFileConfig::load_or_default(concat!(env!("CARGO_MANIFEST_DIR"), "/config/backtest.toml")).unwrap();
        assert_eq!(cfg.account_currency, "JPY");
        let usd = cfg.costs_for("usdjpy_z");
        let eur = cfg.costs_for("EURUSD_z");
        assert!(usd.commission_per_lot > 0.0 && eur.commission_per_lot > 0.0);
        assert_eq!(cfg.exec_period().unwrap(), None);
        // 書いていない銘柄は共通の値
        assert_eq!(cfg.costs_for("GBPJPY").commission_per_lot, cfg.costs.commission_per_lot);
    }
}
