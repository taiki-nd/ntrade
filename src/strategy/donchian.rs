//! ドンチャン・ブレイクアウト: 直近 N 本の高値・安値の外に逆指値を置き、抜けた方向に乗る。
//!
//! - トレンドフィルター: 終値が EMA(`trend_ema`) より上なら買いの逆指値だけ、下なら売りだけを置く（0 で両方向）
//! - 建玉が無い間は足ごとに注文を置き直す（チャネルが動くため）。期限は 1 足分
//! - SL は約定価格から ATR × `sl_atr`、TP は SL 幅 × `tp_rr`（0 なら TP なし）
//! - 保有中は反対側のチャネル（`exit_channel` 本）を終値で抜けたら決済する（0 で使わない）

use anyhow::Result;

use super::types::{Action, Entry, OrderIntent, ParamSpec, Side, Strategy, StrategyContext, StrategyInfo};
use crate::ctrader::BarPeriod;
use crate::indicators::{calculate_atr, calculate_ema, channel};

pub const NAME: &str = "donchian_breakout";

pub fn info() -> StrategyInfo {
    StrategyInfo {
        name: NAME,
        title: "ドンチャン・ブレイクアウト",
        description: "直近 N 本の高値・安値の外に逆指値を置き、抜けた方向に乗る。EMA でトレンド方向に絞り、反対側のチャネルを終値で抜けたら決済",
        params: vec![
            ParamSpec { name: "channel", default: 20.0, min: 2.0, max: 500.0, integer: true, description: "エントリーのチャネル本数" },
            ParamSpec { name: "exit_channel", default: 10.0, min: 0.0, max: 500.0, integer: true, description: "決済のチャネル本数（0 で使わない）" },
            ParamSpec { name: "trend_ema", default: 100.0, min: 0.0, max: 1000.0, integer: true, description: "トレンドフィルターの EMA 期間（0 で両方向）" },
            ParamSpec { name: "atr_period", default: 14.0, min: 2.0, max: 200.0, integer: true, description: "ATR の期間" },
            ParamSpec { name: "sl_atr", default: 2.0, min: 0.1, max: 20.0, integer: false, description: "SL 幅（ATR の倍数）" },
            ParamSpec { name: "tp_rr", default: 0.0, min: 0.0, max: 20.0, integer: false, description: "TP 幅（SL 幅の倍数。0 で TP なし）" },
            ParamSpec { name: "buffer_pips", default: 1.0, min: 0.0, max: 100.0, integer: false, description: "チャネルの外側に置く余白（pips）" },
        ],
    }
}

#[derive(Debug, Clone)]
pub struct DonchianBreakout {
    channel: usize,
    exit_channel: usize,
    trend_ema: usize,
    atr_period: usize,
    sl_atr: f64,
    tp_rr: f64,
    buffer_pips: f64,
    /// 逆指値の期限（分）。1 足分
    expiry_minutes: u32,
}

impl DonchianBreakout {
    pub fn from_params(get: impl Fn(&str) -> f64, timeframe: BarPeriod) -> Result<Self> {
        Ok(Self {
            channel: get("channel") as usize,
            exit_channel: get("exit_channel") as usize,
            trend_ema: get("trend_ema") as usize,
            atr_period: get("atr_period") as usize,
            sl_atr: get("sl_atr"),
            tp_rr: get("tp_rr"),
            buffer_pips: get("buffer_pips"),
            expiry_minutes: timeframe.duration().num_minutes().max(1) as u32,
        })
    }

    fn exits(&self, ctx: &StrategyContext) -> Vec<Action> {
        // 今の足を除いた直近 exit_channel 本の外で終えたら決済
        let n = ctx.bars.len();
        if self.exit_channel == 0 || n < self.exit_channel + 1 {
            return Vec::new();
        }
        let (Some(prev), Some(last)) = (channel(&ctx.bars[..n - 1], self.exit_channel), ctx.last()) else { return Vec::new() };
        ctx.positions
            .iter()
            .filter(|p| match p.side {
                Side::Buy => last.close < prev.low,
                Side::Sell => last.close > prev.high,
            })
            .map(|p| Action::ClosePosition { id: p.id.clone(), reason: format!("反対側の {} 本チャネルを終値で抜けた", self.exit_channel) })
            .collect()
    }
}

impl Strategy for DonchianBreakout {
    fn lookback(&self) -> usize {
        (self.trend_ema * 4).max(self.channel + 1).max(self.exit_channel + 1).max(self.atr_period + 2)
    }

