//! EMA クロス: 短期 EMA が長期 EMA を上抜けたら買い、下抜けたら売る。
//!
//! - エントリーは成行（クロスを確認した足の次の足で約定）
//! - SL は終値から ATR × `sl_atr`、TP は SL 幅 × `tp_rr`（0 なら TP なし）
//! - 反対のクロスで保有中の建玉を決済し、ドテンする（`exit_on_cross = 1` のとき）

use anyhow::{bail, Result};

use super::types::{Action, Entry, OrderIntent, ParamSpec, Side, Strategy, StrategyContext, StrategyInfo};
use crate::indicators::{calculate_atr, calculate_ema};

pub const NAME: &str = "ema_cross";

pub fn info() -> StrategyInfo {
    StrategyInfo {
        name: NAME,
        title: "EMA クロス",
        description: "短期 EMA が長期 EMA を上抜けたら買い・下抜けたら売り（成行）。SL は ATR 倍、TP は SL 幅の RR 倍。反対のクロスで決済してドテン",
        params: vec![
            ParamSpec { name: "fast", default: 20.0, min: 2.0, max: 500.0, integer: true, description: "短期 EMA の期間" },
            ParamSpec { name: "slow", default: 50.0, min: 3.0, max: 1000.0, integer: true, description: "長期 EMA の期間" },
            ParamSpec { name: "atr_period", default: 14.0, min: 2.0, max: 200.0, integer: true, description: "ATR の期間" },
            ParamSpec { name: "sl_atr", default: 1.5, min: 0.1, max: 20.0, integer: false, description: "SL 幅（ATR の倍数）" },
            ParamSpec { name: "tp_rr", default: 2.0, min: 0.0, max: 20.0, integer: false, description: "TP 幅（SL 幅の倍数。0 で TP なし）" },
            ParamSpec { name: "exit_on_cross", default: 1.0, min: 0.0, max: 1.0, integer: true, description: "反対のクロスで決済する（1）/ しない（0）" },
        ],
    }
}

#[derive(Debug, Clone)]
pub struct EmaCross {
    fast: usize,
    slow: usize,
    atr_period: usize,
    sl_atr: f64,
    tp_rr: f64,
    exit_on_cross: bool,
}

impl EmaCross {
    pub fn from_params(get: impl Fn(&str) -> f64) -> Result<Self> {
        let s = Self {
            fast: get("fast") as usize,
            slow: get("slow") as usize,
            atr_period: get("atr_period") as usize,
            sl_atr: get("sl_atr"),
            tp_rr: get("tp_rr"),
            exit_on_cross: get("exit_on_cross") >= 0.5,
        };
        if s.fast >= s.slow {
            bail!("{NAME}: fast ({}) must be shorter than slow ({})", s.fast, s.slow);
        }
        Ok(s)
    }

    /// 直近の足でクロスしたか（上抜け = Buy / 下抜け = Sell）
    fn cross(&self, ctx: &StrategyContext) -> Option<Side> {
        let fast = calculate_ema(ctx.bars, self.fast);
        let slow = calculate_ema(ctx.bars, self.slow);
        let n = ctx.bars.len();
        if n < 2 {
            return None;
        }
        let (f0, s0, f1, s1) = (fast[n - 2]?, slow[n - 2]?, fast[n - 1]?, slow[n - 1]?);
        if f0 <= s0 && f1 > s1 {
            Some(Side::Buy)
        } else if f0 >= s0 && f1 < s1 {
            Some(Side::Sell)
        } else {
            None
        }
    }
}

impl Strategy for EmaCross {
    fn lookback(&self) -> usize {
        // EMA は始点の影響が残るので、長期 EMA の 4 倍を助走にする（本番とバックテストで同じ本数を渡す）
        (self.slow * 4).max(self.atr_period + 2)
    }

