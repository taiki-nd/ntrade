//! 客観的な測定関数群。
//!
//! ここにある関数は「再現可能な測定値」だけを返す。
//! 「トレンドである」「支持線である」「ピンバーである」といった解釈は行わない。

use chrono::{DateTime, Datelike, Timelike, Utc};
use serde::{Deserialize, Serialize};

use crate::ctrader::CandleBar;

/// 通貨ペアに応じた1 pipの価格幅を返す
pub fn get_pip_size(pair: &str) -> f64 {
    let p = pair.to_uppercase();
    if p.contains("JPY") {
        0.01
    } else if p.contains("XAU") || p.contains("GOLD") {
        0.1
    } else {
        0.0001
    }
}

/// 通貨ペアに応じた価格表示桁数
pub fn price_digits(pair: &str) -> usize {
    let p = pair.to_uppercase();
    if p.contains("JPY") {
        3
    } else if p.contains("XAU") || p.contains("GOLD") {
        2
    } else {
        5
    }
}

/// 指数平滑移動平均（EMA）を計算する
pub fn calculate_ema(bars: &[CandleBar], period: usize) -> Vec<Option<f64>> {
    let n = bars.len();
    let mut ema = vec![None; n];

    if n < period || period == 0 {
        return ema;
    }

    let sum: f64 = bars.iter().take(period).map(|b| b.close).sum();
    let initial_sma = sum / period as f64;
    ema[period - 1] = Some(initial_sma);

    let multiplier = 2.0 / (period as f64 + 1.0);
    let mut prev_ema = initial_sma;

    for i in period..n {
        let current_ema = (bars[i].close - prev_ema) * multiplier + prev_ema;
        ema[i] = Some(current_ema);
        prev_ema = current_ema;
    }

    ema
}

/// 最後のEMA値
pub fn last_ema(bars: &[CandleBar], period: usize) -> Option<f64> {
    calculate_ema(bars, period).last().copied().flatten()
}

/// ATR（Average True Range, Wilder 平滑化）の最新値。足が足りなければ None。
pub fn calculate_atr(bars: &[CandleBar], period: usize) -> Option<f64> {
    if period == 0 || bars.len() < period + 1 {
        return None;
    }

    let true_ranges: Vec<f64> = bars
        .windows(2)
        .map(|w| {
            let prev_close = w[0].close;
            let b = &w[1];
            (b.high - b.low)
                .max((b.high - prev_close).abs())
                .max((b.low - prev_close).abs())
        })
        .collect();

    let mut atr: f64 = true_ranges.iter().take(period).sum::<f64>() / period as f64;
    for tr in true_ranges.iter().skip(period) {
        atr = (atr * (period as f64 - 1.0) + tr) / period as f64;
    }
    Some(atr)
}

/// スイング点の種別
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SwingKind {
    High,
    Low,
}

/// スイング高値・安値（時刻付き）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwingPoint {
    pub time: String,
    #[serde(rename = "type")]
    pub kind: SwingKind,
    pub price: f64,
}

/// スイング高値・安値を検出する（左右 window 本の中で極値となる足）。
/// 最新 window 本はまだ確定できないため対象外となる。時刻順に返す。
pub fn find_swing_points(bars: &[CandleBar], window: usize) -> Vec<SwingPoint> {
    let mut points = Vec::new();
    if window == 0 || bars.len() < window * 2 + 1 {
        return points;
    }

    for i in window..(bars.len() - window) {
        let current = &bars[i];
        let mut is_high = true;
        let mut is_low = true;

        for j in (i - window)..=(i + window) {
            if i == j {
                continue;
            }
            if bars[j].high >= current.high {
                is_high = false;
            }
            if bars[j].low <= current.low {
                is_low = false;
            }
        }

        if is_high {
            points.push(SwingPoint {
                time: format_time(current.timestamp),
                kind: SwingKind::High,
                price: current.high,
            });
        }
        if is_low {
            points.push(SwingPoint {
                time: format_time(current.timestamp),
                kind: SwingKind::Low,
                price: current.low,
            });
        }
    }

    points
}

