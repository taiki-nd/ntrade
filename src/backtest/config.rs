//! バックテストの設定ファイル（`config/backtest.toml`）と、実行前の準備（設定の解決・データの読み込み）。
//! CLI（`bin/backtest`）とダッシュボードの API が同じ手順で実行する。

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use super::{BacktestConfig, Costs, DataRequest, MarketData};
use crate::ctrader::BarPeriod;
use crate::guard::{GuardConfig, DEFAULT_GUARD_CONFIG_PATH};
use crate::settings::{TradingSettings, SETTINGS_KEY};
use crate::storage::Db;
use crate::strategy::{self, StrategySpec};

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
