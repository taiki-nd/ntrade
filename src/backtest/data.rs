//! バックテストの入力データ: SQLite の足（戦略の時間足・執行用の足・ATR 用の足）と口座通貨への換算レート。

use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;

use crate::ctrader::{quote_currency_from_name, BarPeriod, CandleBar};
use crate::guard::{AtrSet, AtrTimeframe};
use crate::storage::Db;

/// 建値通貨 1 単位を口座通貨に換算するレート
#[derive(Debug, Clone)]
pub enum Conversion {
    /// 建値通貨 = 口座通貨
    Identity,
    /// 固定レート（換算用の足が無いとき）
    Fixed(f64),
    /// 換算用の銘柄の足の終値（`invert` なら逆数）
    Series { pair: String, bars: Vec<CandleBar>, invert: bool },
}

impl Conversion {
    /// `t` 時点で確定している直近の終値から求めたレート
    pub fn rate_at(&self, t: DateTime<Utc>) -> f64 {
        match self {
            Conversion::Identity => 1.0,
            Conversion::Fixed(r) => *r,
            Conversion::Series { bars, invert, .. } => {
                let i = bars.partition_point(|b| b.timestamp < t);
                let close = bars.get(i.saturating_sub(1)).or(bars.first()).map(|b| b.close).unwrap_or(1.0);
                if *invert {
                    1.0 / close
                } else {
                    close
                }
            }
        }
    }

    pub fn label(&self) -> String {
        match self {
            Conversion::Identity => "same currency".into(),
            Conversion::Fixed(r) => format!("fixed {r}"),
            Conversion::Series { pair, invert, .. } => format!("{}{}", if *invert { "1 / " } else { "" }, pair),
        }
    }
}

/// 1 銘柄分のバックテスト用データ
#[derive(Debug, Clone)]
pub struct MarketData {
    pub pair: String,
    pub strategy_period: BarPeriod,
    /// 戦略の時間足（期間の前の助走分を含む）
    pub strategy_bars: Vec<CandleBar>,
    pub exec_period: BarPeriod,
    /// 約定の判定に使う足（期間内）
    pub exec_bars: Vec<CandleBar>,
    /// リスクチェックの ATR に使う足（時間足ごと）
    pub atr_bars: HashMap<AtrTimeframe, Vec<CandleBar>>,
    pub conversion: Conversion,
    pub pip_size: f64,
    /// 1 lot の数量（FX は 100,000 通貨）
    pub lot_units: f64,
    pub min_lots: f64,
    pub lot_step: f64,
}

/// データの読み込み条件
#[derive(Debug, Clone)]
pub struct DataRequest {
    pub pair: String,
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub strategy_period: BarPeriod,
    /// 戦略に渡す本数（助走として期間の前に読む本数）
    pub lookback: usize,
    /// None なら 1 分足があればそれ、無ければ戦略の時間足
    pub exec_period: Option<BarPeriod>,
    pub atr_timeframes: Vec<AtrTimeframe>,
    pub account_currency: String,
    /// 換算用の足が無いときの固定レート
    pub fx_rate: Option<f64>,
}