/// キリ番（Round numbers）の候補を現在価格の周辺から算出する
pub fn find_round_numbers(pair: &str, current_price: f64, count: usize) -> Vec<f64> {
    let p = pair.to_uppercase();
    let step = if p.contains("JPY") {
        0.50
    } else if p.contains("XAU") {
        5.0
    } else {
        0.0050
    };

    let base = (current_price / step).round() * step;
    let half = (count / 2) as i32;
    let mut numbers: Vec<f64> = (-half..=half)
        .map(|i| {
            let val = base + (i as f64 * step);
            (val * 100_000.0).round() / 100_000.0
        })
        .collect();
    numbers.sort_by(|a, b| a.partial_cmp(b).unwrap());
    numbers
}

/// 取引セッション（UTC時刻から機械的に決まる）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Session {
    Tokyo,
    London,
    LondonNy,
    Ny,
    Off,
}

pub fn session_at(t: DateTime<Utc>) -> Session {
    match t.hour() {
        0..=6 => Session::Tokyo,
        7..=11 => Session::London,
        12..=15 => Session::LondonNy,
        16..=20 => Session::Ny,
        _ => Session::Off,
    }
}

/// 高値・安値の組
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HighLow {
    pub high: f64,
    pub low: f64,
}

fn high_low_of<'a>(bars: impl Iterator<Item = &'a CandleBar>) -> Option<HighLow> {
    let mut acc: Option<HighLow> = None;
    for b in bars {
        acc = Some(match acc {
            None => HighLow { high: b.high, low: b.low },
            Some(hl) => HighLow {
                high: hl.high.max(b.high),
                low: hl.low.min(b.low),
            },
        });
    }
    acc
}

fn same_utc_date(a: DateTime<Utc>, b: DateTime<Utc>) -> bool {
    a.year() == b.year() && a.ordinal() == b.ordinal()
}

/// `now` と同じUTC日付の足から高安を求める。該当足がなければ None。
pub fn today_high_low(bars: &[CandleBar], now: DateTime<Utc>) -> Option<HighLow> {
    high_low_of(bars.iter().filter(|b| same_utc_date(b.timestamp, now)))
}

/// 前日（`now` より前で最も新しいUTC日付）の高安と、その日の足数
pub fn prev_day_high_low(bars: &[CandleBar], now: DateTime<Utc>) -> Option<(HighLow, usize)> {
    let prev_date = bars
        .iter()
        .filter(|b| !same_utc_date(b.timestamp, now) && b.timestamp < now)
        .map(|b| b.timestamp.date_naive())
        .max()?;

    let day_bars: Vec<&CandleBar> = bars
        .iter()
        .filter(|b| b.timestamp.date_naive() == prev_date)
        .collect();
    let hl = high_low_of(day_bars.iter().copied())?;
    Some((hl, day_bars.len()))
}

/// アジア時間（UTC 00:00〜06:59）の当日高安
pub fn asia_session_high_low(bars: &[CandleBar], now: DateTime<Utc>) -> Option<HighLow> {
    high_low_of(
        bars.iter()
            .filter(|b| same_utc_date(b.timestamp, now) && b.timestamp.hour() < 7),
    )
}

/// 当日を除く直近 `max_days` 日分の日次レンジ平均と、実際に集計した日数
pub fn avg_daily_range(bars: &[CandleBar], now: DateTime<Utc>, max_days: usize) -> Option<(f64, usize)> {
    use std::collections::BTreeMap;

    let mut by_day: BTreeMap<chrono::NaiveDate, HighLow> = BTreeMap::new();
    for b in bars.iter().filter(|b| !same_utc_date(b.timestamp, now)) {
        by_day
            .entry(b.timestamp.date_naive())
            .and_modify(|hl| {
                hl.high = hl.high.max(b.high);
                hl.low = hl.low.min(b.low);
            })
            .or_insert(HighLow { high: b.high, low: b.low });
    }

    let ranges: Vec<f64> = by_day
        .values()
        .rev()
        .take(max_days)
        .map(|hl| hl.high - hl.low)
        .collect();
    if ranges.is_empty() {
        return None;
    }
    Some((ranges.iter().sum::<f64>() / ranges.len() as f64, ranges.len()))
}

/// 小数第1位に丸めた pips 換算
pub fn to_pips(value: f64, pip_size: f64) -> f64 {
    ((value / pip_size) * 10.0).round() / 10.0
}

