//! ノイズ帯ブレイクアウト（Zarattini, Aziz & Barbon 2024 "Beat the Market"）。米国株価指数 CFD の 30 分足で使う。
//!
//! - 帯: 9:30 の始値と前日の引けから、上は max(始値, 前日引け) × (1 + σ)、下は min(始値, 前日引け) × (1 − σ)。
//!   σ は同じ時刻の「始値からの動き |価格 / 始値 − 1|」の直近 `lookback_days` 日（13 本そろった日）の平均
//! - 10:00〜15:30 の 30 分ごとに判断する。帯の上なら買い、下なら売り（成行）。建玉が帯の内側に戻ったら決済
//! - 16:00 の引けで全部決済する（日をまたがない）
//! - SL は終値から ATR × `sl_atr`（帯に戻ったときの決済が先に効く。SL は途中の急変への備え）

use anyhow::Result;

use super::types::{Action, Entry, OrderIntent, ParamSpec, Side, Strategy, StrategyContext, StrategyInfo};
use super::us_cash::{self, CashDay, BARS_PER_DAY, SLOTS};
use crate::indicators::calculate_atr;

pub const NAME: &str = "noise_area";

pub fn info() -> StrategyInfo {
    StrategyInfo {
        name: NAME,
        title: "ノイズ帯ブレイクアウト（米国株価指数・30 分足）",
        description: "始値と前日引けから、同時刻の平均的な値動きで帯を作る。10:00〜15:30 の 30 分ごとに帯の外へ出た方向に乗り、帯に戻ったら決済、16:00 に手仕舞い",
        params: vec![
            ParamSpec { name: "lookback_days", default: 14.0, min: 2.0, max: 60.0, integer: true, description: "帯の幅（σ）を平均する日数" },
            ParamSpec { name: "atr_period", default: 14.0, min: 2.0, max: 200.0, integer: true, description: "SL に使う ATR の期間" },
            ParamSpec { name: "sl_atr", default: 4.0, min: 0.5, max: 20.0, integer: false, description: "SL 幅（ATR の倍数）" },
        ],
    }
}

#[derive(Debug, Clone)]
pub struct NoiseArea {
    lookback_days: usize,
    atr_period: usize,
    sl_atr: f64,
}

/// ある時刻の帯
#[derive(Debug, Clone, Copy, PartialEq)]
struct Band {
    upper: f64,
    lower: f64,
}

impl NoiseArea {
    pub fn from_params(get: impl Fn(&str) -> f64) -> Result<Self> {
        Ok(Self { lookback_days: get("lookback_days") as usize, atr_period: get("atr_period") as usize, sl_atr: get("sl_atr") })
    }

    /// 今日（`days` の末尾）の `slot` 本目の終わりの帯。前の日が足りなければ None
    fn band(&self, days: &[CashDay], slot: usize) -> Option<Band> {
        let (today, past) = days.split_last()?;
        let complete: Vec<&CashDay> = past.iter().filter(|d| d.complete()).collect();
        if complete.len() < self.lookback_days {
            return None;
        }
        let prev_close = complete.last()?.close()?;
        let recent = &complete[complete.len() - self.lookback_days..];
        let sigma = recent.iter().map(|d| (d.closes[slot].unwrap_or(d.open) / d.open - 1.0).abs()).sum::<f64>() / recent.len() as f64;
        Some(Band { upper: today.open.max(prev_close) * (1.0 + sigma), lower: today.open.min(prev_close) * (1.0 - sigma) })
    }
}

impl Strategy for NoiseArea {
    fn lookback(&self) -> usize {
        // 祝日・欠けた日の分を見込んで 1 週間ほど余分に持つ
        (self.lookback_days + 7) * BARS_PER_DAY
    }