impl MarketData {
    pub fn load(db: &Db, req: &DataRequest) -> Result<Self> {
        let pair = req.pair.to_uppercase();
        // 保存済みの銘柄仕様（lot サイズ・pip 位置）があれば使う
        db.register_saved_instrument(&pair)?;
        let spec = crate::instrument::get(&pair);

        let mut strategy_bars = db.bars_until(&pair, req.strategy_period, req.from, req.lookback)?;
        let start = strategy_bars.last().map(|b| b.timestamp + Duration::seconds(1)).unwrap_or(req.from);
        strategy_bars.extend(db.bars_range(&pair, req.strategy_period, start, req.to)?);
        if strategy_bars.is_empty() {
            bail!("no {} bars for {pair} in {}..{} (run `backtest fetch` first)", req.strategy_period.as_str(), req.from, req.to);
        }

        let exec_period = match req.exec_period {
            Some(p) => p,
            None => {
                let m1_from = db.first_bar_ts(&pair, BarPeriod::M1)?;
                let m1_to = db.last_bar_ts(&pair, BarPeriod::M1)?;
                match (m1_from, m1_to) {
                    (Some(a), Some(b)) if a <= req.from + Duration::days(1) && b >= req.to.min(Utc::now()) - Duration::days(3) => BarPeriod::M1,
                    _ => req.strategy_period,
                }
            }
        };
        if exec_period.duration() > req.strategy_period.duration() {
            bail!("execution timeframe {} must not be longer than the strategy timeframe {}", exec_period.as_str(), req.strategy_period.as_str());
        }
        let exec_bars = db.bars_range(&pair, exec_period, req.from, req.to)?;
        if exec_bars.is_empty() {
            bail!("no {} bars for {pair} in {}..{}", exec_period.as_str(), req.from, req.to);
        }

        let mut atr_bars = HashMap::new();
        for tf in &req.atr_timeframes {
            let period = tf.period();
            let mut bars = db.bars_until(&pair, period, req.from, 40)?;
            let start = bars.last().map(|b| b.timestamp + Duration::seconds(1)).unwrap_or(req.from);
            bars.extend(db.bars_range(&pair, period, start, req.to)?);
            atr_bars.insert(*tf, bars);
        }

        let conversion = load_conversion(db, &pair, spec.as_ref().and_then(|s| s.quote_asset.clone()), req)?;
        let pip_size = crate::instrument::pip_size(&pair);
        let lot_units = spec.as_ref().map(|s| s.lot_units).filter(|v| *v > 0.0).unwrap_or_else(|| guess_lot_units(&pair));
        let min_lots = spec.as_ref().and_then(|s| s.min_lots()).unwrap_or(0.01);
        let lot_step = spec
            .as_ref()
            .and_then(|s| s.step_volume.filter(|v| *v > 0).map(|v| s.volume_to_lots(v)))
            .unwrap_or(0.01);
        Ok(Self {
            pair,
            strategy_period: req.strategy_period,
            strategy_bars,
            exec_period,
            exec_bars,
            atr_bars,
            conversion,
            pip_size,
            lot_units,
            min_lots,
            lot_step,
        })
    }

    /// `t` 時点で確定している足から ATR(14)（pips）を時間足ごとに求める
    pub fn atr_at(&self, t: DateTime<Utc>) -> AtrSet {
        let mut set = AtrSet::default();
        for (tf, bars) in &self.atr_bars {
            let dur = tf.period().duration();
            let end = bars.partition_point(|b| b.timestamp + dur <= t);
            let start = end.saturating_sub(30);
            set.insert_from_bars(*tf, &bars[start..end], self.pip_size);
        }
        set
    }
}

/// 銘柄名から 1 lot の数量を推測する（銘柄仕様が保存されていないとき）
fn guess_lot_units(pair: &str) -> f64 {
    let p = pair.to_uppercase();
    if p.contains("XAU") {
        100.0
    } else if p.contains("XAG") {
        5_000.0
    } else if crate::instrument::name_looks_like_fx(&p) {
        100_000.0
    } else {
        1.0
    }
}

