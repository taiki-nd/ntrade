//! 採点: 判断時刻以降の5M足で結果を機械的に判定する。

use serde::{Deserialize, Serialize};

use crate::ctrader::CandleBar;
use crate::strategy::types::{Action, ConditionalPlan, TradeDecision};

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
    /// 条件付きプランは成立したが、再判断で LLM が入らなかった（HOLD）
    PlanDeclined,
    /// 条件付きプランは成立し、再判断で入ろうとしたが事後ガードで止まった
    PlanRejected,
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
            Outcome::PlanDeclined => "PLAN_DECLINED",
            Outcome::PlanRejected => "PLAN_REJECTED",
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

/// SL の判定方法（本番の執行と同じ定義を使う）
pub use crate::guard::StopMode;

/// 決済ルール。LLM の判断はそのままに、SL の執行方法だけを変えて採点するためのもの。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ExitRule {
    pub stop_mode: StopMode,
    /// SL を ATR(5M,14) の何倍だけ外側へずらすか
    pub sl_buffer_atr: f64,
    /// `Close` のときのハードSL: バッファ込みの SL からさらに ATR(5M,14) の何倍外側に置くか
    pub hard_stop_atr: f64,
}

impl Default for ExitRule {
    fn default() -> Self {
        Self { stop_mode: StopMode::Touch, sl_buffer_atr: 0.0, hard_stop_atr: 2.0 }
    }
}

impl ExitRule {
    /// `touch`, `touch:0.5`, `close:0.5:2` の形式を解釈する（数値は ATR 倍率）
    pub fn parse(s: &str) -> Option<Self> {
        let mut it = s.split(':');
        let stop_mode = match it.next()?.trim() {
            "touch" => StopMode::Touch,
            "close" => StopMode::Close,
            _ => return None,
        };
        let mut rule = Self { stop_mode, ..Self::default() };
        if let Some(v) = it.next() {
            rule.sl_buffer_atr = v.trim().parse().ok().filter(|x: &f64| *x >= 0.0)?;
        }
        if let Some(v) = it.next() {
            if stop_mode != StopMode::Close {
                return None;
            }
            rule.hard_stop_atr = v.trim().parse().ok().filter(|x: &f64| *x >= 0.0)?;
        }
        if it.next().is_some() {
            return None;
        }
        Some(rule)
    }