    fn on_bar(&mut self, ctx: &StrategyContext) -> Vec<Action> {
        let mut actions = self.exits(ctx);
        let closing = actions.len();
        // 置いてある逆指値は毎足取り消して置き直す
        actions.extend(ctx.orders.iter().map(|o| Action::CancelOrder { id: o.id.clone(), reason: "チャネルを更新".into() }));
        if ctx.positions.len() > closing {
            return actions;
        }
        let (Some(range), Some(atr), Some(last)) =
            (channel(ctx.bars, self.channel), calculate_atr(ctx.bars, self.atr_period), ctx.last())
        else {
            return actions;
        };
        let trend = if self.trend_ema == 0 {
            None
        } else {
            match calculate_ema(ctx.bars, self.trend_ema).last().copied().flatten() {
                Some(ema) if last.close > ema => Some(Side::Buy),
                Some(_) => Some(Side::Sell),
                None => return actions,
            }
        };
        let buffer = self.buffer_pips * ctx.pip_size;
        for side in [Side::Buy, Side::Sell] {
            if trend.is_some_and(|t| t != side) {
                continue;
            }
            let price = match side {
                Side::Buy => range.high + buffer,
                Side::Sell => range.low - buffer,
            };
            let (stop_loss, take_profit) = super::stops_from(side, price, atr * self.sl_atr, self.tp_rr);
            actions.push(Action::Open(OrderIntent {
                side,
                entry: Entry::Stop { price },
                stop_loss,
                take_profit,
                expiry_minutes: Some(self.expiry_minutes),
                reason: format!("{} 本チャネルの{}抜け", self.channel, if side == Side::Buy { "上" } else { "下" }),
            }));
        }
        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctrader::{CandleBar, PendingKind};
    use crate::strategy::{OrderView, PositionView};
    use chrono::{Duration, TimeZone, Utc};

    fn bars(n: usize, slope: f64) -> Vec<CandleBar> {
        let t0 = Utc.with_ymd_and_hms(2026, 9, 1, 0, 0, 0).unwrap();
        (0..n)
            .map(|i| {
                let c = 150.0 + i as f64 * slope;
                CandleBar { timestamp: t0 + Duration::minutes(5 * i as i64), open: c, high: c + 0.05, low: c - 0.05, close: c, volume: 1 }
            })
            .collect()
    }

    fn strat() -> DonchianBreakout {
        let p = |k: &str| match k {
            "channel" => 10.0,
            "exit_channel" => 5.0,
            "trend_ema" => 20.0,
            "atr_period" => 5.0,
            "sl_atr" => 2.0,
            "tp_rr" => 0.0,
            "buffer_pips" => 1.0,
            _ => 0.0,
        };
        DonchianBreakout::from_params(p, BarPeriod::M5).unwrap()
    }

    #[test]
    fn uptrend_places_only_a_buy_stop_above_the_channel() {
        let b = bars(100, 0.01);
        let order = OrderView {
            id: "o1".into(),
            side: Side::Buy,
            kind: PendingKind::Stop,
            price: 1.0,
            stop_loss: None,
            take_profit: None,
            created_at: b[0].timestamp,
            expires_at: None,
        };
        let ctx = StrategyContext { pair: "USDJPY", now: b[99].timestamp, bars: &b, pip_size: 0.01, positions: &[], orders: std::slice::from_ref(&order) };
        let acts = strat().on_bar(&ctx);
        assert!(matches!(&acts[0], Action::CancelOrder { id, .. } if id == "o1"));
        let Action::Open(i) = &acts[1] else { panic!("{acts:?}") };
        assert_eq!(acts.len(), 2);
        assert_eq!(i.side, Side::Buy);
        let high = b[90..].iter().map(|x| x.high).fold(f64::MIN, f64::max);
        assert!(matches!(i.entry, Entry::Stop { price } if (price - (high + 0.01)).abs() < 1e-9));
        assert_eq!(i.take_profit, None);
        assert_eq!(i.expiry_minutes, Some(5));
    }

    #[test]
    fn holding_a_position_exits_on_the_opposite_channel_and_places_nothing() {
        let mut b = bars(100, 0.01);
        let last = b.last().unwrap().clone();
        b.push(CandleBar { timestamp: last.timestamp + Duration::minutes(5), open: last.close, high: last.close, low: 149.0, close: 149.0, volume: 1 });
        let pos = PositionView { id: "p1".into(), side: Side::Buy, volume_lots: 0.1, entry_price: 150.5, stop_loss: None, take_profit: None, open_time: b[0].timestamp };
        let ctx = StrategyContext { pair: "USDJPY", now: last.timestamp, bars: &b, pip_size: 0.01, positions: std::slice::from_ref(&pos), orders: &[] };
        let acts = strat().on_bar(&ctx);
        assert!(matches!(&acts[0], Action::ClosePosition { id, .. } if id == "p1"));
        // 決済する建玉しか無いので、次の逆指値を置く（終値が EMA を割ったので売り側）
        assert_eq!(acts.len(), 2);
        assert!(matches!(&acts[1], Action::Open(i) if i.side == Side::Sell));

        // 抜けていなければ何もしない
        let b2 = bars(100, 0.01);
        let ctx = StrategyContext { bars: &b2, ..ctx };
        assert!(strat().on_bar(&ctx).is_empty());
    }
}
