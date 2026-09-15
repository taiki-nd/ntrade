//! ペーパー口座の決済シミュレーション。
//! 確定した 5M 足の高安で SL/TP 到達を判定し、ポジションを約定履歴へ移す。

use chrono::Utc;

use crate::ctrader::CandleBar;
use crate::server::types::{CloseReason, Position, TradeHistory};
use crate::snapshot::measures::get_pip_size;

/// 1 lot・1 pip あたりの損益額（口座通貨建ての概算）。JPY口座 + JPYクロス想定。
pub fn pip_value_per_lot(pair: &str) -> f64 {
    if pair.to_uppercase().contains("JPY") {
        1000.0
    } else {
        // USD建てペアを JPY 口座で見た概算（1 pip = 10 USD ≒ 1500 JPY）
        1500.0
    }
}

/// 確定足で SL/TP に触れたポジションを決済し、(残ポジション, 決済トレード) を返す。
/// 同一足で両方に触れた場合は保守的に SL 扱い。
pub fn settle(positions: Vec<Position>, pair: &str, bar: &CandleBar) -> (Vec<Position>, Vec<TradeHistory>) {
    let pip = get_pip_size(pair);
    let pv = pip_value_per_lot(pair);
    let mut open = Vec::new();
    let mut closed = Vec::new();

    for mut p in positions {
        if !p.symbol.eq_ignore_ascii_case(pair) || !p.id.starts_with("paper-") {
            open.push(p);
            continue;
        }
        let is_buy = p.side == "BUY";
        let hit_sl = if is_buy { bar.low <= p.stop_loss } else { bar.high >= p.stop_loss };
        let hit_tp = if is_buy { bar.high >= p.take_profit } else { bar.low <= p.take_profit };
        let (close_price, reason) = match (hit_sl, hit_tp) {
            (true, _) => (p.stop_loss, CloseReason::StopLoss),
            (false, true) => (p.take_profit, CloseReason::TakeProfit),
            _ => {
                // 含み損益を更新して保持
                let sign = if is_buy { 1.0 } else { -1.0 };
                p.current_price = bar.close;
                p.pnl_pips = ((bar.close - p.entry_price) * sign / pip * 10.0).round() / 10.0;
                p.pnl_amount = (p.pnl_pips * pv * p.volume_lots).round();
                open.push(p);
                continue;
            }
        };
        let sign = if is_buy { 1.0 } else { -1.0 };
        let pnl_pips = ((close_price - p.entry_price) * sign / pip * 10.0).round() / 10.0;
        closed.push(TradeHistory {
            id: format!("trd-{}", p.id),
            symbol: p.symbol.clone(),
            side: p.side.clone(),
            volume_lots: p.volume_lots,
            entry_price: p.entry_price,
            close_price,
            stop_loss: p.stop_loss,
            take_profit: p.take_profit,
            pnl_pips,
            pnl_amount: (pnl_pips * pv * p.volume_lots).round(),
            close_reason: reason,
            open_time: p.open_time.clone(),
            close_time: bar.timestamp.format("%Y-%m-%d %H:%M:%S").to_string(),
            cot_log_id: Some(p.invalidation_reason.clone()).filter(|s| s.starts_with("cot-")),
        });
    }
    let _ = Utc::now();
    (open, closed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn pos(side: &str, entry: f64, sl: f64, tp: f64) -> Position {
        Position {
            id: "paper-1".into(),
            symbol: "USDJPY".into(),
            side: side.into(),
            volume_lots: 0.1,
            entry_price: entry,
            current_price: entry,
            stop_loss: sl,
            take_profit: tp,
            pnl_pips: 0.0,
            pnl_amount: 0.0,
            open_time: "t".into(),
            invalidation_reason: "cot-9".into(),
        }
    }

    fn bar(h: f64, l: f64, c: f64) -> CandleBar {
        CandleBar { timestamp: Utc.with_ymd_and_hms(2026, 9, 15, 9, 0, 0).unwrap(), open: c, high: h, low: l, close: c, volume: 1 }
    }

    #[test]
    fn tp_sl_and_hold() {
        let (open, closed) = settle(vec![pos("BUY", 154.20, 154.10, 154.40)], "USDJPY", &bar(154.45, 154.15, 154.40));
        assert!(open.is_empty());
        assert!(matches!(closed[0].close_reason, CloseReason::TakeProfit));
        assert!((closed[0].pnl_pips - 20.0).abs() < 1e-9);
        assert_eq!(closed[0].cot_log_id.as_deref(), Some("cot-9"));

        let (_, closed) = settle(vec![pos("BUY", 154.20, 154.10, 154.40)], "USDJPY", &bar(154.45, 154.05, 154.40));
        assert!(matches!(closed[0].close_reason, CloseReason::StopLoss)); // 同一足は SL 扱い

        let (open, closed) = settle(vec![pos("SELL", 154.20, 154.30, 154.00)], "USDJPY", &bar(154.25, 154.10, 154.12));
        assert!(closed.is_empty());
        assert!((open[0].pnl_pips - 8.0).abs() < 1e-9);
    }
}