    pub fn label(&self) -> String {
        match self.stop_mode {
            StopMode::Touch => format!("touch:{}", self.sl_buffer_atr),
            StopMode::Close => format!("close:{}:{}", self.sl_buffer_atr, self.hard_stop_atr),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ScoreConfig {
    pub pip_size: f64,
    /// スプレッド（pips）。損益から差し引く
    pub spread_pips: f64,
    /// 何本以内に決着しなければ TIMEOUT にするか
    pub max_bars: usize,
    pub exit: ExitRule,
    /// 判断時刻の ATR(5M,14)（価格単位）。取れない場合は 0 とし、バッファとハードSLの幅は 0 になる
    pub atr: f64,
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
    let done = |outcome: Outcome, exit: f64, i: usize| ScoreResult {
        outcome,
        pnl_pips: Some(pnl(exit)),
        bars_to_exit: Some(i + 1),
        entry_price: Some(entry),
    };

    // SL は建値の反対側にあるので、「外側へずらす」は BUY なら下、SELL なら上
    let stop = sl - sign * cfg.exit.sl_buffer_atr * cfg.atr;
    let close_mode = cfg.exit.stop_mode == StopMode::Close;
    let hard = if close_mode { stop - sign * cfg.exit.hard_stop_atr * cfg.atr } else { stop };

    for (i, b) in bars.iter().take(cfg.max_bars).enumerate() {
        let hit_hard = if sign > 0.0 { b.low <= hard } else { b.high >= hard };
        let hit_tp = if sign > 0.0 { b.high >= tp } else { b.low <= tp };
        let closed_out = close_mode && if sign > 0.0 { b.close < stop } else { b.close > stop };
        // 同一足内の順序は分からないので、TP と損切りが重なったら不利側に倒す
        match (hit_hard, hit_tp, closed_out) {
            (true, true, _) => return done(Outcome::SameBar, hard, i),
            (true, false, _) => return done(Outcome::SlHit, hard, i),
            (false, true, true) => return done(Outcome::SameBar, b.close, i),
            (false, true, false) => return done(Outcome::TpHit, tp, i),
            (false, false, true) => return done(Outcome::SlHit, b.close, i),
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

/// 条件付きプランを判断時刻以降の5M足（昇順）で進め、成立した足の位置を返す。
/// 成立しなければ、その結末（期限切れ・破棄・評価不能・データ切れ）を返す。
/// 本番の Executor と同じ `step_plan` で各確定足を評価する。
pub fn plan_trigger_index(bars: &[CandleBar], plan: &ConditionalPlan) -> Result<usize, Outcome> {
    use crate::executor::{step_plan, PlanStep};

    if !plan.is_structured() {
        return Err(Outcome::PlanUnstructured);
    }
    for (i, b) in bars.iter().enumerate() {
        match step_plan(plan, b) {
            PlanStep::Waiting => {}
            PlanStep::Expired => return Err(Outcome::PlanExpired),
            PlanStep::Invalidated => return Err(Outcome::PlanInvalidated),
            PlanStep::Unstructured => return Err(Outcome::PlanUnstructured),
            PlanStep::Triggered { .. } => return Ok(i),
        }
    }
    // 期限前にデータが尽きた
    Err(Outcome::NoData)
}

/// 条件付きプランを機械的に執行したとみなす採点（成立足の終値で入り、プランの SL/TP を使う）。
/// 本番は成立時に LLM が判断し直すので、それと比べるための旧方式。
pub fn evaluate_plan(bars: &[CandleBar], plan: &ConditionalPlan, cfg: &ScoreConfig) -> ScoreResult {
    let i = match plan_trigger_index(bars, plan) {
        Ok(i) => i,
        Err(outcome) => return ScoreResult::flat(outcome),
    };
    let (Some(sl), Some(tp)) = (plan.stop_loss, plan.take_profit) else {
        return ScoreResult::flat(Outcome::NoSlTp);
    };
    let rest = &bars[i + 1..];
    if rest.is_empty() {
        return ScoreResult::flat(Outcome::NoData);
    }
    let mut r = score_from(rest, plan.then_action, bars[i].close, sl, tp, cfg);
    r.bars_to_exit = r.bars_to_exit.map(|n| n + i + 1);
    r
}

/// プラン成立時の再判断の採点。`bars` は再判断の時刻（成立足の確定時刻）以降の5M足。
/// `offset` はプランを立てた判断から成立足までの本数で、`bars_to_exit` に足し込む。
pub fn score_followup(bars: &[CandleBar], followup: &TradeDecision, guard_passed: bool, offset: usize, cfg: &ScoreConfig) -> ScoreResult {
    if followup.action == Action::Hold {
        return ScoreResult::flat(Outcome::PlanDeclined);
    }
    if !guard_passed {
        return ScoreResult::flat(Outcome::PlanRejected);
    }
    let mut r = score_trade(bars, followup.action, followup.entry_price, followup.stop_loss, followup.take_profit, cfg);
    r.bars_to_exit = r.bars_to_exit.map(|n| n + offset);
    r
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
            Some(plan) if plan.then_action != Action::Hold => evaluate_plan(bars_after, plan, cfg),
            _ => ScoreResult::flat(Outcome::Hold),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategy::types::PriceCondition;
    use chrono::{Duration, TimeZone, Utc};

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
        ScoreConfig { pip_size: 0.01, spread_pips: 0.2, max_bars: 4, exit: ExitRule::default(), atr: 0.0 }
    }

    fn cfg_with(exit: &str, atr: f64) -> ScoreConfig {
        ScoreConfig { exit: ExitRule::parse(exit).unwrap(), atr, ..cfg() }
    }

    #[test]
    fn exit_rule_parse_and_label() {
        assert_eq!(ExitRule::parse("touch"), Some(ExitRule::default()));
        let r = ExitRule::parse("close:0.5:3").unwrap();
        assert_eq!((r.stop_mode, r.sl_buffer_atr, r.hard_stop_atr), (StopMode::Close, 0.5, 3.0));
        assert_eq!(ExitRule::parse(&r.label()), Some(r));
        assert_eq!(ExitRule::parse("touch:0.5:2"), None);
        assert_eq!(ExitRule::parse("touch:-1"), None);
        assert_eq!(ExitRule::parse("wick"), None);
    }

    #[test]
    fn sl_buffer_survives_a_wick() {
        // SELL 154.20 / SL 154.30 / TP 154.00。1本目の高値 154.31 がヒゲで SL を刺し、2本目で TP
        let b = bars(&[(154.31, 154.15, 154.20), (154.22, 153.98, 154.00)]);
        assert_eq!(score_trade(&b, Action::Sell, Some(154.20), Some(154.30), Some(154.00), &cfg()).outcome, Outcome::SlHit);

        // ATR 0.04 の 0.5 倍 = 2pips 外側へずらすと SL は 154.32 になり、ヒゲを耐える
        let r = score_trade(&b, Action::Sell, Some(154.20), Some(154.30), Some(154.00), &cfg_with("touch:0.5", 0.04));
        assert_eq!(r.outcome, Outcome::TpHit);

        // ずらした先で刺さった場合の損失はバッファ分だけ大きい
        let b = bars(&[(154.33, 154.15, 154.20)]);
        let r = score_trade(&b, Action::Sell, Some(154.20), Some(154.30), Some(154.00), &cfg_with("touch:0.5", 0.04));
        assert_eq!(r.outcome, Outcome::SlHit);
        assert!((r.pnl_pips.unwrap() - (-12.0 - 0.2)).abs() < 1e-6);
    }

    #[test]
    fn close_mode_ignores_wicks_and_exits_at_close() {
        let exit = "close:0:2"; // ハードSL = 154.30 + 2 × 0.04 = 154.38
        // ヒゲで SL を越えても終値が内側なら続行し、TP に届く
        let b = bars(&[(154.35, 154.15, 154.25), (154.22, 153.98, 154.00)]);
        let r = score_trade(&b, Action::Sell, Some(154.20), Some(154.30), Some(154.00), &cfg_with(exit, 0.04));
        assert_eq!(r.outcome, Outcome::TpHit);

        // 終値が SL を越えたら、その足の終値で決済
        let b = bars(&[(154.36, 154.15, 154.34)]);
        let r = score_trade(&b, Action::Sell, Some(154.20), Some(154.30), Some(154.00), &cfg_with(exit, 0.04));
        assert_eq!(r.outcome, Outcome::SlHit);
        assert!((r.pnl_pips.unwrap() - (-14.0 - 0.2)).abs() < 1e-6);

        // ハードSLに触れたら終値を待たずにハードSL価格で決済
        let b = bars(&[(154.40, 154.15, 154.25)]);
        let r = score_trade(&b, Action::Sell, Some(154.20), Some(154.30), Some(154.00), &cfg_with(exit, 0.04));
        assert_eq!(r.outcome, Outcome::SlHit);
        assert!((r.pnl_pips.unwrap() - (-18.0 - 0.2)).abs() < 1e-6);

        // TP に触れた足の終値が SL の外なら、順序不明として不利側（終値決済）に倒す
        let b = bars(&[(154.36, 153.98, 154.33)]);
        let r = score_trade(&b, Action::Sell, Some(154.20), Some(154.30), Some(154.00), &cfg_with(exit, 0.04));
        assert_eq!(r.outcome, Outcome::SameBar);
        assert!((r.pnl_pips.unwrap() - (-13.0 - 0.2)).abs() < 1e-6);
    }

    #[test]
    fn close_mode_buy_side() {
        // BUY 154.20 / SL 154.10 / TP 154.40。安値 154.05 のヒゲは耐え、終値 154.08 で決済
        let b = bars(&[(154.25, 154.05, 154.15), (154.20, 154.06, 154.08)]);
        let r = score_trade(&b, Action::Buy, Some(154.20), Some(154.10), Some(154.40), &cfg_with("close:0:2", 0.04));
        assert_eq!(r.outcome, Outcome::SlHit);
        assert_eq!(r.bars_to_exit, Some(2));
        assert!((r.pnl_pips.unwrap() - (-12.0 - 0.2)).abs() < 1e-6);
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
        let r = evaluate_plan(&b, &plan(), &cfg());
        assert_eq!(r.outcome, Outcome::TpHit);
        assert_eq!(r.entry_price, Some(154.26));
        assert_eq!(r.bars_to_exit, Some(3));
    }

    #[test]
    fn plan_invalidated_expired_and_unstructured() {
        let b = bars(&[(154.10, 154.00, 154.05)]);
        assert_eq!(evaluate_plan(&b, &plan(), &cfg()).outcome, Outcome::PlanInvalidated);

        // bars() は 09:00 から 5 分刻み。期限を 09:10 にすると 3 本目で期限切れ
        let b = bars(&[(154.22, 154.15, 154.20); 3]);
        let mut p = plan();
        p.expires_at = "2026-09-15 09:10:00 UTC".into();
        assert_eq!(evaluate_plan(&b, &p, &cfg()).outcome, Outcome::PlanExpired);

        let mut p = plan();
        p.trigger_price = None;
        assert_eq!(evaluate_plan(&b, &p, &cfg()).outcome, Outcome::PlanUnstructured);
    }

    #[test]
    fn plan_trigger_index_points_at_the_triggering_bar() {
        let b = bars(&[(154.22, 154.15, 154.20), (154.30, 154.18, 154.26), (154.65, 154.25, 154.60)]);
        assert_eq!(plan_trigger_index(&b, &plan()), Ok(1));
        assert_eq!(plan_trigger_index(&b[..1], &plan()), Err(Outcome::NoData));
    }

    /// 再判断が HOLD なら見送り、ガードで止まれば不成立。入るなら再判断の価格と SL/TP で採点する
    #[test]
    fn followup_is_scored_with_its_own_levels() {
        let mut d = TradeDecision::fallback_hold("t");
        assert_eq!(score_followup(&[], &d, true, 2, &cfg()).outcome, Outcome::PlanDeclined);

        d.action = Action::Buy;
        d.entry_price = Some(154.30);
        d.stop_loss = Some(154.10);
        d.take_profit = Some(154.60);
        assert_eq!(score_followup(&[], &d, false, 2, &cfg()).outcome, Outcome::PlanRejected);

        let b = bars(&[(154.40, 154.25, 154.35), (154.65, 154.30, 154.60)]);
        let r = score_followup(&b, &d, true, 2, &cfg());
        assert_eq!(r.outcome, Outcome::TpHit);
        assert_eq!(r.entry_price, Some(154.30));
        assert_eq!(r.bars_to_exit, Some(4));
    }
}
