//! 採点: 判断時刻以降の5M足で結果を機械的に判定する。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ctrader::CandleBar;
use crate::strategy::types::{Action, ConditionalPlan, PriceCondition, TradeDecision};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Outcome {
    /// SL 到達前に TP 到達
    TpHit,
    /// TP 到達前に SL 到達
    SlHit,
    /// 同一足で SL と TP の両方に触れた（保守的に SL 扱い）
    SameBar,
    /// 規定本数内にどちらにも到達しない
    Timeout,
    /// HOLD（条件付きプランなし）
    Hold,
    /// 条件付きプランが期限内に成立しなかった
    PlanExpired,
    /// 条件付きプランが破棄条件に達した
    PlanInvalidated,
    /// 条件付きプランに構造化条件が無く評価不能
    PlanUnstructured,
    /// BUY/SELL なのに SL/TP が無い
    NoSlTp,
    /// 採点に使う将来の足が無い（期間末尾）
    NoData,
}

impl Outcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            Outcome::TpHit => "TP_HIT",
            Outcome::SlHit => "SL_HIT",
            Outcome::SameBar => "SAME_BAR",
            Outcome::Timeout => "TIMEOUT",
            Outcome::Hold => "HOLD",
            Outcome::PlanExpired => "PLAN_EXPIRED",
            Outcome::PlanInvalidated => "PLAN_INVALIDATED",
            Outcome::PlanUnstructured => "PLAN_UNSTRUCTURED",
            Outcome::NoSlTp => "NO_SL_TP",
            Outcome::NoData => "NO_DATA",
        }
    }

    pub fn is_trade(&self) -> bool {
        matches!(self, Outcome::TpHit | Outcome::SlHit | Outcome::SameBar | Outcome::Timeout)
    }

    pub fn is_win(&self) -> bool {
        matches!(self, Outcome::TpHit)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoreResult {
    pub outcome: Outcome,
    pub pnl_pips: Option<f64>,
    pub bars_to_exit: Option<usize>,
    /// 実際にエントリーしたとみなした価格（プラン成立時は成立足の終値）
    pub entry_price: Option<f64>,
}

impl ScoreResult {
    fn flat(outcome: Outcome) -> Self {
        Self { outcome, pnl_pips: None, bars_to_exit: None, entry_price: None }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ScoreConfig {
    pub pip_size: f64,
    /// スプレッド（pips）。損益から差し引く
    pub spread_pips: f64,
    /// 何本以内に決着しなければ TIMEOUT にするか
    pub max_bars: usize,
}

/// 成行エントリーの採点。`bars` は判断時刻以降の5M足（昇順）。
pub fn score_trade(
    bars: &[CandleBar],
    action: Action,
    entry_price: Option<f64>,
    stop_loss: Option<f64>,
    take_profit: Option<f64>,
    cfg: &ScoreConfig,
) -> ScoreResult {
    let (Some(sl), Some(tp)) = (stop_loss, take_profit) else {
        return ScoreResult::flat(Outcome::NoSlTp);
    };
    let Some(first) = bars.first() else {
        return ScoreResult::flat(Outcome::NoData);
    };
    let entry = entry_price.unwrap_or(first.open);
    score_from(bars, action, entry, sl, tp, cfg)
}

fn score_from(bars: &[CandleBar], action: Action, entry: f64, sl: f64, tp: f64, cfg: &ScoreConfig) -> ScoreResult {
    let sign = match action {
        Action::Buy => 1.0,
        Action::Sell => -1.0,
        Action::Hold => return ScoreResult::flat(Outcome::Hold),
    };
    let pnl = |exit: f64| ((exit - entry) * sign) / cfg.pip_size - cfg.spread_pips;

    for (i, b) in bars.iter().take(cfg.max_bars).enumerate() {
        let hit_sl = if sign > 0.0 { b.low <= sl } else { b.high >= sl };
        let hit_tp = if sign > 0.0 { b.high >= tp } else { b.low <= tp };
        match (hit_sl, hit_tp) {
            (true, true) => {
                return ScoreResult { outcome: Outcome::SameBar, pnl_pips: Some(pnl(sl)), bars_to_exit: Some(i + 1), entry_price: Some(entry) }
            }
            (true, false) => {
                return ScoreResult { outcome: Outcome::SlHit, pnl_pips: Some(pnl(sl)), bars_to_exit: Some(i + 1), entry_price: Some(entry) }
            }
            (false, true) => {
                return ScoreResult { outcome: Outcome::TpHit, pnl_pips: Some(pnl(tp)), bars_to_exit: Some(i + 1), entry_price: Some(entry) }
            }
            _ => {}
        }
    }

    let n = bars.len().min(cfg.max_bars);
    if n == 0 {
        return ScoreResult::flat(Outcome::NoData);
    }
    if bars.len() < cfg.max_bars {
        // 期間末尾でデータが尽きた: 結果不明
        return ScoreResult { outcome: Outcome::NoData, pnl_pips: None, bars_to_exit: None, entry_price: Some(entry) };
    }
    ScoreResult {
        outcome: Outcome::Timeout,
        pnl_pips: Some(pnl(bars[n - 1].close)),
        bars_to_exit: Some(n),
        entry_price: Some(entry),
    }
}

fn condition_met(cond: PriceCondition, price: f64, close: f64) -> bool {
    match cond {
        PriceCondition::CloseAbove => close > price,
        PriceCondition::CloseBelow => close < price,
    }
}

/// 条件付きプランの評価。`bars` は判断時刻以降の5M足（昇順）。
/// 破棄 → 成立の順で各確定足を評価し、成立したら次の足からトレードとして採点する。
pub fn evaluate_plan(bars: &[CandleBar], plan: &ConditionalPlan, expires_at: Option<DateTime<Utc>>, cfg: &ScoreConfig) -> ScoreResult {
    if !plan.is_structured() {
        return ScoreResult::flat(Outcome::PlanUnstructured);
    }
    let (tp_price, tp_cond) = (plan.trigger_price.unwrap(), plan.trigger_condition.unwrap());
    let (inv_price, inv_cond) = (plan.invalidate_price.unwrap(), plan.invalidate_condition.unwrap());

    for (i, b) in bars.iter().enumerate() {
        if let Some(exp) = expires_at {
            if b.timestamp >= exp {
                return ScoreResult::flat(Outcome::PlanExpired);
            }
        }
        if condition_met(inv_cond, inv_price, b.close) {
            return ScoreResult::flat(Outcome::PlanInvalidated);
        }
        if condition_met(tp_cond, tp_price, b.close) {
            let (Some(sl), Some(tp)) = (plan.stop_loss, plan.take_profit) else {
                return ScoreResult::flat(Outcome::NoSlTp);
            };
            let rest = &bars[i + 1..];
            if rest.is_empty() {
                return ScoreResult::flat(Outcome::NoData);
            }
            let mut r = score_from(rest, plan.then_action, b.close, sl, tp, cfg);
            r.bars_to_exit = r.bars_to_exit.map(|n| n + i + 1);
            return r;
        }
    }
    if expires_at.is_some() {
        ScoreResult::flat(Outcome::NoData)
    } else {
        ScoreResult::flat(Outcome::PlanExpired)
    }
}

/// TradeDecision 全体の採点
pub fn score_decision(bars_after: &[CandleBar], decision: &TradeDecision, cfg: &ScoreConfig) -> ScoreResult {
    match decision.action {
        Action::Buy | Action::Sell => score_trade(
            bars_after,
            decision.action,
            decision.entry_price,
            decision.stop_loss,
            decision.take_profit,
            cfg,
        ),
        Action::Hold => match &decision.conditional_plan {
            Some(plan) if plan.then_action != Action::Hold => {
                let expires = crate::storage::parse_ts(&plan.expires_at)
                    .ok()
                    .and_then(|s| chrono::TimeZone::timestamp_opt(&Utc, s, 0).single());
                evaluate_plan(bars_after, plan, expires, cfg)
            }
            _ => ScoreResult::flat(Outcome::Hold),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone};

    fn bars(closes: &[(f64, f64, f64)]) -> Vec<CandleBar> {
        let base = Utc.with_ymd_and_hms(2026, 9, 15, 9, 0, 0).unwrap();
        closes
            .iter()
            .enumerate()
            .map(|(i, (h, l, c))| CandleBar {
                timestamp: base + Duration::minutes(5 * i as i64),
                open: *c,
                high: *h,
                low: *l,
                close: *c,
                volume: 1,
            })
            .collect()
    }

    fn cfg() -> ScoreConfig {
        ScoreConfig { pip_size: 0.01, spread_pips: 0.2, max_bars: 4 }
    }

    #[test]
    fn buy_hits_tp_before_sl() {
        let b = bars(&[(154.25, 154.15, 154.20), (154.55, 154.18, 154.50)]);
        let r = score_trade(&b, Action::Buy, Some(154.20), Some(154.07), Some(154.52), &cfg());
        assert_eq!(r.outcome, Outcome::TpHit);
        assert!((r.pnl_pips.unwrap() - (32.0 - 0.2)).abs() < 1e-6);
        assert_eq!(r.bars_to_exit, Some(2));
    }

    #[test]
    fn sell_hits_sl_and_same_bar_is_conservative() {
        let b = bars(&[(154.35, 154.10, 154.20)]);
        let r = score_trade(&b, Action::Sell, Some(154.20), Some(154.30), Some(154.00), &cfg());
        assert_eq!(r.outcome, Outcome::SlHit);

        let b = bars(&[(154.35, 153.95, 154.20)]);
        let r = score_trade(&b, Action::Sell, Some(154.20), Some(154.30), Some(154.00), &cfg());
        assert_eq!(r.outcome, Outcome::SameBar);
        assert!(r.pnl_pips.unwrap() < 0.0);
    }

    #[test]
    fn timeout_and_no_data() {
        let flat = bars(&[(154.22, 154.18, 154.20); 4]);
        let r = score_trade(&flat, Action::Buy, Some(154.20), Some(154.00), Some(154.50), &cfg());
        assert_eq!(r.outcome, Outcome::Timeout);
        let short = bars(&[(154.22, 154.18, 154.20); 2]);
        let r = score_trade(&short, Action::Buy, Some(154.20), Some(154.00), Some(154.50), &cfg());
        assert_eq!(r.outcome, Outcome::NoData);
        assert_eq!(score_trade(&flat, Action::Buy, None, None, None, &cfg()).outcome, Outcome::NoSlTp);
    }

    fn plan() -> ConditionalPlan {
        ConditionalPlan {
            wait_for: "x".into(),
            then_action: Action::Buy,
            invalidate_if: "y".into(),
            expires_at: "2026-09-15 10:00:00 UTC".into(),
            trigger_price: Some(154.24),
            trigger_condition: Some(PriceCondition::CloseAbove),
            invalidate_price: Some(154.07),
            invalidate_condition: Some(PriceCondition::CloseBelow),
            stop_loss: Some(154.07),
            take_profit: Some(154.60),
        }
    }

    #[test]
    fn plan_triggers_then_scores_trade() {
        let b = bars(&[
            (154.22, 154.15, 154.20), // 未成立
            (154.30, 154.18, 154.26), // 成立 (close > 154.24) -> entry 154.26
            (154.65, 154.25, 154.60), // TP
        ]);
        let r = evaluate_plan(&b, &plan(), None, &cfg());
        assert_eq!(r.outcome, Outcome::TpHit);
        assert_eq!(r.entry_price, Some(154.26));
        assert_eq!(r.bars_to_exit, Some(3));
    }

    #[test]
    fn plan_invalidated_expired_and_unstructured() {
        let b = bars(&[(154.10, 154.00, 154.05)]);
        assert_eq!(evaluate_plan(&b, &plan(), None, &cfg()).outcome, Outcome::PlanInvalidated);

        let b = bars(&[(154.22, 154.15, 154.20); 3]);
        let exp = Utc.with_ymd_and_hms(2026, 9, 15, 9, 10, 0).unwrap();
        assert_eq!(evaluate_plan(&b, &plan(), Some(exp), &cfg()).outcome, Outcome::PlanExpired);

        let mut p = plan();
        p.trigger_price = None;
        assert_eq!(evaluate_plan(&b, &p, None, &cfg()).outcome, Outcome::PlanUnstructured);
    }
}
