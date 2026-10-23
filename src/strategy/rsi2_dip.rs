//! RSI(2) の押し目買い（Connors の短期逆張り）。米国株価指数 CFD の 30 分足で使う。
//!
//! - 16:00 の引けで日足の RSI(`rsi_period`) を計算し、`threshold` 未満なら成行で買う
//! - 次の日以降の 16:00 に RSI が `threshold` 以上なら決済、まだ未満なら持ち続ける（決済して買い直すのと同じで、コストが掛からない）
//! - SL は終値から ATR × `sl_atr`（夜間の急落への備え）
//! - 日足は現物時間の引け（16:00 NY）の終値で作る。CFD の日足（ロールオーバー基準）は使わない

use anyhow::Result;

use super::types::{Action, Entry, OrderIntent, ParamSpec, Side, Strategy, StrategyContext, StrategyInfo};
use super::us_cash::{self, BARS_PER_DAY, SLOTS};
use crate::indicators::calculate_atr;

pub const NAME: &str = "rsi2_dip";

pub fn info() -> StrategyInfo {
    StrategyInfo {
        name: NAME,
        title: "RSI(2) 押し目買い（米国株価指数・30 分足）",
        description: "16:00 の引けで日足 RSI(2) が閾値未満なら買い、RSI が閾値以上に戻った日の引けで決済。日をまたいで持つのでスワップが掛かる",
        params: vec![
            ParamSpec { name: "rsi_period", default: 2.0, min: 2.0, max: 30.0, integer: true, description: "日足 RSI の期間" },
            ParamSpec { name: "threshold", default: 10.0, min: 1.0, max: 50.0, integer: false, description: "買う RSI の上限" },
            ParamSpec { name: "atr_period", default: 14.0, min: 2.0, max: 200.0, integer: true, description: "SL に使う ATR の期間" },
            ParamSpec { name: "sl_atr", default: 12.0, min: 0.5, max: 30.0, integer: false, description: "SL 幅（ATR の倍数）。狭いと押し目の途中で切られて優位性が消える" },
        ],
    }
}

#[derive(Debug, Clone)]
pub struct Rsi2Dip {
    rsi_period: usize,
    threshold: f64,
    atr_period: usize,
    sl_atr: f64,
}

/// Wilder の RSI（平滑化係数 1/period、最初の変化から指数平滑）。変化が `period` 個未満なら None
fn rsi(closes: &[f64], period: usize) -> Option<f64> {
    if closes.len() < period + 1 {
        return None;
    }
    let alpha = 1.0 / period as f64;
    let (mut up, mut down) = (None::<f64>, None::<f64>);
    for w in closes.windows(2) {
        let ch = w[1] - w[0];
        let (u, d) = (ch.max(0.0), (-ch).max(0.0));
        up = Some(up.map_or(u, |a| a + alpha * (u - a)));
        down = Some(down.map_or(d, |a| a + alpha * (d - a)));
    }
    let (up, down) = (up?, down?);
    Some(if down == 0.0 { 100.0 } else { 100.0 - 100.0 / (1.0 + up / down) })
}

impl Rsi2Dip {
    pub fn from_params(get: impl Fn(&str) -> f64) -> Result<Self> {
        Ok(Self {
            rsi_period: get("rsi_period") as usize,
            threshold: get("threshold"),
            atr_period: get("atr_period") as usize,
            sl_atr: get("sl_atr"),
        })
    }

    /// RSI の助走に使う日数（係数 1/period の平滑は 10 × period 日でほぼ収束する）
    fn warmup_days(&self) -> usize {
        (self.rsi_period * 10).max(20)
    }
}

impl Strategy for Rsi2Dip {
    fn lookback(&self) -> usize {
        (self.warmup_days() + 7) * BARS_PER_DAY
    }

