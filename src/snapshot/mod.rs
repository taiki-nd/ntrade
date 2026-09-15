//! MarketSnapshot: LLM に渡す「客観的事実」の集合。
//!
//! 画像4枚（4H/1H/15M/5M）、15M/5M の生OHLC、ATR・前日高安・キリ番・スイング価格リスト
//! などを組み立てる。パターン判定・トレンドラベル・サポレジ判定は一切含まない。
//! 本番（現在時刻）でもリプレイ（過去時刻）でも同じ経路で生成される。

pub mod measures;
pub mod mock;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::chart::{ChartPlotter, ChartPlotterConfig, PriceLevel, TimeframeChart};
use crate::ctrader::CandleBar;
use measures::*;

/// 生OHLC 1本（コンパクト表現）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BarRow {
    pub t: String,
    pub o: f64,
    pub h: f64,
    pub l: f64,
    pub c: f64,
}

impl BarRow {
    fn from_bar(b: &CandleBar) -> Self {
        Self {
            t: format_time(b.timestamp),
            o: b.open,
            h: b.high,
            l: b.low,
            c: b.close,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmaPair {
    pub ema20: Option<f64>,
    pub ema50: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Volatility {
    pub atr14_5m_pips: Option<f64>,
    pub atr14_15m_pips: Option<f64>,
    pub atr14_1h_pips: Option<f64>,
    pub atr14_4h_pips: Option<f64>,
    pub today_range_pips: Option<f64>,
    /// 当日を除く直近最大20日の日次レンジ平均
    pub avg_daily_range_pips: Option<f64>,
    /// 上記の集計に使えた日数（データが少ないときの信頼度の目安）
    pub avg_daily_range_days_sampled: usize,
    pub today_range_vs_avg: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReferenceLevels {
    pub prev_day_high: Option<f64>,
    pub prev_day_low: Option<f64>,
    /// 前日高安の算出に使えた1H足の本数（24未満なら不完全）
    pub prev_day_bars_1h: usize,
    pub today_high: Option<f64>,
    pub today_low: Option<f64>,
    pub asia_session_high: Option<f64>,
    pub asia_session_low: Option<f64>,
    pub round_numbers: Vec<f64>,
    pub ema_1h: EmaPair,
    pub ema_4h: EmaPair,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwingPoints {
    #[serde(rename = "4h")]
    pub h4: Vec<SwingPoint>,
    #[serde(rename = "1h")]
    pub h1: Vec<SwingPoint>,
    #[serde(rename = "15m")]
    pub m15: Vec<SwingPoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenPositionSummary {
    pub side: String,
    pub entry_price: f64,
    pub stop_loss: Option<f64>,
    pub take_profit: Option<f64>,
    pub open_time: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecentDecision {
    pub time: String,
    pub action: String,
    pub summary: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AccountState {
    pub open_positions: Vec<OpenPositionSummary>,
    pub recent_decisions: Vec<RecentDecision>,
}

/// LLM に渡す客観的事実の集合
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketSnapshot {
    pub pair: String,
    pub timestamp: String,
    pub session: Session,
    pub current_price: f64,
    pub spread_pips: f64,
    pub pip_size: f64,
    pub volatility: Volatility,
    pub reference_levels: ReferenceLevels,
    pub swing_points: SwingPoints,
    pub bars_15m: Vec<BarRow>,
    pub bars_5m: Vec<BarRow>,
    pub account_state: AccountState,
    /// 各時間足の最新確定足の時刻。LLM の `observed` と突き合わせる。
    pub latest_bars: LatestBars,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LatestBars {
    #[serde(rename = "4h")]
    pub h4: Option<String>,
    #[serde(rename = "1h")]
    pub h1: Option<String>,
    #[serde(rename = "15m")]
    pub m15: Option<String>,
    #[serde(rename = "5m")]
    pub m5: Option<String>,
}

/// Snapshot 生成の入力
pub struct SnapshotInput<'a> {
    pub pair: &'a str,
    pub bars_4h: &'a [CandleBar],
    pub bars_1h: &'a [CandleBar],
    pub bars_15m: &'a [CandleBar],
    pub bars_5m: &'a [CandleBar],
    pub spread_pips: f64,
    /// None なら 5M 最新足の終値
    pub current_price: Option<f64>,
    /// None なら 5M 最新足の時刻（リプレイ時は評価時刻を明示的に渡す）
    pub now: Option<DateTime<Utc>>,
    pub account_state: AccountState,
}

/// Snapshot 生成の設定
#[derive(Debug, Clone)]
pub struct SnapshotConfig {
    pub raw_bars_15m: usize,
    pub raw_bars_5m: usize,
    pub swing_window: usize,
    pub max_swings_per_tf: usize,
    pub round_number_count: usize,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            raw_bars_15m: 30,
            raw_bars_5m: 30,
            swing_window: 2,
            max_swings_per_tf: 8,
            round_number_count: 5,
        }
    }
}

impl MarketSnapshot {
    pub fn build(input: &SnapshotInput, cfg: &SnapshotConfig) -> Self {
        let pip = get_pip_size(input.pair);
        let latest_5m = input.bars_5m.last();
        let now = input
            .now
            .or_else(|| latest_5m.map(|b| b.timestamp))
            .unwrap_or_else(Utc::now);
        let current_price = input
            .current_price
            .or_else(|| latest_5m.map(|b| b.close))
            .unwrap_or(0.0);

        let pips = |v: Option<f64>| v.map(|x| to_pips(x, pip));

        let today = today_high_low(input.bars_5m, now)
            .or_else(|| today_high_low(input.bars_15m, now))
            .or_else(|| today_high_low(input.bars_1h, now));
        let prev_day = prev_day_high_low(input.bars_1h, now);
        let asia = asia_session_high_low(input.bars_5m, now)
            .or_else(|| asia_session_high_low(input.bars_15m, now));
        let avg_range = avg_daily_range(input.bars_4h, now, 20)
            .or_else(|| avg_daily_range(input.bars_1h, now, 20));

        let today_range = today.map(|hl| hl.high - hl.low);
        let today_range_vs_avg = match (today_range, avg_range) {
            (Some(r), Some((avg, _))) if avg > 0.0 => Some(((r / avg) * 100.0).round() / 100.0),
            _ => None,
        };

        let tail = |v: Vec<SwingPoint>| -> Vec<SwingPoint> {
            let skip = v.len().saturating_sub(cfg.max_swings_per_tf);
            v.into_iter().skip(skip).collect()
        };

        Self {
            pair: input.pair.to_string(),
            timestamp: now.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
            session: session_at(now),
            current_price,
            spread_pips: input.spread_pips,
            pip_size: pip,
            volatility: Volatility {
                atr14_5m_pips: pips(calculate_atr(input.bars_5m, 14)),
                atr14_15m_pips: pips(calculate_atr(input.bars_15m, 14)),
                atr14_1h_pips: pips(calculate_atr(input.bars_1h, 14)),
                atr14_4h_pips: pips(calculate_atr(input.bars_4h, 14)),
                today_range_pips: pips(today_range),
                avg_daily_range_pips: pips(avg_range.map(|(a, _)| a)),
                avg_daily_range_days_sampled: avg_range.map(|(_, d)| d).unwrap_or(0),
                today_range_vs_avg,
            },
            reference_levels: ReferenceLevels {
                prev_day_high: prev_day.map(|(hl, _)| hl.high),
                prev_day_low: prev_day.map(|(hl, _)| hl.low),
                prev_day_bars_1h: prev_day.map(|(_, n)| n).unwrap_or(0),
                today_high: today.map(|hl| hl.high),
                today_low: today.map(|hl| hl.low),
                asia_session_high: asia.map(|hl| hl.high),
                asia_session_low: asia.map(|hl| hl.low),
                round_numbers: if current_price > 0.0 {
                    find_round_numbers(input.pair, current_price, cfg.round_number_count)
                } else {
                    Vec::new()
                },
                ema_1h: EmaPair {
                    ema20: last_ema(input.bars_1h, 20),
                    ema50: last_ema(input.bars_1h, 50),
                },
                ema_4h: EmaPair {
                    ema20: last_ema(input.bars_4h, 20),
                    ema50: last_ema(input.bars_4h, 50),
                },
            },
            swing_points: SwingPoints {
                h4: tail(find_swing_points(input.bars_4h, cfg.swing_window)),
                h1: tail(find_swing_points(input.bars_1h, cfg.swing_window)),
                m15: tail(find_swing_points(input.bars_15m, cfg.swing_window)),
            },
            bars_15m: last_n_rows(input.bars_15m, cfg.raw_bars_15m),
            bars_5m: last_n_rows(input.bars_5m, cfg.raw_bars_5m),
            account_state: input.account_state.clone(),
            latest_bars: LatestBars {
                h4: input.bars_4h.last().map(|b| format_time(b.timestamp)),
                h1: input.bars_1h.last().map(|b| format_time(b.timestamp)),
                m15: input.bars_15m.last().map(|b| format_time(b.timestamp)),
                m5: latest_5m.map(|b| format_time(b.timestamp)),
            },
        }
    }

    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

fn last_n_rows(bars: &[CandleBar], n: usize) -> Vec<BarRow> {
    let start = bars.len().saturating_sub(n);
    bars[start..].iter().map(BarRow::from_bar).collect()
}

/// 時間足ごとの画像パス
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartSet {
    pub dir: PathBuf,
    pub h4: PathBuf,
    pub h1: PathBuf,
    pub m15: PathBuf,
    pub m5: PathBuf,
}

impl ChartSet {
    /// (時間足ラベル, パス) の順序付きリスト
    pub fn ordered(&self) -> [(&'static str, &Path); 4] {
        [
            ("4H", self.h4.as_path()),
            ("1H", self.h1.as_path()),
            ("15M", self.m15.as_path()),
            ("5M", self.m5.as_path()),
        ]
    }

    pub fn by_timeframe(&self, tf: &str) -> Option<&Path> {
        match tf.to_uppercase().as_str() {
            "4H" => Some(&self.h4),
            "1H" => Some(&self.h1),
            "15M" => Some(&self.m15),
            "5M" => Some(&self.m5),
            _ => None,
        }
    }
}

/// Snapshot と画像の組
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotBundle {
    pub snapshot: MarketSnapshot,
    pub charts: ChartSet,
}

/// Snapshot 生成 + 画像4枚の描画をまとめて行う
pub struct SnapshotPipeline {
    plotter: ChartPlotter,
    output_root: PathBuf,
    config: SnapshotConfig,
}

impl SnapshotPipeline {
    pub fn new(output_root: impl AsRef<Path>) -> Self {
        Self {
            plotter: ChartPlotter::with_default(),
            output_root: output_root.as_ref().to_path_buf(),
            config: SnapshotConfig::default(),
        }
    }

    pub fn with_plotter_config(mut self, cfg: ChartPlotterConfig) -> Self {
        self.plotter = ChartPlotter::new(cfg);
        self
    }

    pub fn with_snapshot_config(mut self, cfg: SnapshotConfig) -> Self {
        self.config = cfg;
        self
    }

    /// 画像の出力先: `<root>/<pair>/<YYYYmmdd_HHMMSS>/{4H,1H,15M,5M}.png`
    pub fn build(&self, input: &SnapshotInput) -> Result<SnapshotBundle> {
        let snapshot = MarketSnapshot::build(input, &self.config);

        let stamp = snapshot
            .timestamp
            .replace(" UTC", "")
            .replace(['-', ':'], "")
            .replace(' ', "_");
        let dir = self.output_root.join(&snapshot.pair).join(stamp);
        std::fs::create_dir_all(&dir)?;

        let r = &snapshot.reference_levels;
        let mut common: Vec<PriceLevel> = Vec::new();
        push_level(&mut common, r.prev_day_high, "PDH");
        push_level(&mut common, r.prev_day_low, "PDL");
        push_level(&mut common, r.today_high, "today H");
        push_level(&mut common, r.today_low, "today L");
        for &rn in &r.round_numbers {
            common.push(PriceLevel { price: rn, tag: "round".into() });
        }

        let swing_levels = |pts: &[SwingPoint]| -> Vec<PriceLevel> {
            pts.iter()
                .map(|p| PriceLevel {
                    price: p.price,
                    tag: match p.kind {
                        SwingKind::High => format!("swing H {}", &p.time[5..]),
                        SwingKind::Low => format!("swing L {}", &p.time[5..]),
                    },
                })
                .collect()
        };

        let mut levels_4h = common.clone();
        levels_4h.extend(swing_levels(&snapshot.swing_points.h4));
        let mut levels_1h = common.clone();
        levels_1h.extend(swing_levels(&snapshot.swing_points.h1));
        let mut levels_15m = common.clone();
        levels_15m.extend(swing_levels(&snapshot.swing_points.m15));
        let mut levels_5m = common.clone();
        push_level(&mut levels_5m, r.asia_session_high, "asia H");
        push_level(&mut levels_5m, r.asia_session_low, "asia L");
        levels_5m.extend(swing_levels(&snapshot.swing_points.m15));

        let cp = Some(snapshot.current_price).filter(|p| *p > 0.0);
        let plots = [
            ("4H", input.bars_4h, &levels_4h, dir.join("4H.png")),
            ("1H", input.bars_1h, &levels_1h, dir.join("1H.png")),
            ("15M", input.bars_15m, &levels_15m, dir.join("15M.png")),
            ("5M", input.bars_5m, &levels_5m, dir.join("5M.png")),
        ];
        for (tf, bars, levels, path) in &plots {
            self.plotter.plot_to_file(
                &TimeframeChart {
                    pair: input.pair,
                    timeframe: tf,
                    bars,
                    levels,
                    current_price: cp,
                },
                path,
            )?;
        }

        Ok(SnapshotBundle {
            snapshot,
            charts: ChartSet {
                dir: dir.clone(),
                h4: dir.join("4H.png"),
                h1: dir.join("1H.png"),
                m15: dir.join("15M.png"),
                m5: dir.join("5M.png"),
            },
        })
    }
}

fn push_level(v: &mut Vec<PriceLevel>, price: Option<f64>, tag: &str) {
    if let Some(p) = price {
        v.push(PriceLevel { price: p, tag: tag.to_string() });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone};

    fn make_bars(count: usize, step_min: i64, base: f64, end: DateTime<Utc>) -> Vec<CandleBar> {
        (0..count)
            .map(|i| {
                let t = end - Duration::minutes(step_min * (count as i64 - 1 - i as i64));
                let p = base + ((i as f64) * 0.37).sin() * 0.3;
                CandleBar {
                    timestamp: t,
                    open: p,
                    high: p + 0.12,
                    low: p - 0.12,
                    close: p + 0.04,
                    volume: 100,
                }
            })
            .collect()
    }

    #[test]
    fn snapshot_contains_only_facts_and_no_fabrication() {
        let now = Utc.with_ymd_and_hms(2026, 9, 15, 9, 35, 0).unwrap();
        let b4h = make_bars(60, 240, 154.0, now);
        let b1h = make_bars(60, 60, 154.0, now);
        let b15 = make_bars(60, 15, 154.0, now);
        let b5 = make_bars(60, 5, 154.0, now);

        let snap = MarketSnapshot::build(
            &SnapshotInput {
                pair: "USDJPY",
                bars_4h: &b4h,
                bars_1h: &b1h,
                bars_15m: &b15,
                bars_5m: &b5,
                spread_pips: 0.2,
                current_price: None,
                now: Some(now),
                account_state: AccountState::default(),
            },
            &SnapshotConfig::default(),
        );

        assert_eq!(snap.bars_5m.len(), 30);
        assert_eq!(snap.bars_15m.len(), 30);
        assert!(snap.volatility.atr14_5m_pips.is_some());
        assert!(snap.reference_levels.prev_day_high.is_some());
        assert_eq!(snap.session, Session::London);
        assert_eq!(snap.latest_bars.m5.as_deref(), Some("2026-09-15 09:35"));

        let json = snap.to_json_pretty();
        for forbidden in ["PINBAR", "BULLISH", "BEARISH", "trend", "support", "resistance"] {
            assert!(!json.contains(forbidden), "snapshot leaks a judgment: {forbidden}");
        }
    }

    #[test]
    fn empty_input_yields_nones_not_defaults() {
        let snap = MarketSnapshot::build(
            &SnapshotInput {
                pair: "EURUSD",
                bars_4h: &[],
                bars_1h: &[],
                bars_15m: &[],
                bars_5m: &[],
                spread_pips: 0.3,
                current_price: None,
                now: None,
                account_state: AccountState::default(),
            },
            &SnapshotConfig::default(),
        );
        assert!(snap.reference_levels.prev_day_high.is_none());
        assert!(snap.reference_levels.round_numbers.is_empty());
        assert!(snap.volatility.atr14_4h_pips.is_none());
        assert!(snap.swing_points.h4.is_empty());
    }

    #[test]
    fn pipeline_writes_four_images() {
        let now = Utc.with_ymd_and_hms(2026, 9, 15, 9, 35, 0).unwrap();
        let b4h = make_bars(60, 240, 154.0, now);
        let b1h = make_bars(60, 60, 154.0, now);
        let b15 = make_bars(60, 15, 154.0, now);
        let b5 = make_bars(60, 5, 154.0, now);
        let root = std::env::temp_dir().join(format!("ntrade_snapshot_{}", std::process::id()));

        let bundle = SnapshotPipeline::new(&root)
            .build(&SnapshotInput {
                pair: "USDJPY",
                bars_4h: &b4h,
                bars_1h: &b1h,
                bars_15m: &b15,
                bars_5m: &b5,
                spread_pips: 0.2,
                current_price: None,
                now: Some(now),
                account_state: AccountState::default(),
            })
            .unwrap();

        for (_, p) in bundle.charts.ordered() {
            assert!(p.exists(), "missing {:?}", p);
        }
        assert!(bundle.charts.by_timeframe("15m").is_some());
        let _ = std::fs::remove_dir_all(root);
    }
}