fn load_conversion(db: &Db, pair: &str, quote_asset: Option<String>, req: &DataRequest) -> Result<Conversion> {
    let account = req.account_currency.to_uppercase();
    let quote = quote_asset
        .or_else(|| quote_currency_from_name(pair))
        .or_else(|| pair.contains("XAU").then(|| "USD".to_string()))
        .ok_or_else(|| anyhow!("cannot determine the quote currency of {pair}"))?
        .to_uppercase();
    if quote == account {
        return Ok(Conversion::Identity);
    }
    // 同じ口座サフィックスの換算用銘柄（EURUSD_Z なら USDJPY_Z）を探す
    let suffix: String = pair.chars().skip_while(|c| c.is_ascii_alphabetic()).collect();
    let coverage = db.coverage()?;
    let pick = |base: &str, quote: &str| -> Option<String> {
        let want = format!("{base}{quote}");
        let mut cands: Vec<&str> = coverage
            .iter()
            .map(|c| c.pair.as_str())
            .filter(|p| p.chars().take_while(|c| c.is_ascii_alphabetic()).collect::<String>() == want)
            .collect();
        cands.sort_by_key(|p| !p.ends_with(&suffix));
        cands.first().map(|s| s.to_string())
    };
    for (base, q, invert) in [(quote.as_str(), account.as_str(), false), (account.as_str(), quote.as_str(), true)] {
        let Some(conv_pair) = pick(base, q) else { continue };
        for period in [BarPeriod::H1, BarPeriod::M15, BarPeriod::M5, BarPeriod::H4, BarPeriod::D1] {
            let mut bars = db.bars_until(&conv_pair, period, req.from, 2)?;
            bars.extend(db.bars_range(&conv_pair, period, req.from, req.to)?);
            if bars.len() > 2 {
                return Ok(Conversion::Series { pair: conv_pair, bars, invert });
            }
        }
    }
    match req.fx_rate {
        Some(r) if r > 0.0 => Ok(Conversion::Fixed(r)),
        _ => bail!(
            "no bars to convert {quote} into {account} (fetch {quote}{account} or pass --fx-rate)"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn bars(start: DateTime<Utc>, n: usize, step_min: i64, price: f64) -> Vec<CandleBar> {
        (0..n)
            .map(|i| CandleBar {
                timestamp: start + Duration::minutes(step_min * i as i64),
                open: price,
                high: price + 0.1,
                low: price - 0.1,
                close: price + i as f64 * 0.001,
                volume: 1,
            })
            .collect()
    }

    #[test]
    fn loads_warmup_exec_bars_and_conversion() {
        let mut db = Db::open_in_memory().unwrap();
        let t0 = Utc.with_ymd_and_hms(2026, 9, 1, 0, 0, 0).unwrap();
        db.insert_bars("EURUSD_Z", BarPeriod::M5, &bars(t0, 600, 5, 1.10)).unwrap();
        db.insert_bars("EURUSD_Z", BarPeriod::H1, &bars(t0, 50, 60, 1.10)).unwrap();
        db.insert_bars("USDJPY_Z", BarPeriod::H1, &bars(t0, 50, 60, 150.0)).unwrap();
        let from = t0 + Duration::hours(10);
        let req = DataRequest {
            pair: "EURUSD_z".into(),
            from,
            to: from + Duration::hours(12),
            strategy_period: BarPeriod::M5,
            lookback: 50,
            exec_period: None,
            atr_timeframes: vec![AtrTimeframe::M5, AtrTimeframe::H1],
            account_currency: "JPY".into(),
            fx_rate: None,
        };
        let d = MarketData::load(&db, &req).unwrap();
        // 助走 50 本 + 期間内 144 本
        assert_eq!(d.strategy_bars.len(), 50 + 144);
        assert!(d.strategy_bars[49].timestamp + Duration::minutes(5) <= from);
        // 1 分足が無いので戦略の時間足で約定を判定する
        assert_eq!(d.exec_period, BarPeriod::M5);
        assert_eq!(d.exec_bars.first().unwrap().timestamp, from);
        assert_eq!(d.lot_units, 100_000.0);
        assert!(matches!(&d.conversion, Conversion::Series { pair, invert: false, .. } if pair == "USDJPY_Z"));
        assert!((d.conversion.rate_at(from) - 150.009).abs() < 1e-9);
        let atr = d.atr_at(from);
        assert!(atr.get(AtrTimeframe::M5).is_some());

        // 換算できなければ固定レートを求める
        let req2 = DataRequest { account_currency: "GBP".into(), ..req.clone() };
        assert!(MarketData::load(&db, &req2).is_err());
        let d = MarketData::load(&db, &DataRequest { fx_rate: Some(0.8), ..req2 }).unwrap();
        assert_eq!(d.conversion.rate_at(from), 0.8);
    }
}
