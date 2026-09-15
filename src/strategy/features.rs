use chrono::{DateTime, Utc};

use crate::ctrader::CandleBar;
use crate::strategy::types::{
    CurrentBarFeatures, FiveMinutePriceAction, MarketStructure, PriceActionInput,
};

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

/// 指数平滑移動平均（EMA）を計算する
pub fn calculate_ema(bars: &[CandleBar], period: usize) -> Vec<Option<f64>> {
    let n = bars.len();
    let mut ema = vec![None; n];

    if n < period || period == 0 {
        return ema;
    }

    // 最初の period 本で単純移動平均（SMA）を計算
    let sum: f64 = bars.iter().take(period).map(|b| b.close).sum();
    let initial_sma = sum / period as f64;
    ema[period - 1] = Some(initial_sma);

    // 平滑化係数
    let multiplier = 2.0 / (period as f64 + 1.0);
    let mut prev_ema = initial_sma;

    for i in period..n {
        let current_close = bars[i].close;
        let current_ema = (current_close - prev_ema) * multiplier + prev_ema;
        ema[i] = Some(current_ema);
        prev_ema = current_ema;
    }

    ema
}

/// スイング高値・安値を検出する（左右 window 本の中で極値となる足）
pub fn find_swing_points(bars: &[CandleBar], window: usize) -> (Vec<f64>, Vec<f64>) {
    let mut swing_highs = Vec::new();
    let mut swing_lows = Vec::new();

    if bars.len() < window * 2 + 1 {
        return (swing_highs, swing_lows);
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
            swing_highs.push(current.high);
        }
        if is_low {
            swing_lows.push(current.low);
        }
    }

    (swing_highs, swing_lows)
}

/// キリ番（Round numbers）の候補を算出する
pub fn find_round_numbers(pair: &str, current_price: f64, count: usize) -> Vec<f64> {
    let step = if pair.to_uppercase().contains("JPY") {
        0.50
    } else if pair.to_uppercase().contains("XAU") {
        5.0
    } else {
        0.0050
    };

    let base = (current_price / step).round() * step;
    let mut numbers = Vec::new();

    let half = (count / 2) as i32;
    for i in -half..=half {
        let val = base + (i as f64 * step);
        numbers.push((val * 100_000.0).round() / 100_000.0);
    }
    numbers.sort_by(|a, b| a.partial_cmp(b).unwrap());
    numbers
}

/// マルチタイムフレーム入力データ構造体
#[derive(Debug, Clone)]
pub struct MultiTimeframeBars<'a> {
    pub pair: &'a str,
    pub bars_4h: &'a [CandleBar],
    pub bars_1h: &'a [CandleBar],
    pub bars_15m: &'a [CandleBar],
    pub bars_5m: &'a [CandleBar],
    pub spread_pips: f64,
    pub current_price: Option<f64>,
    pub timestamp: Option<DateTime<Utc>>,
}

/// プライスアクション分析エンジン
pub struct PriceActionAnalyzer;

impl PriceActionAnalyzer {
    /// マルチタイムフレームデータから PriceActionInput を算出・生成する
    pub fn analyze(data: &MultiTimeframeBars) -> PriceActionInput {
        let pip_size = get_pip_size(data.pair);

        let latest_5m = data.bars_5m.last().cloned().unwrap_or_else(|| CandleBar {
            timestamp: Utc::now(),
            open: 0.0,
            high: 0.0,
            low: 0.0,
            close: 0.0,
            volume: 0,
        });

        let current_price = data.current_price.unwrap_or(latest_5m.close);
        let timestamp = data
            .timestamp
            .unwrap_or(latest_5m.timestamp)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();

        // 1. 環境認識 (4H / 1H)
        let market_structure = Self::analyze_market_structure(
            data.pair,
            data.bars_4h,
            data.bars_1h,
            current_price,
            pip_size,
        );

        // 2. 5分足プライスアクション (5M)
        let five_minute_pa = Self::analyze_five_minute_pa(
            data.bars_5m,
            data.bars_15m,
            data.bars_1h,
            current_price,
            pip_size,
            &market_structure,
        );

        PriceActionInput {
            pair: data.pair.to_string(),
            timestamp,
            spread_pips: data.spread_pips,
            current_price,
            market_structure,
            five_minute_pa,
        }
    }