    fn on_bar(&mut self, ctx: &StrategyContext) -> Vec<Action> {
        let Some(side) = self.cross(ctx) else { return Vec::new() };
        let mut actions = Vec::new();
        if self.exit_on_cross {
            for p in ctx.positions.iter().filter(|p| p.side != side) {
                actions.push(Action::ClosePosition { id: p.id.clone(), reason: "反対のクロス".into() });
            }
        }
        // 同じ向きの建玉があれば積み増さない
        if ctx.position(side).is_some() {
            return actions;
        }
        let (Some(last), Some(atr)) = (ctx.last(), calculate_atr(ctx.bars, self.atr_period)) else { return actions };
        let (stop_loss, take_profit) = super::stops_from(side, last.close, atr * self.sl_atr, self.tp_rr);
        actions.push(Action::Open(OrderIntent {
            side,
            entry: Entry::Market,
            stop_loss,
            take_profit,
            expiry_minutes: None,
            reason: format!("EMA{} が EMA{} を{}", self.fast, self.slow, if side == Side::Buy { "上抜け" } else { "下抜け" }),
        }));
        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctrader::CandleBar;
    use crate::strategy::PositionView;
    use chrono::{Duration, TimeZone, Utc};

    fn bars(closes: &[f64]) -> Vec<CandleBar> {
        let t0 = Utc.with_ymd_and_hms(2026, 9, 1, 0, 0, 0).unwrap();
        closes
            .iter()
            .enumerate()
            .map(|(i, c)| CandleBar { timestamp: t0 + Duration::minutes(5 * i as i64), open: *c, high: c + 0.05, low: c - 0.05, close: *c, volume: 1 })
            .collect()
    }

    fn strat() -> EmaCross {
        let p = |k: &str| match k {
            "fast" => 3.0,
            "slow" => 6.0,
            "atr_period" => 3.0,
            "sl_atr" => 1.0,
            "tp_rr" => 2.0,
            _ => 1.0,
        };
        EmaCross::from_params(p).unwrap()
    }

    fn ctx<'a>(b: &'a [CandleBar], positions: &'a [PositionView]) -> StrategyContext<'a> {
        StrategyContext { pair: "USDJPY", now: b.last().unwrap().timestamp + Duration::minutes(5), bars: b, pip_size: 0.01, positions, orders: &[] }
    }

    #[test]
    fn golden_cross_opens_a_buy_with_atr_stops() {
        // 下落のあと急騰して短期 EMA が長期 EMA を上抜ける足を探す
        let mut closes: Vec<f64> = (0..20).map(|i| 150.0 - i as f64 * 0.1).collect();
        let mut s = strat();
        let mut opened = None;
        for k in 0..10 {
            closes.push(148.2 + k as f64 * 0.4);
            let b = bars(&closes);
            let acts = s.on_bar(&ctx(&b, &[]));
            if let Some(Action::Open(i)) = acts.first() {
                opened = Some((i.clone(), b.last().unwrap().close));
                break;
            }
        }
        let (intent, close) = opened.expect("cross should happen");
        assert_eq!(intent.side, Side::Buy);
        assert_eq!(intent.entry, Entry::Market);
        assert!(intent.stop_loss < close);
        let tp = intent.take_profit.unwrap();
        assert!(((tp - close) - 2.0 * (close - intent.stop_loss)).abs() < 1e-9);
    }

    #[test]
    fn opposite_cross_closes_and_reverses_but_never_stacks() {
        let mut closes: Vec<f64> = (0..20).map(|i| 150.0 + i as f64 * 0.1).collect();
        closes.extend([150.5, 149.5, 148.5]);
        let b = bars(&closes);
        let long = PositionView { id: "p1".into(), side: Side::Buy, volume_lots: 0.1, entry_price: 151.0, stop_loss: None, take_profit: None, open_time: b[0].timestamp };
        let mut s = strat();
        // 下抜けた足を見つける
        let acts = (21..=b.len()).find_map(|n| {
            let a = s.on_bar(&ctx(&b[..n], std::slice::from_ref(&long)));
            (!a.is_empty()).then_some(a)
        });
        let acts = acts.expect("dead cross");
        assert!(matches!(&acts[0], Action::ClosePosition { id, .. } if id == "p1"));
        assert!(matches!(&acts[1], Action::Open(i) if i.side == Side::Sell));

        // 同じ向きの建玉があれば新規は出さない
        let short = PositionView { side: Side::Sell, ..long };
        let acts = (21..=b.len()).find_map(|n| {
            let a = s.on_bar(&ctx(&b[..n], std::slice::from_ref(&short)));
            (!a.is_empty()).then_some(a)
        });
        assert!(acts.is_none());
    }

    #[test]
    fn fast_must_be_shorter_than_slow() {
        assert!(EmaCross::from_params(|k| if k == "fast" { 50.0 } else { 20.0 }).is_err());
    }
}
