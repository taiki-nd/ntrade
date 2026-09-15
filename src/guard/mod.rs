//! 事後ガード: LLM の出力を発注前に機械的に検証する。
//!
//! 入力前に相場観を絞る「事前判定」は置かない。ここはリスク管理であり、数値は設定ファイルで固定する。
//! 弾いた場合も、どのガードで弾いたかを必ず記録する（ガード自体の妥当性をリプレイで検証するため）。

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::snapshot::MarketSnapshot;
use crate::strategy::types::{Action, ConditionalPlan, TradeDecision};

pub const DEFAULT_GUARD_CONFIG_PATH: &str = "config/guard.toml";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsBlackout {
    /// "YYYY-MM-DD HH:MM:SS" (UTC)
    pub time: String,
    pub before_min: i64,
    pub after_min: i64,
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GuardConfig {
    pub observed_check: bool,
    pub min_confidence: f64,
    pub min_rr: f64,
    pub sl_atr_min: f64,
    pub sl_atr_max: f64,
    pub sl_min_pips: f64,
    pub max_positions_per_pair: usize,
    pub max_positions_total: usize,
    pub daily_loss_limit_pct: f64,
    pub plan_max_hours: f64,
    pub fixed_volume_lots: f64,
    pub risk_pct: Option<f64>,
    pub pip_value_per_lot: Option<f64>,
    pub max_spread_pips: HashMap<String, f64>,
    pub news_blackout: Vec<NewsBlackout>,
}

impl Default for GuardConfig {
    fn default() -> Self {
        Self {
            observed_check: true,
            min_confidence: 0.70,
            min_rr: 1.5,
            sl_atr_min: 0.5,
            sl_atr_max: 4.0,
            sl_min_pips: 3.0,
            max_positions_per_pair: 1,
            max_positions_total: 1,
            daily_loss_limit_pct: 3.0,
            plan_max_hours: 8.0,
            fixed_volume_lots: 0.01,
            risk_pct: None,
            pip_value_per_lot: None,
            max_spread_pips: HashMap::from([("default".to_string(), 1.0)]),
            news_blackout: Vec::new(),
        }
    }
}

impl GuardConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let text = std::fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read guard config {:?}", path.as_ref()))?;
        toml::from_str(&text).context("Failed to parse guard config")
    }

    /// ファイルが無ければデフォルト
    pub fn load_or_default(path: impl AsRef<Path>) -> Self {
        Self::load(path).unwrap_or_default()
    }

    pub fn max_spread_for(&self, pair: &str) -> f64 {
        self.max_spread_pips
            .get(&pair.to_uppercase())
            .or_else(|| self.max_spread_pips.get("default"))
            .copied()
            .unwrap_or(1.0)
    }

    /// 発注ロット。risk_pct と pip_value_per_lot が設定されていれば SL 幅から算出。
    pub fn volume_lots(&self, balance: f64, sl_pips: f64) -> f64 {
        match (self.risk_pct, self.pip_value_per_lot) {
            (Some(r), Some(pv)) if sl_pips > 0.0 && pv > 0.0 => {
                let risk_amount = balance * r / 100.0;
                let lots = risk_amount / (sl_pips * pv);
                (lots * 100.0).floor() / 100.0
            }
            _ => self.fixed_volume_lots,
        }
        .max(0.01)
    }
}