    /// 上位足（4H/1H）の環境認識を分析
    fn analyze_market_structure(
        pair: &str,
        bars_4h: &[CandleBar],
        bars_1h: &[CandleBar],
        current_price: f64,
        pip_size: f64,
    ) -> MarketStructure {
        // --- 4H分析 ---
        let (highs_4h, lows_4h) = find_swing_points(bars_4h, 2);
        let ema20_4h = calculate_ema(bars_4h, 20);
        let ema50_4h = calculate_ema(bars_4h, 50);

        // 4H トレンド判定 (スイング高安値 + EMA)
        let trend_4h = if highs_4h.len() >= 2 && lows_4h.len() >= 2 {
            let last_high = highs_4h[highs_4h.len() - 1];
            let prev_high = highs_4h[highs_4h.len() - 2];
            let last_low = lows_4h[lows_4h.len() - 1];
            let prev_low = lows_4h[lows_4h.len() - 2];

            if last_high > prev_high && last_low > prev_low {
                "BULLISH (Higher Highs / Higher Lows)".to_string()
            } else if last_high < prev_high && last_low < prev_low {
                "BEARISH (Lower Highs / Lower Lows)".to_string()
            } else {
                "RANGE / CONSOLIDATION".to_string()
            }
        } else {
            // スイング点が足りない場合は直近EMA関係から推定
            let last_ema20 = ema20_4h.last().and_then(|v| *v);
            let last_ema50 = ema50_4h.last().and_then(|v| *v);

            match (last_ema20, last_ema50) {
                (Some(e20), Some(e50)) if current_price > e20 && e20 > e50 => {
                    "BULLISH (Above EMA20/50)".to_string()
                }
                (Some(e20), Some(e50)) if current_price < e20 && e20 < e50 => {
                    "BEARISH (Below EMA20/50)".to_string()
                }
                _ => "RANGE / CONSOLIDATION".to_string(),
            }
        };

        // 4H サポレジ探索 (スイング高安値 + キリ番)
        let round_numbers = find_round_numbers(pair, current_price, 6);
        let mut resistance_candidates = Vec::new();
        let mut support_candidates = Vec::new();

        for &h in &highs_4h {
            if h > current_price {
                resistance_candidates.push(h);
            } else {
                support_candidates.push(h);
            }
        }
        for &l in &lows_4h {
            if l > current_price {
                resistance_candidates.push(l);
            } else {
                support_candidates.push(l);
            }
        }
        for &r in &round_numbers {
            if r > current_price {
                resistance_candidates.push(r);
            } else {
                support_candidates.push(r);
            }
        }

        resistance_candidates.sort_by(|a, b| a.partial_cmp(b).unwrap());
        support_candidates.sort_by(|a, b| b.partial_cmp(a).unwrap());

        let nearest_h4_resistance = resistance_candidates
            .first()
            .copied()
            .unwrap_or(current_price + (50.0 * pip_size));
        let nearest_h4_support = support_candidates
            .first()
            .copied()
            .unwrap_or(current_price - (50.0 * pip_size));

        // --- 1H分析 ---
        let ema20_1h = calculate_ema(bars_1h, 20);
        let ema50_1h = calculate_ema(bars_1h, 50);
        let last_ema20_1h = ema20_1h.last().and_then(|v| *v);
        let last_ema50_1h = ema50_1h.last().and_then(|v| *v);

        let trend_1h = match (last_ema20_1h, last_ema50_1h) {
            (Some(e20), Some(e50)) => {
                let dist_pips = (current_price - e20).abs() / pip_size;
                if e20 > e50 {
                    if dist_pips <= 10.0 {
                        "BULLISH_PULLBACK_TO_EMA20".to_string()
                    } else if current_price > e20 {
                        "BULLISH_IMPULSE".to_string()
                    } else {
                        "BULLISH_CORRECTION_BELOW_EMA20".to_string()
                    }
                } else if e20 < e50 {
                    if dist_pips <= 10.0 {
                        "BEARISH_PULLBACK_TO_EMA20".to_string()
                    } else if current_price < e20 {
                        "BEARISH_IMPULSE".to_string()
                    } else {
                        "BEARISH_CORRECTION_ABOVE_EMA20".to_string()
                    }
                } else {
                    "RANGE_BOUND".to_string()
                }
            }
            _ => "NEUTRAL".to_string(),
        };

        MarketStructure {
            trend_4h,
            nearest_h4_resistance,
            nearest_h4_support,
            trend_1h,
        }
    }

