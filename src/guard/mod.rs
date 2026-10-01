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

/// 損切りの執行方法
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StopMode {
    /// LLM の SL をそのままブローカーに置き、ヒゲが触れたら決済する
    #[default]
    Touch,
    /// LLM の SL は「5M 確定足の終値で越えたら無効」のラインとして ntrade が判定し、成行で決済する。
    /// ブローカーには急変に備えたハードSL（`hard_stop_atr` だけ外側）を置く。
    /// プロンプトが損切りを「実体で抜けたら無効」と定義しているのに合わせた執行方法
    Close,
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
    pub stop_mode: StopMode,
    /// `stop_mode = close` のとき、ブローカーに置くハードSLを LLM の SL から 5M ATR の何倍外側に置くか
    pub hard_stop_atr: f64,
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
            stop_mode: StopMode::Touch,
            hard_stop_atr: 2.0,
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

    /// ペア名は大文字小文字を区別せずに照合する（キーは画面で入力した表記のまま保存するため）
    pub fn max_spread_for(&self, pair: &str) -> f64 {
        self.max_spread_pips
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(pair))
            .or_else(|| self.max_spread_pips.get_key_value("default"))
            .map(|(_, v)| *v)
            .unwrap_or(1.0)
    }

    /// 発注ロット。risk_pct が設定されていれば SL 幅から算出する。
    ///
    /// 1 lot・1 pip の価値は `pip_value_per_lot`（設定での固定値）を優先し、無ければ
    /// ブローカーのレートから求めた `live_pip_value` を使う。どちらも無ければ固定ロット。
    pub fn volume_lots(&self, balance: f64, sl_pips: f64, live_pip_value: Option<f64>) -> f64 {
        match (self.risk_pct, self.pip_value_per_lot.or(live_pip_value)) {
            (Some(r), Some(pv)) if sl_pips > 0.0 && pv > 0.0 => {
                let risk_amount = balance * r / 100.0;
                let lots = risk_amount / (sl_pips * pv);
                (lots * 100.0).floor() / 100.0
            }
            _ => self.fixed_volume_lots,
        }
        .max(0.01)
    }

    /// 画面から保存する前の値チェック。明らかに運用できない値だけを弾く
    pub fn validate(&self) -> Result<()> {
        let mut errs: Vec<String> = Vec::new();
        let mut check = |ok: bool, msg: &str| {
            if !ok {
                errs.push(msg.to_string());
            }
        };
        check((0.0..=1.0).contains(&self.min_confidence), "min_confidence は 0〜1");
        check(self.min_rr > 0.0, "min_rr は 0 より大きい値");
        check(self.sl_atr_min >= 0.0 && self.sl_atr_min <= self.sl_atr_max, "sl_atr_min は 0 以上かつ sl_atr_max 以下");
        check(self.sl_min_pips >= 0.0, "sl_min_pips は 0 以上");
        check(self.max_positions_per_pair >= 1, "max_positions_per_pair は 1 以上");
        check(self.max_positions_total >= 1, "max_positions_total は 1 以上");
        check(self.daily_loss_limit_pct > 0.0, "daily_loss_limit_pct は 0 より大きい値");
        check(self.plan_max_hours > 0.0, "plan_max_hours は 0 より大きい値");
        check(self.fixed_volume_lots >= 0.01, "fixed_volume_lots は 0.01 以上");
        check(self.risk_pct.is_none_or(|r| r > 0.0 && r <= 10.0), "risk_pct は 0〜10（%）");
        check(self.pip_value_per_lot.is_none_or(|v| v > 0.0), "pip_value_per_lot は 0 より大きい値");
        check(self.max_spread_pips.values().all(|v| *v >= 0.0), "max_spread_pips は 0 以上");
        check(self.news_blackout.iter().all(|b| parse_utc(&b.time).is_some()), "news_blackout.time は YYYY-MM-DD HH:MM:SS（UTC）");
        check(self.news_blackout.iter().all(|b| b.before_min >= 0 && b.after_min >= 0), "news_blackout の前後分数は 0 以上");
        check(self.hard_stop_atr > 0.0 && self.hard_stop_atr <= 10.0, "hard_stop_atr は 0 より大きく 10 以下");
        if errs.is_empty() {
            Ok(())
        } else {
            anyhow::bail!(errs.join(" / "))
        }
    }

    /// スプレッド上限のキーの前後の空白を除き、default だけ小文字にそろえる（銘柄名の表記は保つ）
    pub fn normalize(&mut self) {
        self.max_spread_pips = std::mem::take(&mut self.max_spread_pips)
            .into_iter()
            .map(|(k, v)| if k.trim().eq_ignore_ascii_case("default") { ("default".to_string(), v) } else { (k.trim().to_string(), v) })
            .filter(|(k, _)| !k.is_empty())
            .collect();
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

    // 3. 条件付きプランの検証（成立時は LLM の再判断が同じ検証を通るので、構造と期限だけ見る）
    if let Some(p) = plan {
        check_plan(&mut c, p, snapshot, cfg, now);
    }

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
    }

    #[test]
    fn config_loads_from_repo_file_and_sizes_lots() {
        let cfg = GuardConfig::load(concat!(env!("CARGO_MANIFEST_DIR"), "/config/guard.toml")).unwrap();
        assert_eq!(cfg.max_spread_for("eurusd"), 1.2);
        assert_eq!(cfg.max_spread_for("GBPUSD"), 1.0);
        assert_eq!(cfg.volume_lots(1_000_000.0, 10.0, Some(1000.0)), 0.01);
        let sized = GuardConfig { risk_pct: Some(0.5), pip_value_per_lot: Some(1000.0), ..cfg.clone() };
        assert!((sized.volume_lots(1_000_000.0, 10.0, None) - 0.5).abs() < 1e-9);
        // 固定値が無ければブローカーのレートから求めた pip 価値で算出する（EURUSD・USDJPY=150 なら 1500 円）
        let live = GuardConfig { risk_pct: Some(0.5), pip_value_per_lot: None, ..cfg.clone() };
        assert!((live.volume_lots(1_000_000.0, 10.0, Some(1500.0)) - 0.33).abs() < 1e-9);
        assert_eq!(live.volume_lots(1_000_000.0, 10.0, None), 0.01);
    }

    #[test]
    fn stop_mode_is_read_from_saved_settings_and_defaults_to_touch() {
        let cfg: GuardConfig = serde_json::from_str(r#"{"stop_mode":"close","hard_stop_atr":2.5}"#).unwrap();
        assert_eq!((cfg.stop_mode, cfg.hard_stop_atr), (StopMode::Close, 2.5));
        // 項目追加前に保存された設定は従来どおり touch
        let old: GuardConfig = serde_json::from_str(r#"{"min_rr":1.5}"#).unwrap();
        assert_eq!(old.stop_mode, StopMode::Touch);
        assert!(GuardConfig { hard_stop_atr: 0.0, ..GuardConfig::default() }.validate().is_err());
    }

    #[test]
    fn validate_rejects_unusable_values() {
        assert!(GuardConfig::default().validate().is_ok());
        let bad = GuardConfig { min_confidence: 1.5, max_positions_total: 0, ..GuardConfig::default() };
        let err = bad.validate().unwrap_err().to_string();
        assert!(err.contains("min_confidence") && err.contains("max_positions_total"), "{err}");

        let mut cfg = GuardConfig::default();
        cfg.max_spread_pips.clear();
        cfg.max_spread_pips.insert(" EURUSD_z ".into(), 1.2);
        cfg.max_spread_pips.insert("DEFAULT".into(), 2.0);
        cfg.normalize();
        assert!(cfg.max_spread_pips.contains_key("EURUSD_z"));
        assert_eq!(cfg.max_spread_for("EURUSD_z"), 1.2);
        assert_eq!(cfg.max_spread_for("eurusd_Z"), 1.2);
        assert_eq!(cfg.max_spread_for("EURUSD"), 2.0);
        assert_eq!(cfg.max_spread_for("GBPUSD"), 2.0);
    }
}