    fn on_bar(&mut self, ctx: &StrategyContext) -> Vec<Action> {
        let Some(last) = ctx.last() else { return Vec::new() };
        let slot = us_cash::slot_ending_at(ctx.now);
        let today = us_cash::ny_date(ctx.now);
        let close_all = |reason: &str| -> Vec<Action> {
            ctx.positions.iter().map(|p| Action::ClosePosition { id: p.id.clone(), reason: reason.to_string() }).collect()
        };
        // 引け（16:00）か、前の日から持ち越した建玉（引けの判断を逃した）は決済
        if slot == Some(SLOTS - 1) {
            return close_all("16:00 の引けで手仕舞い");
        }
        if ctx.positions.iter().any(|p| us_cash::ny_date(p.open_time) != today) || slot.is_none() {
            return close_all("現物時間の外");
        }
        let Some(slot) = slot else { return Vec::new() };

        let days = us_cash::cash_days(ctx.bars);
        if days.last().map(|d| d.date) != Some(today) {
            return close_all("今日の始値が無い");
        }
        let Some(band) = self.band(&days, slot) else { return Vec::new() };
        let price = last.close;

        let mut actions: Vec<Action> = ctx
            .positions
            .iter()
            .filter(|p| match p.side {
                Side::Buy => price < band.upper,
                Side::Sell => price > band.lower,
            })
            .map(|p| Action::ClosePosition { id: p.id.clone(), reason: "帯の内側に戻った".into() })
            .collect();
        if ctx.positions.len() > actions.len() {
            return actions;
        }
        let side = if price > band.upper {
            Side::Buy
        } else if price < band.lower {
            Side::Sell
        } else {
            return actions;
        };
        let Some(atr) = calculate_atr(ctx.bars, self.atr_period) else { return actions };
        let (stop_loss, _) = super::stops_from(side, price, atr * self.sl_atr, 0.0);
        actions.push(Action::Open(OrderIntent {
            side,
            entry: Entry::Market,
            stop_loss,
            take_profit: None,
            expiry_minutes: None,
            reason: format!(
                "ノイズ帯の{}抜け（帯 {:.2}〜{:.2}）",
                if side == Side::Buy { "上" } else { "下" },
                band.lower,
                band.upper
            ),
        }));
        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctrader::CandleBar;
    use crate::strategy::us_cash::test_bars::{day, slot_close, weekdays};
    use crate::strategy::PositionView;
    use chrono::NaiveDate;

    fn strat() -> NoiseArea {
        NoiseArea::from_params(|k| match k {
            "lookback_days" => 3.0,
            "atr_period" => 5.0,
            "sl_atr" => 4.0,
            _ => 0.0,
        })
        .unwrap()
    }

    /// 3 日間、始値 100 から各時刻 ±1%（σ = 1%）動いて 100 で引ける日。最後に今日の足を 0..=upto 本目まで
    fn history(today_path: &[f64]) -> (Vec<CandleBar>, NaiveDate) {
        let dates = weekdays(NaiveDate::from_ymd_opt(2025, 3, 3).unwrap(), 4);
        let mut bars = Vec::new();
        for (i, d) in dates[..3].iter().enumerate() {
            let s = if i % 2 == 0 { 1.0 } else { -1.0 };
            let mut path = [100.0 + s; SLOTS];
            path[SLOTS - 1] = 100.0;
            bars.extend(day(*d, 100.0, &path));
        }
        let mut path = [100.0; SLOTS];
        path[..today_path.len()].copy_from_slice(today_path);
        let today = day(dates[3], 100.0, &path);
        // 今日は 9:30 から today_path の本数分だけ確定している（8:00〜9:30 の 3 本 + 現物時間の足）
        bars.extend_from_slice(&today[..3 + today_path.len()]);
        (bars, dates[3])
    }

    fn ctx<'a>(bars: &'a [CandleBar], now: chrono::DateTime<chrono::Utc>, positions: &'a [PositionView]) -> StrategyContext<'a> {
        StrategyContext { pair: "US100_Z", now, bars, pip_size: 0.1, positions, orders: &[] }
    }

    #[test]
    fn buys_above_the_band_and_ignores_moves_inside() {
        // σ = 1%、始値 = 前日引け = 100 → 帯は 99〜101
        let (bars, today) = history(&[101.5]);
        let acts = strat().on_bar(&ctx(&bars, slot_close(today, 0), &[]));
        let [Action::Open(i)] = acts.as_slice() else { panic!("{acts:?}") };
        assert_eq!(i.side, Side::Buy);
        assert_eq!(i.entry, Entry::Market);
        assert!(i.stop_loss < 101.5);

        let (bars, today) = history(&[100.5]);
        assert!(strat().on_bar(&ctx(&bars, slot_close(today, 0), &[])).is_empty());

        let (bars, today) = history(&[98.0]);
        assert!(matches!(strat().on_bar(&ctx(&bars, slot_close(today, 0), &[])).as_slice(), [Action::Open(i)] if i.side == Side::Sell));
    }

    #[test]
    fn exits_back_inside_the_band_and_at_the_close() {
        let (bars, today) = history(&[101.5, 100.5]);
        let pos = PositionView { id: "p1".into(), side: Side::Buy, volume_lots: 1.0, entry_price: 101.5, stop_loss: None, take_profit: None, open_time: slot_close(today, 0) };
        let acts = strat().on_bar(&ctx(&bars, slot_close(today, 1), std::slice::from_ref(&pos)));
        assert!(matches!(acts.as_slice(), [Action::ClosePosition { id, .. }] if id == "p1"));

        // 帯の上にいる間は持ち続ける
        let (bars, today) = history(&[101.5, 102.0]);
        assert!(strat().on_bar(&ctx(&bars, slot_close(today, 1), std::slice::from_ref(&pos))).is_empty());

        // 16:00 は必ず手仕舞い
        let path: Vec<f64> = vec![102.0; SLOTS];
        let (bars, today) = history(&path);
        let acts = strat().on_bar(&ctx(&bars, slot_close(today, SLOTS - 1), std::slice::from_ref(&pos)));
        assert!(matches!(acts.as_slice(), [Action::ClosePosition { .. }]));
    }

    #[test]
    fn needs_enough_past_days() {
        let (bars, today) = history(&[105.0]);
        let mut s = strat();
        s.lookback_days = 10;
        assert!(s.on_bar(&ctx(&bars, slot_close(today, 0), &[])).is_empty());
    }
}