pub fn format_time(t: DateTime<Utc>) -> String {
    t.format("%Y-%m-%d %H:%M").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone};

    fn bar(t: DateTime<Utc>, o: f64, h: f64, l: f64, c: f64) -> CandleBar {
        CandleBar { timestamp: t, open: o, high: h, low: l, close: c, volume: 1 }
    }

    fn base() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 9, 15, 0, 0, 0).unwrap()
    }

    #[test]
    fn ema_has_values_after_period() {
        let bars: Vec<CandleBar> = (0..25)
            .map(|i| bar(base() + Duration::minutes(i * 5), 100.0 + i as f64, 101.0 + i as f64, 99.0 + i as f64, 100.5 + i as f64))
            .collect();
        let ema20 = calculate_ema(&bars, 20);
        assert!(ema20[18].is_none());
        assert!(ema20[19].is_some());
    }

    #[test]
    fn atr_requires_period_plus_one_bars() {
        let bars: Vec<CandleBar> = (0..14)
            .map(|i| bar(base() + Duration::minutes(i * 5), 1.0, 1.1, 0.9, 1.0))
            .collect();
        assert!(calculate_atr(&bars, 14).is_none());
        let bars: Vec<CandleBar> = (0..15)
            .map(|i| bar(base() + Duration::minutes(i * 5), 1.0, 1.1, 0.9, 1.0))
            .collect();
        let atr = calculate_atr(&bars, 14).unwrap();
        assert!((atr - 0.2).abs() < 1e-9);
    }

    #[test]
    fn swing_points_exclude_unconfirmed_tail() {
        let mut bars: Vec<CandleBar> = (0..7)
            .map(|i| bar(base() + Duration::minutes(i * 5), 1.0, 1.1, 0.9, 1.0))
            .collect();
        bars[3].high = 1.5; // 確定したスイング高値
        bars[6].high = 2.0; // 最新足: 右側が無いので確定不可
        let pts = find_swing_points(&bars, 2);
        assert_eq!(pts.len(), 1);
        assert_eq!(pts[0].kind, SwingKind::High);
        assert_eq!(pts[0].price, 1.5);
    }

    #[test]
    fn day_partition_and_sessions() {
        let now = Utc.with_ymd_and_hms(2026, 9, 15, 9, 30, 0).unwrap();
        let mut bars = Vec::new();
        // 前日 (9/14) 3本
        for h in [10, 14, 20] {
            bars.push(bar(Utc.with_ymd_and_hms(2026, 9, 14, h, 0, 0).unwrap(), 1.0, 1.2, 0.8, 1.0));
        }
        // 当日 (9/15) アジア時間 2本 + ロンドン 1本
        bars.push(bar(Utc.with_ymd_and_hms(2026, 9, 15, 1, 0, 0).unwrap(), 1.0, 1.05, 0.95, 1.0));
        bars.push(bar(Utc.with_ymd_and_hms(2026, 9, 15, 5, 0, 0).unwrap(), 1.0, 1.06, 0.94, 1.0));
        bars.push(bar(Utc.with_ymd_and_hms(2026, 9, 15, 8, 0, 0).unwrap(), 1.0, 1.30, 0.70, 1.0));

        let (prev, n) = prev_day_high_low(&bars, now).unwrap();
        assert_eq!(n, 3);
        assert_eq!(prev.high, 1.2);
        let asia = asia_session_high_low(&bars, now).unwrap();
        assert_eq!(asia.high, 1.06);
        assert_eq!(asia.low, 0.94);
        let today = today_high_low(&bars, now).unwrap();
        assert_eq!(today.high, 1.30);
        assert_eq!(session_at(now), Session::London);
        let (avg, days) = avg_daily_range(&bars, now, 20).unwrap();
        assert_eq!(days, 1);
        assert!((avg - 0.4).abs() < 1e-9);
    }

    #[test]
    fn empty_inputs_yield_none_not_fabricated_values() {
        let now = base();
        assert!(today_high_low(&[], now).is_none());
        assert!(prev_day_high_low(&[], now).is_none());
        assert!(asia_session_high_low(&[], now).is_none());
        assert!(avg_daily_range(&[], now, 20).is_none());
        assert!(calculate_atr(&[], 14).is_none());
        assert!(find_swing_points(&[], 2).is_empty());
    }
}
