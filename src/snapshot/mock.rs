//! cTrader に接続できないときの疑似相場データ。
//!
//! 特定のパターン（ピンバーなど）を意図的に作り込まない。判定ロジックが無くなった以上、
//! 「このパターンが出る」前提のデータは検証の役に立たないため。

use chrono::{DateTime, Duration, Utc};

use crate::ctrader::CandleBar;

/// 4H / 1H / 15M / 5M の擬似バーを生成する（末尾が `now` に揃う）
pub fn simulate_multi_timeframe(
    now: DateTime<Utc>,
    base_price: f64,
) -> (Vec<CandleBar>, Vec<CandleBar>, Vec<CandleBar>, Vec<CandleBar>) {
    (
        simulate(now, base_price, 60, 240, 0.35, 0.0015),
        simulate(now, base_price, 60, 60, 0.18, 0.0008),
        simulate(now, base_price, 60, 15, 0.09, 0.0004),
        simulate(now, base_price, 60, 5, 0.05, 0.0002),
    )
}

fn simulate(
    now: DateTime<Utc>,
    base: f64,
    count: usize,
    step_min: i64,
    amplitude: f64,
    drift: f64,
) -> Vec<CandleBar> {
    let mut bars = Vec::with_capacity(count);
    let mut price = base;
    for i in 0..count {
        let t = now - Duration::minutes(step_min * (count as i64 - 1 - i as i64));
        let wave = ((i as f64) * 0.41).sin() * amplitude * 0.3
            + ((i as f64) * 0.13).cos() * amplitude * 0.2;
        let open = price;
        let close = open + drift * base + wave;
        let spread = amplitude * 0.25;
        let high = open.max(close) + spread * (0.5 + ((i as f64) * 0.7).sin().abs());
        let low = open.min(close) - spread * (0.5 + ((i as f64) * 0.9).cos().abs());
        price = close;
        bars.push(CandleBar {
            timestamp: t,
            open,
            high,
            low,
            close,
            volume: 100 + (i as i64 % 7) * 30,
        });
    }
    bars
}