    fn on_bar(&mut self, ctx: &StrategyContext) -> Vec<Action> {
        // 判断は 16:00 の引けだけ
        if us_cash::slot_ending_at(ctx.now) != Some(SLOTS - 1) {
            return Vec::new();
        }
        let today = us_cash::ny_date(ctx.now);
        let days = us_cash::cash_days(ctx.bars);
        if days.last().map(|d| d.date) != Some(today) {
            return Vec::new();
        }
        let closes: Vec<f64> = days.iter().filter_map(|d| d.close()).collect();
        if closes.len() < self.warmup_days() {
            return Vec::new();
        }
        let Some(value) = rsi(&closes, self.rsi_period) else { return Vec::new() };
        let dip = value < self.threshold;

        if !ctx.positions.is_empty() {
            if dip {
                return Vec::new();
            }
            return ctx
                .positions
                .iter()
                .map(|p| Action::ClosePosition { id: p.id.clone(), reason: format!("RSI({}) = {value:.1} に戻った", self.rsi_period) })
                .collect();
        }
        if !dip {
            return Vec::new();
        }
        let (Some(last), Some(atr)) = (ctx.last(), calculate_atr(ctx.bars, self.atr_period)) else { return Vec::new() };
        let (stop_loss, _) = super::stops_from(Side::Buy, last.close, atr * self.sl_atr, 0.0);
        vec![Action::Open(OrderIntent {
            side: Side::Buy,
            entry: Entry::Market,
            stop_loss,
            take_profit: None,
            expiry_minutes: None,
            reason: format!("日足 RSI({}) = {value:.1} < {}", self.rsi_period, self.threshold),
        })]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctrader::CandleBar;
    use crate::strategy::us_cash::test_bars::{day, slot_close, weekdays};
    use crate::strategy::PositionView;
    use chrono::NaiveDate;

    fn strat() -> Rsi2Dip {
        Rsi2Dip::from_params(|k| match k {
            "rsi_period" => 2.0,
            "threshold" => 10.0,
            "atr_period" => 5.0,
            "sl_atr" => 5.0,
            _ => 0.0,
        })
        .unwrap()
    }

    /// 引けの値を `closes` の順に並べた日々（各日は始値 = 前日引けから一直線に動く）
    fn days(closes: &[f64]) -> (Vec<CandleBar>, NaiveDate) {
        let dates = weekdays(NaiveDate::from_ymd_opt(2025, 3, 3).unwrap(), closes.len());
        let mut bars = Vec::new();
        let mut prev = closes[0];
        for (d, &c) in dates.iter().zip(closes) {
            let path: [f64; SLOTS] = std::array::from_fn(|j| prev + (c - prev) * (j + 1) as f64 / SLOTS as f64);
            bars.extend(day(*d, prev, &path));
            prev = c;
        }
        // 最後の日は 16:00 で確定したところまで（16:00〜17:00 の足は捨てる）
        bars.truncate(bars.len() - 2);
        (bars, *dates.last().unwrap())
    }

    fn rising_then(tail: &[f64]) -> Vec<f64> {
        let mut c: Vec<f64> = (0..25).map(|i| 100.0 + i as f64 * 0.5).collect();
        c.extend_from_slice(tail);
        c
    }

    #[test]
    fn rsi_matches_wilder_smoothing() {
        // 上げ 1・下げ 1 が交互なら RSI は 50 付近、上げ続けなら 100
        assert_eq!(rsi(&[1.0, 2.0, 3.0, 4.0], 2), Some(100.0));
        let v = rsi(&[1.0, 2.0, 1.0, 2.0, 1.0, 2.0, 1.0], 2).unwrap();
        assert!(v < 50.0 && v > 20.0, "{v}");
        assert_eq!(rsi(&[1.0, 2.0], 2), None);
    }

    #[test]
    fn buys_after_a_sharp_dip_at_the_close_only() {
        let (bars, today) = days(&rising_then(&[110.0, 105.0, 101.0]));
        let ctx = StrategyContext { pair: "US500_Z", now: slot_close(today, SLOTS - 1), bars: &bars, pip_size: 0.1, positions: &[], orders: &[] };
        let acts = strat().on_bar(&ctx);
        let [Action::Open(i)] = acts.as_slice() else { panic!("{acts:?}") };
        assert_eq!(i.side, Side::Buy);
        assert!(i.stop_loss < 101.0);

        // 16:00 以外の足では何もしない
        let ctx = StrategyContext { now: slot_close(today, 5), ..ctx };
        assert!(strat().on_bar(&ctx).is_empty());

        // 上げ続けなら買わない
        let (bars, today) = days(&rising_then(&[113.0, 114.0]));
        let ctx = StrategyContext { pair: "US500_Z", now: slot_close(today, SLOTS - 1), bars: &bars, pip_size: 0.1, positions: &[], orders: &[] };
        assert!(strat().on_bar(&ctx).is_empty());
    }

    #[test]
    fn holds_while_still_oversold_and_exits_on_the_rebound() {
        let pos = PositionView { id: "p1".into(), side: Side::Buy, volume_lots: 1.0, entry_price: 101.0, stop_loss: None, take_profit: None, open_time: chrono::Utc::now() };
        let (bars, today) = days(&rising_then(&[110.0, 105.0, 101.0, 98.0]));
        let ctx = StrategyContext { pair: "US500_Z", now: slot_close(today, SLOTS - 1), bars: &bars, pip_size: 0.1, positions: std::slice::from_ref(&pos), orders: &[] };
        assert!(strat().on_bar(&ctx).is_empty());

        let (bars, today) = days(&rising_then(&[110.0, 105.0, 101.0, 106.0]));
        let ctx = StrategyContext { bars: &bars, now: slot_close(today, SLOTS - 1), ..ctx };
        assert!(matches!(strat().on_bar(&ctx).as_slice(), [Action::ClosePosition { id, .. }] if id == "p1"));
    }
}