/// ガード評価に必要な口座・環境の状態
#[derive(Debug, Clone, Default)]
pub struct GuardContext {
    pub open_positions_pair: usize,
    pub open_positions_total: usize,
    /// 当日の確定損益（資金比 %、損失は負）
    pub daily_pnl_pct: f64,
    /// 評価時刻（None なら Snapshot の時刻）
    pub now: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GuardCheck {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GuardVerdict {
    pub passed: bool,
    pub failed: Vec<String>,
    pub checks: Vec<GuardCheck>,
}

impl GuardVerdict {
    /// "PASS" または失敗ガード名を "|" で連結
    pub fn summary(&self) -> String {
        if self.passed {
            "PASS".to_string()
        } else {
            self.failed.join("|")
        }
    }
}

struct Checker {
    checks: Vec<GuardCheck>,
}

impl Checker {
    fn add(&mut self, name: &str, passed: bool, detail: impl Into<String>) {
        self.checks.push(GuardCheck { name: name.to_string(), passed, detail: detail.into() });
    }
    fn finish(self) -> GuardVerdict {
        let failed: Vec<String> = self.checks.iter().filter(|c| !c.passed).map(|c| c.name.clone()).collect();
        GuardVerdict { passed: failed.is_empty(), failed, checks: self.checks }
    }
}

fn parse_utc(s: &str) -> Option<DateTime<Utc>> {
    crate::storage::parse_ts(s).ok().and_then(|secs| Utc.timestamp_opt(secs, 0).single())
}

fn snapshot_time(snapshot: &MarketSnapshot) -> Option<DateTime<Utc>> {
    parse_utc(&snapshot.timestamp)
}

/// 全ガードを評価する。決定の種類（BUY/SELL/HOLD/プラン付きHOLD）に応じて適用範囲が変わる。
pub fn evaluate(decision: &TradeDecision, snapshot: &MarketSnapshot, ctx: &GuardContext, cfg: &GuardConfig) -> GuardVerdict {
    let mut c = Checker { checks: Vec::new() };
    let now = ctx.now.or_else(|| snapshot_time(snapshot)).unwrap_or_else(Utc::now);

    // 1. 観測整合（全決定に適用）
    if cfg.observed_check {
        let lb = &snapshot.latest_bars;
        let ob = &decision.observed;
        let pairs = [
            ("4H", &lb.h4, &ob.latest_bar_4h),
            ("1H", &lb.h1, &ob.latest_bar_1h),
            ("15M", &lb.m15, &ob.latest_bar_15m),
            ("5M", &lb.m5, &ob.latest_bar_5m),
        ];
        let mismatches: Vec<String> = pairs
            .iter()
            .filter(|(_, exp, got)| exp.is_some() && *exp != *got)
            .map(|(tf, exp, got)| format!("{tf}: expected {:?} got {:?}", exp, got))
            .collect();
        c.add("OBSERVED_MISMATCH", mismatches.is_empty(), mismatches.join("; "));
    }

    let is_trade = matches!(decision.action, Action::Buy | Action::Sell);
    let plan = decision.conditional_plan.as_ref().filter(|p| p.then_action != Action::Hold);

    // 2. 即時エントリーの検証
    if is_trade {
        c.add(
            "CONFIDENCE_LOW",
            decision.confidence >= cfg.min_confidence,
            format!("confidence {:.2} < min {:.2}", decision.confidence, cfg.min_confidence),
        );
        match (decision.entry_price, decision.stop_loss, decision.take_profit) {
            (Some(entry), Some(sl), Some(tp)) => {
                check_levels(&mut c, decision.action, entry, sl, tp, snapshot, cfg);
            }
            _ => c.add("NO_SL_TP", false, "entry/stop_loss/take_profit must all be set for BUY/SELL"),
        }
        check_environment(&mut c, snapshot, ctx, cfg, now);
    }

    // 3. 条件付きプランの検証（成立時に同じ検証を再度通す前提で、構造と期限だけ見る）
    if let Some(p) = plan {
        check_plan(&mut c, p, snapshot, cfg, now);
    }

    c.finish()
}

/// 条件付きプランが成立した瞬間に、実際の成立価格で行う検証
pub fn evaluate_plan_trigger(
    plan: &ConditionalPlan,
    entry: f64,
    snapshot: &MarketSnapshot,
    ctx: &GuardContext,
    cfg: &GuardConfig,
) -> GuardVerdict {
    let mut c = Checker { checks: Vec::new() };
    let now = ctx.now.or_else(|| snapshot_time(snapshot)).unwrap_or_else(Utc::now);
    match (plan.stop_loss, plan.take_profit) {
        (Some(sl), Some(tp)) => check_levels(&mut c, plan.then_action, entry, sl, tp, snapshot, cfg),
        _ => c.add("NO_SL_TP", false, "plan.stop_loss / take_profit must be set"),
    }
    check_environment(&mut c, snapshot, ctx, cfg, now);
    c.finish()
}

fn check_levels(c: &mut Checker, action: Action, entry: f64, sl: f64, tp: f64, snapshot: &MarketSnapshot, cfg: &GuardConfig) {
    let pip = snapshot.pip_size.max(1e-9);
    let sign = if action == Action::Buy { 1.0 } else { -1.0 };
    let sl_dist = (entry - sl) * sign;
    let tp_dist = (tp - entry) * sign;

    c.add("SL_WRONG_SIDE", sl_dist > 0.0, format!("entry {entry} sl {sl} action {:?}", action));
    c.add("TP_WRONG_SIDE", tp_dist > 0.0, format!("entry {entry} tp {tp} action {:?}", action));
    if sl_dist <= 0.0 || tp_dist <= 0.0 {
        return;
    }

    let sl_pips = sl_dist / pip;
    let rr = tp_dist / sl_dist;
    c.add("RR_TOO_LOW", rr >= cfg.min_rr, format!("rr {:.2} < min {:.2}", rr, cfg.min_rr));
    c.add("SL_TOO_TIGHT", sl_pips >= cfg.sl_min_pips, format!("sl {:.1} pips < min {:.1}", sl_pips, cfg.sl_min_pips));

    match snapshot.volatility.atr14_5m_pips {
        Some(atr) if atr > 0.0 => {
            let ratio = sl_pips / atr;
            c.add(
                "SL_ATR_RANGE",
                ratio >= cfg.sl_atr_min && ratio <= cfg.sl_atr_max,
                format!("sl {:.1} pips = {:.2} x ATR5M({:.1}); allowed {:.1}..{:.1}", sl_pips, ratio, atr, cfg.sl_atr_min, cfg.sl_atr_max),
            );
        }
        _ => c.add("SL_ATR_RANGE", true, "ATR unavailable; skipped"),
    }
}

fn check_environment(c: &mut Checker, snapshot: &MarketSnapshot, ctx: &GuardContext, cfg: &GuardConfig, now: DateTime<Utc>) {
    let max_spread = cfg.max_spread_for(&snapshot.pair);
    c.add(
        "SPREAD_TOO_WIDE",
        snapshot.spread_pips <= max_spread,
        format!("spread {:.1} > max {:.1}", snapshot.spread_pips, max_spread),
    );
    c.add(
        "MAX_POSITIONS",
        ctx.open_positions_pair < cfg.max_positions_per_pair && ctx.open_positions_total < cfg.max_positions_total,
        format!("open pair={} total={} (limits {}/{})", ctx.open_positions_pair, ctx.open_positions_total, cfg.max_positions_per_pair, cfg.max_positions_total),
    );
    c.add(
        "DAILY_LOSS_LIMIT",
        ctx.daily_pnl_pct > -cfg.daily_loss_limit_pct,
        format!("daily pnl {:.2}% <= -{:.2}%", ctx.daily_pnl_pct, cfg.daily_loss_limit_pct),
    );
    let blackout = cfg.news_blackout.iter().find(|b| {
        parse_utc(&b.time)
            .map(|t| now >= t - Duration::minutes(b.before_min) && now <= t + Duration::minutes(b.after_min))
            .unwrap_or(false)
    });
    c.add(
        "NEWS_BLACKOUT",
        blackout.is_none(),
        blackout.map(|b| format!("{} @ {}", b.label, b.time)).unwrap_or_default(),
    );
}

fn check_plan(c: &mut Checker, plan: &ConditionalPlan, snapshot: &MarketSnapshot, cfg: &GuardConfig, now: DateTime<Utc>) {
    c.add("PLAN_UNSTRUCTURED", plan.is_structured(), "trigger/invalidate price+condition must be set");
    c.add("PLAN_NO_SL_TP", plan.stop_loss.is_some() && plan.take_profit.is_some(), "plan stop_loss/take_profit must be set");
    match parse_utc(&plan.expires_at) {
        Some(exp) => {
            let max = now + Duration::minutes((cfg.plan_max_hours * 60.0) as i64);
            c.add(
                "PLAN_EXPIRY",
                exp > now && exp <= max,
                format!("expires_at {} must be within ({}, {}]", plan.expires_at, now, max),
            );
        }
        None => c.add("PLAN_EXPIRY", false, format!("unparsable expires_at {:?}", plan.expires_at)),
    }
    // SL の向きは trigger 価格を仮のエントリーとして確認
    if let (Some(trigger), Some(sl), Some(tp)) = (plan.trigger_price, plan.stop_loss, plan.take_profit) {
        let sign = if plan.then_action == Action::Buy { 1.0 } else { -1.0 };
        c.add("PLAN_SL_WRONG_SIDE", (trigger - sl) * sign > 0.0, format!("trigger {trigger} sl {sl}"));
        c.add("PLAN_TP_WRONG_SIDE", (tp - trigger) * sign > 0.0, format!("trigger {trigger} tp {tp}"));
        let _ = snapshot;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{AccountState, SnapshotConfig, SnapshotInput};
    use crate::strategy::types::{Analysis, EntryType, Observed, PriceCondition};
    use chrono::TimeZone;

    fn snapshot() -> MarketSnapshot {
        let end = Utc.with_ymd_and_hms(2026, 9, 15, 9, 0, 0).unwrap();
        let bars: Vec<crate::ctrader::CandleBar> = (0..40)
            .map(|i| {
                let t = end - Duration::minutes(5 * (39 - i));
                let p = 154.0 + (i as f64 * 0.2).sin() * 0.05;
                crate::ctrader::CandleBar { timestamp: t, open: p, high: p + 0.04, low: p - 0.04, close: p + 0.01, volume: 1 }
            })
            .collect();
        MarketSnapshot::build(
            &SnapshotInput {
                pair: "USDJPY",
                bars_4h: &[],
                bars_1h: &[],
                bars_15m: &[],
                bars_5m: &bars,
                spread_pips: 0.3,
                current_price: None,
                now: Some(end + Duration::minutes(5)),
                account_state: AccountState::default(),
            },
            &SnapshotConfig::default(),
        )
    }

    fn decision(snap: &MarketSnapshot) -> TradeDecision {
        TradeDecision {
            action: Action::Buy,
            confidence: 0.8,
            entry_type: Some(EntryType::Market),
            entry_price: Some(154.00),
            stop_loss: Some(153.92),  // 8 pips
            take_profit: Some(154.16), // RR 2.0
            analysis: Analysis { macro_context: "a".into(), order_flow: "b".into(), invalidation: "c".into(), conflicts: "d".into() },
            conditional_plan: None,
            observed: Observed {
                latest_bar_4h: snap.latest_bars.h4.clone(),
                latest_bar_1h: snap.latest_bars.h1.clone(),
                latest_bar_15m: snap.latest_bars.m15.clone(),
                latest_bar_5m: snap.latest_bars.m5.clone(),
            },
            reasoning: "r".into(),
        }
    }

    #[test]
    fn good_trade_passes() {
        let snap = snapshot();
        let v = evaluate(&decision(&snap), &snap, &GuardContext::default(), &GuardConfig::default());
        assert!(v.passed, "{:?}", v.failed);
        assert_eq!(v.summary(), "PASS");
    }

    #[test]
    fn each_guard_fires() {
        let snap = snapshot();
        let cfg = GuardConfig::default();

        let mut d = decision(&snap);
        d.confidence = 0.5;
        assert!(evaluate(&d, &snap, &GuardContext::default(), &cfg).failed.contains(&"CONFIDENCE_LOW".to_string()));

        let mut d = decision(&snap);
        d.take_profit = Some(154.05); // RR 0.6
        assert!(evaluate(&d, &snap, &GuardContext::default(), &cfg).failed.contains(&"RR_TOO_LOW".to_string()));

        let mut d = decision(&snap);
        d.stop_loss = Some(153.99); // 1 pip
        d.take_profit = Some(154.03);
        let f = evaluate(&d, &snap, &GuardContext::default(), &cfg).failed;
        assert!(f.contains(&"SL_TOO_TIGHT".to_string()) && f.contains(&"SL_ATR_RANGE".to_string()));

        let mut d = decision(&snap);
        d.stop_loss = Some(154.10);
        assert!(evaluate(&d, &snap, &GuardContext::default(), &cfg).failed.contains(&"SL_WRONG_SIDE".to_string()));

        let mut d = decision(&snap);
        d.observed.latest_bar_5m = Some("2000-01-01 00:00".into());
        assert!(evaluate(&d, &snap, &GuardContext::default(), &cfg).failed.contains(&"OBSERVED_MISMATCH".to_string()));

        let ctx = GuardContext { open_positions_total: 1, ..Default::default() };
        assert!(evaluate(&decision(&snap), &snap, &ctx, &cfg).failed.contains(&"MAX_POSITIONS".to_string()));

        let ctx = GuardContext { daily_pnl_pct: -3.5, ..Default::default() };
        assert!(evaluate(&decision(&snap), &snap, &ctx, &cfg).failed.contains(&"DAILY_LOSS_LIMIT".to_string()));

        let mut wide = snap.clone();
        wide.spread_pips = 2.0;
        assert!(evaluate(&decision(&snap), &wide, &GuardContext::default(), &cfg).failed.contains(&"SPREAD_TOO_WIDE".to_string()));

        let mut cfg2 = cfg.clone();
        cfg2.news_blackout.push(NewsBlackout { time: "2026-09-15 09:20:00".into(), before_min: 30, after_min: 15, label: "CPI".into() });
        assert!(evaluate(&decision(&snap), &snap, &GuardContext::default(), &cfg2).failed.contains(&"NEWS_BLACKOUT".to_string()));
    }

    #[test]
    fn hold_with_plan_checks_plan_only() {
        let snap = snapshot();
        let mut d = decision(&snap);
        d.action = Action::Hold;
        d.confidence = 0.4;
        d.entry_price = None;
        d.stop_loss = None;
        d.take_profit = None;
        d.conditional_plan = Some(ConditionalPlan {
            wait_for: "w".into(),
            then_action: Action::Buy,
            invalidate_if: "i".into(),
            expires_at: "2026-09-15 12:00:00 UTC".into(),
            trigger_price: Some(154.10),
            trigger_condition: Some(PriceCondition::CloseAbove),
            invalidate_price: Some(153.90),
            invalidate_condition: Some(PriceCondition::CloseBelow),
            stop_loss: Some(153.95),
            take_profit: Some(154.40),
        });
        let v = evaluate(&d, &snap, &GuardContext::default(), &GuardConfig::default());
        assert!(v.passed, "{:?}", v.failed);

        let mut late = d.clone();
        late.conditional_plan.as_mut().unwrap().expires_at = "2026-09-16 12:00:00 UTC".into();
        assert!(evaluate(&late, &snap, &GuardContext::default(), &GuardConfig::default()).failed.contains(&"PLAN_EXPIRY".to_string()));

        let tv = evaluate_plan_trigger(d.conditional_plan.as_ref().unwrap(), 154.11, &snap, &GuardContext::default(), &GuardConfig::default());
        assert!(tv.passed, "{:?}", tv.failed);
    }

    #[test]
    fn config_loads_from_repo_file_and_sizes_lots() {
        let cfg = GuardConfig::load(concat!(env!("CARGO_MANIFEST_DIR"), "/config/guard.toml")).unwrap();
        assert_eq!(cfg.max_spread_for("eurusd"), 1.2);
        assert_eq!(cfg.max_spread_for("GBPUSD"), 1.0);
        assert_eq!(cfg.volume_lots(1_000_000.0, 10.0), 0.01);
        let sized = GuardConfig { risk_pct: Some(0.5), pip_value_per_lot: Some(1000.0), ..cfg };
        assert!((sized.volume_lots(1_000_000.0, 10.0) - 0.5).abs() < 1e-9);
    }
}