    /// 5分足のプライスアクションを分析
    fn analyze_five_minute_pa(
        bars_5m: &[CandleBar],
        bars_15m: &[CandleBar],
        bars_1h: &[CandleBar],
        current_price: f64,
        pip_size: f64,
        market_structure: &MarketStructure,
    ) -> FiveMinutePriceAction {
        if bars_5m.is_empty() {
            return FiveMinutePriceAction {
                current_bar: CurrentBarFeatures {
                    bar_type: "NONE".to_string(),
                    direction: "NEUTRAL".to_string(),
                    total_range_pips: 0.0,
                    lower_wick_ratio: 0.0,
                    upper_wick_ratio: 0.0,
                    body_ratio: 0.0,
                    rejection_level: None,
                },
                pattern_detected: "NONE".to_string(),
                at_key_level: "NONE".to_string(),
            };
        }

        let current = bars_5m.last().unwrap();
        let prev = if bars_5m.len() >= 2 {
            Some(&bars_5m[bars_5m.len() - 2])
        } else {
            None
        };

        // 幾何学的特徴量
        let total_range = current.high - current.low;
        let body = (current.close - current.open).abs();
        let upper_wick = current.high - current.close.max(current.open);
        let lower_wick = current.close.min(current.open) - current.low;

        let total_range_pips = if pip_size > 0.0 {
            (total_range / pip_size * 10.0).round() / 10.0
        } else {
            0.0
        };

        let body_ratio = if total_range > 0.0 {
            ((body / total_range) * 100.0).round() / 100.0
        } else {
            0.0
        };

        let upper_wick_ratio = if total_range > 0.0 {
            ((upper_wick / total_range) * 100.0).round() / 100.0
        } else {
            0.0
        };

        let lower_wick_ratio = if total_range > 0.0 {
            ((lower_wick / total_range) * 100.0).round() / 100.0
        } else {
            0.0
        };

        let is_bullish = current.close >= current.open;
        let direction = if is_bullish { "BULLISH" } else { "BEARISH" }.to_string();

        // ローソク足タイプ判定
        let is_pinbar = lower_wick_ratio >= 0.55 || upper_wick_ratio >= 0.55;
        let is_engulfing = if let Some(p) = prev {
            let prev_bullish = p.close >= p.open;
            let current_bullish = is_bullish;
            // 逆方向かつ実体が前足を完全に包んでいる
            if prev_bullish != current_bullish {
                let current_body_top = current.open.max(current.close);
                let current_body_bottom = current.open.min(current.close);
                let prev_body_top = p.open.max(p.close);
                let prev_body_bottom = p.open.min(p.close);

                current_body_top >= prev_body_top && current_body_bottom <= prev_body_bottom
            } else {
                false
            }
        } else {
            false
        };

        let bar_type = if is_pinbar {
            "PINBAR".to_string()
        } else if is_engulfing {
            "ENGULFING".to_string()
        } else if body_ratio <= 0.12 {
            "DOJI".to_string()
        } else {
            "STANDARD".to_string()
        };

        let rejection_level = if lower_wick_ratio >= 0.55 {
            Some(current.low)
        } else if upper_wick_ratio >= 0.55 {
            Some(current.high)
        } else {
            None
        };

        let current_bar = CurrentBarFeatures {
            bar_type,
            direction,
            total_range_pips,
            lower_wick_ratio,
            upper_wick_ratio,
            body_ratio,
            rejection_level,
        };

        // キーレベル判定
        let mut key_level_desc = "NONE".to_string();
        let h4_sup_dist = (current_price - market_structure.nearest_h4_support).abs() / pip_size;
        let h4_res_dist = (market_structure.nearest_h4_resistance - current_price).abs() / pip_size;

        // 15M スイング高安値
        let (highs_15m, lows_15m) = find_swing_points(bars_15m, 2);
        let ema20_1h = calculate_ema(bars_1h, 20);
        let last_ema20_1h = ema20_1h.last().and_then(|v| *v);

        if h4_sup_dist <= 8.0 {
            key_level_desc = format!("TESTING_H4_KEY_SUPPORT_{:.3}", market_structure.nearest_h4_support);
        } else if h4_res_dist <= 8.0 {
            key_level_desc = format!("TESTING_H4_KEY_RESISTANCE_{:.3}", market_structure.nearest_h4_resistance);
        } else if let Some(e20) = last_ema20_1h {
            let dist = (current_price - e20).abs() / pip_size;
            if dist <= 6.0 {
                key_level_desc = "AT_1H_EMA20_DYNAMIC_LEVEL".to_string();
            }
        }

        if key_level_desc == "NONE" {
            // 15M 直近サポレジチェック
            for &l in lows_15m.iter().rev().take(3) {
                if (current_price - l).abs() / pip_size <= 5.0 {
                    key_level_desc = "TESTING_15M_PREVIOUS_SWING_SUPPORT".to_string();
                    break;
                }
            }
            if key_level_desc == "NONE" {
                for &h in highs_15m.iter().rev().take(3) {
                    if (current_price - h).abs() / pip_size <= 5.0 {
                        key_level_desc = "TESTING_15M_PREVIOUS_SWING_RESISTANCE".to_string();
                        break;
                    }
                }
            }
        }

        // パターン検出 (ピンバー、包み足、フェイクアウト)
        let mut pattern_detected = "NONE".to_string();

        if is_pinbar {
            if lower_wick_ratio >= 0.55 {
                pattern_detected = "SUPPORT_REJECTION_PINBAR".to_string();
            } else if upper_wick_ratio >= 0.55 {
                pattern_detected = "RESISTANCE_REJECTION_PINBAR".to_string();
            }
        } else if is_engulfing {
            if is_bullish {
                pattern_detected = "BULLISH_ENGULFING".to_string();
            } else {
                pattern_detected = "BEARISH_ENGULFING".to_string();
            }
        } else {
            // フェイクアウト判定 (直近10本の最高値/最安値を瞬間ブレイク後に反転)
            let lookback = 12.min(bars_5m.len().saturating_sub(1));
            if lookback >= 4 {
                let past_bars = &bars_5m[(bars_5m.len() - 1 - lookback)..(bars_5m.len() - 1)];
                let past_high = past_bars.iter().map(|b| b.high).fold(f64::MIN, f64::max);
                let past_low = past_bars.iter().map(|b| b.low).fold(f64::MAX, f64::min);

                // 安値ブレイク後の急反発 (Bullish Fakeout)
                if current.low < past_low && current.close > past_low && is_bullish {
                    pattern_detected = "BULLISH_FAKEOUT".to_string();
                }
                // 高値ブレイク後の急反落 (Bearish Fakeout)
                else if current.high > past_high && current.close < past_high && !is_bullish {
                    pattern_detected = "BEARISH_FAKEOUT".to_string();
                }
            }
        }

        FiveMinutePriceAction {
            current_bar,
            pattern_detected,
            at_key_level: key_level_desc,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    fn make_candle(time_offset_mins: i64, open: f64, high: f64, low: f64, close: f64) -> CandleBar {
        let now = Utc::now();
        CandleBar {
            timestamp: now + Duration::minutes(time_offset_mins),
            open,
            high,
            low,
            close,
            volume: 100,
        }
    }

    #[test]
    fn test_ema_calculation() {
        let bars: Vec<CandleBar> = (0..25)
            .map(|i| make_candle(i * 5, 100.0 + i as f64, 101.0 + i as f64, 99.0 + i as f64, 100.5 + i as f64))
            .collect();

        let ema20 = calculate_ema(&bars, 20);
        assert_eq!(ema20.len(), 25);
        assert!(ema20[18].is_none());
        assert!(ema20[19].is_some());
        assert!(ema20[24].is_some());
    }

    #[test]
    fn test_pinbar_detection() {
        // 下ヒゲが非常に長い強気ピンバー (下ヒゲ率 >= 60%)
        // open: 154.20, close: 154.25 (実体 0.05)
        // high: 154.28 (上ヒゲ 0.03)
        // low: 154.00 (下ヒゲ 0.20)
        // range: 0.28, lower_wick: 0.20 / 0.28 = ~71.4%
        let pinbar = make_candle(0, 154.20, 154.28, 154.00, 154.25);
        let bars_5m = vec![
            make_candle(-10, 154.10, 154.30, 154.05, 154.15),
            make_candle(-5, 154.15, 154.25, 154.10, 154.18),
            pinbar,
        ];

        let mtf = MultiTimeframeBars {
            pair: "USDJPY",
            bars_4h: &[],
            bars_1h: &[],
            bars_15m: &[],
            bars_5m: &bars_5m,
            spread_pips: 0.2,
            current_price: None,
            timestamp: None,
        };

        let result = PriceActionAnalyzer::analyze(&mtf);
        assert_eq!(result.five_minute_pa.current_bar.bar_type, "PINBAR");
        assert_eq!(result.five_minute_pa.current_bar.direction, "BULLISH");
        assert!(result.five_minute_pa.current_bar.lower_wick_ratio >= 0.60);
        assert_eq!(
            result.five_minute_pa.pattern_detected,
            "SUPPORT_REJECTION_PINBAR"
        );
    }

    #[test]
    fn test_engulfing_detection() {
        // 直前: 陰線 (154.30 -> 154.15)
        let prev = make_candle(-5, 154.30, 154.35, 154.10, 154.15);
        // 現在: 直前の実体を完全に包み込む大陽線 (154.10 -> 154.40)
        let curr = make_candle(0, 154.10, 154.45, 154.08, 154.40);
        let bars_5m = vec![prev, curr];

        let mtf = MultiTimeframeBars {
            pair: "USDJPY",
            bars_4h: &[],
            bars_1h: &[],
            bars_15m: &[],
            bars_5m: &bars_5m,
            spread_pips: 0.2,
            current_price: None,
            timestamp: None,
        };

        let result = PriceActionAnalyzer::analyze(&mtf);
        assert_eq!(result.five_minute_pa.current_bar.bar_type, "ENGULFING");
        assert_eq!(result.five_minute_pa.current_bar.direction, "BULLISH");
        assert_eq!(result.five_minute_pa.pattern_detected, "BULLISH_ENGULFING");
    }

    #[test]
    fn test_fakeout_detection() {
        // 直近安値 154.00 を割り込む 153.95 まで下落したが、終値 154.05 で陽線引け
        let mut bars = Vec::new();
        for i in 0..10 {
            bars.push(make_candle((i - 10) * 5, 154.10, 154.20, 154.00, 154.12));
        }
        // 安値更新後にレンジ内に回帰する足
        bars.push(make_candle(0, 153.98, 154.10, 153.90, 154.05));

        let mtf = MultiTimeframeBars {
            pair: "USDJPY",
            bars_4h: &[],
            bars_1h: &[],
            bars_15m: &[],
            bars_5m: &bars,
            spread_pips: 0.2,
            current_price: None,
            timestamp: None,
        };

        let result = PriceActionAnalyzer::analyze(&mtf);
        assert_eq!(result.five_minute_pa.pattern_detected, "BULLISH_FAKEOUT");
    }
}
