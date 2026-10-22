//! 採点: 判断時刻以降の5M足で結果を機械的に判定する。
//!
//! 予約注文は「期限内に価格が届いたか（約定）」→「約定後に SL と TP のどちらに先に届いたか」の順に見る。

use serde::{Deserialize, Serialize};

use crate::ctrader::CandleBar;
use crate::strategy::types::{Action, EntryType, TradeDecision};

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
    /// HOLD（新しい注文なし）
    Hold,
    /// 予約注文が期限内に約定しなかった
    OrderExpired,
    /// 旧: 条件付きプランが期限内に成立しなかった（以下 Plan* は保存済みの run を読むためだけに残す）
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
            Outcome::OrderExpired => "ORDER_EXPIRED",
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
    /// 実際にエントリーしたとみなした価格（逆指値が窓を空けて約定した場合は約定足の始値）
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

/// 予約注文が約定する足と約定価格。届かなければ None。
///
/// - 指値: 価格まで届いたら約定（BUY は安値が、SELL は高値が届く）。約定価格は指値（有利な窓開けは見込まない）
/// - 逆指値: 価格を抜けたら約定。窓を空けて抜けたら始値で約定する（不利側の滑り）
pub fn pending_fill(bars: &[CandleBar], action: Action, kind: EntryType, price: f64) -> Option<(usize, f64)> {
    let buy = action == Action::Buy;
    for (i, b) in bars.iter().enumerate() {
        match kind {
            EntryType::Limit if (buy && b.low <= price) || (!buy && b.high >= price) => return Some((i, price)),
            EntryType::Stop if buy && b.high >= price => return Some((i, b.open.max(price))),
            EntryType::Stop if !buy && b.low <= price => return Some((i, b.open.min(price))),
            _ => {}
        }
    }
    None
}

/// 予約注文の採点。`bars` は判断時刻以降の5M足（昇順）、`expiry_bars` は期限までの本数。
///
/// 約定した足の中では、約定と SL/TP のどちらが先かは分からない。SL に触れていれば損切りとし、
/// TP にだけ触れていてもその足では利確とみなさない（不利側に倒す）
#[allow(clippy::too_many_arguments)]
pub fn score_pending(
    bars: &[CandleBar],
    action: Action,
    kind: EntryType,
    entry: f64,
    sl: f64,
    tp: f64,
    expiry_bars: usize,
    cfg: &ScoreConfig,
) -> ScoreResult {
    let window = &bars[..bars.len().min(expiry_bars)];
    let Some((i, fill)) = pending_fill(window, action, kind, entry) else {
        return ScoreResult::flat(if bars.len() < expiry_bars { Outcome::NoData } else { Outcome::OrderExpired });
    };
    // 約定足の TP 側の値動きは約定前のものかもしれないので、約定価格までしか動かなかったとみなす
    let mut fill_bar = bars[i].clone();
    if action == Action::Buy {
        fill_bar.high = fill;
    } else {
        fill_bar.low = fill;
    }
    let mut rest = Vec::with_capacity(bars.len() - i);
    rest.push(fill_bar);
    rest.extend_from_slice(&bars[i + 1..]);
    let mut r = score_from(&rest, action, fill, sl, tp, cfg);
    r.bars_to_exit = r.bars_to_exit.map(|n| n + i);
    r
}

/// TradeDecision 全体の採点（`bars_after` は 5M 足。注文の期限は 5 分 = 1 本で数える）
pub fn score_decision(bars_after: &[CandleBar], decision: &TradeDecision, cfg: &ScoreConfig) -> ScoreResult {
    match (decision.action, decision.entry_type) {
        (Action::Hold, _) => ScoreResult::flat(Outcome::Hold),
        (action, Some(kind @ (EntryType::Limit | EntryType::Stop))) => {
            let (Some(entry), Some(sl), Some(tp)) = (decision.entry_price, decision.stop_loss, decision.take_profit) else {
                return ScoreResult::flat(Outcome::NoSlTp);
            };
            // 期限の無い注文はガードで弾かれるが、仮に執行した場合の集計のために 1 時間とみなす
            let expiry_bars = (decision.expiry_minutes.unwrap_or(60) as usize).div_ceil(5).max(1);
            score_pending(bars_after, action, kind, entry, sl, tp, expiry_bars, cfg)
        }
        // 旧: 成行（保存済みの run の採点し直し用）
        (action, _) => score_trade(bars_after, action, decision.entry_price, decision.stop_loss, decision.take_profit, cfg),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

    fn pending(action: Action, kind: EntryType, entry: f64, sl: f64, tp: f64) -> TradeDecision {
        TradeDecision {
            action,
            entry_type: Some(kind),
            entry_price: Some(entry),
            stop_loss: Some(sl),
            take_profit: Some(tp),
            expiry_minutes: Some(15), // 3 本
            ..TradeDecision::fallback_hold("t")
        }
    }

    #[test]
    fn limit_waits_for_the_price_then_scores_from_the_fill() {
        let d = pending(Action::Buy, EntryType::Limit, 154.10, 154.00, 154.30);
        let b = bars(&[
            (154.25, 154.15, 154.20), // 未約定
            (154.20, 154.08, 154.12), // 安値が 154.10 に届いて約定
            (154.35, 154.11, 154.30), // TP
        ]);
        let r = score_decision(&b, &d, &cfg());
        assert_eq!(r.outcome, Outcome::TpHit);
        assert_eq!(r.entry_price, Some(154.10));
        assert_eq!(r.bars_to_exit, Some(3));
        assert!((r.pnl_pips.unwrap() - (20.0 - 0.2)).abs() < 1e-6);
    }

    #[test]
    fn unfilled_order_expires_and_fill_bar_is_judged_conservatively() {
        // 期限（3 本）までに 154.10 へ届かない
        let d = pending(Action::Buy, EntryType::Limit, 154.10, 154.00, 154.30);
        let b = bars(&[(154.25, 154.15, 154.20); 4]);
        assert_eq!(score_decision(&b, &d, &cfg()).outcome, Outcome::OrderExpired);
        // データが期限より短ければ結果不明
        assert_eq!(score_decision(&b[..2], &d, &cfg()).outcome, Outcome::NoData);

        // 約定した足で SL にも触れたら損切り
        let b = bars(&[(154.20, 153.98, 154.05)]);
        assert_eq!(score_decision(&b, &d, &cfg()).outcome, Outcome::SlHit);

        // 約定した足で TP にだけ触れても利確とはみなさない（次の足以降で判定）
        let b = bars(&[(154.32, 154.08, 154.20), (154.25, 154.15, 154.20), (154.25, 154.15, 154.20), (154.25, 154.15, 154.20)]);
        assert_eq!(score_decision(&b, &d, &cfg()).outcome, Outcome::Timeout);
    }

    #[test]
    fn stop_entry_slips_on_a_gap() {
        // SELL 逆指値 154.00。2 本目は始値 153.92（bars() は始値 = 終値）で窓を空けて下抜け → 153.92 で約定
        let d = pending(Action::Sell, EntryType::Stop, 154.00, 154.10, 153.80);
        let b = bars(&[(154.05, 154.01, 154.03), (153.96, 153.90, 153.92), (153.93, 153.78, 153.80)]);
        let r = score_decision(&b, &d, &cfg());
        assert_eq!(r.entry_price, Some(153.92));
        assert_eq!(r.outcome, Outcome::TpHit);
        assert_eq!(pending_fill(&b, Action::Sell, EntryType::Stop, 154.00), Some((1, 153.92)));
        assert_eq!(pending_fill(&b, Action::Buy, EntryType::Stop, 154.10), None);
    }
}
