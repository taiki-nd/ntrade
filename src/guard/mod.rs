//! 事後ガード: LLM の出力を発注前に機械的に検証する。
//!
//! 入力前に相場観を絞る「事前判定」は置かない。ここはリスク管理であり、数値は設定ファイルで固定する。
//! 弾いた場合も、どのガードで弾いたかを必ず記録する（ガード自体の妥当性をリプレイで検証するため）。

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::snapshot::{AtrTimeframe, MarketSnapshot};
use crate::strategy::types::{Action, EntryType, ManageAction, ManageOp, TradeDecision};

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
    /// 旧: 観測整合チェック。LLM にチャート画像を渡さなくなったので使わない（保存済みの設定を読めるよう残す）
    pub observed_check: bool,
    pub min_confidence: f64,
    pub min_rr: f64,
    /// SL 幅の下限（5M ATR 比）。日々の値動きだけで刈られる狭さを弾く
    pub sl_atr_min: f64,
    /// SL 幅の上限（`sl_atr_max_tf` の ATR 比）
    pub sl_atr_max: f64,
    /// SL 幅の上限を測る ATR の時間足。LLM は上位足の構造で SL を置くので、5M で測ると
    /// 値幅の大きい銘柄（XAUUSD など）ではほぼ常に上限を超えてしまう
    pub sl_atr_max_tf: AtrTimeframe,
    pub sl_min_pips: f64,
    pub max_positions_per_pair: usize,
    pub max_positions_total: usize,
    pub daily_loss_limit_pct: f64,
    /// 予約注文の有効期限の上限（時間）。旧: 条件付きプランの期限の上限（保存済みの設定を読めるよう名前は据え置き）
    pub plan_max_hours: f64,
    pub fixed_volume_lots: f64,
    pub risk_pct: Option<f64>,
    pub pip_value_per_lot: Option<f64>,
    /// 銘柄ごとのスプレッド上限（pips）。`default` は FX にだけ適用する（pip の定義が銘柄で違うため）
    pub max_spread_pips: HashMap<String, f64>,
    /// スプレッド上限を 5M ATR に対する比で持つ（全銘柄に適用。None なら使わない）
    pub max_spread_atr: Option<f64>,
    /// 発注後の証拠金使用率（使用証拠金 / 有効証拠金, %）の上限。None なら見ない
    pub max_margin_usage_pct: Option<f64>,
    /// 発注後の実効レバレッジ（建玉の想定元本合計 / 有効証拠金）の上限。None なら見ない
    pub max_effective_leverage: Option<f64>,
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
            sl_atr_max_tf: AtrTimeframe::H1,
            sl_min_pips: 3.0,
            max_positions_per_pair: 1,
            max_positions_total: 1,
            daily_loss_limit_pct: 3.0,
            plan_max_hours: 8.0,
            fixed_volume_lots: 0.01,
            risk_pct: None,
            pip_value_per_lot: None,
            max_spread_pips: HashMap::from([("default".to_string(), 1.0)]),
            max_spread_atr: None,
            max_margin_usage_pct: None,
            max_effective_leverage: None,
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

    /// 銘柄のスプレッド上限（pips）。ペア名は大文字小文字を区別せずに照合する（キーは画面で入力した表記のまま保存するため）。
    /// `default` は FX にだけ当てる。XAUUSD や株価指数は pip の大きさが違い、FX の上限では常に弾かれるため
    pub fn max_spread_for(&self, pair: &str) -> Option<f64> {
        let explicit = self.max_spread_pips.iter().find(|(k, _)| k.eq_ignore_ascii_case(pair)).map(|(_, v)| *v);
        explicit.or_else(|| {
            crate::instrument::is_forex(pair).then(|| self.max_spread_pips.get("default").copied()).flatten()
        })
    }

    /// 発注ロット。risk_pct が設定されていれば SL 幅から算出する。
    ///
    /// 1 lot・1 pip の価値は `pip_value_per_lot`（設定での固定値）を優先し、無ければ
    /// ブローカーのレートから求めた `live_pip_value` を使う。どちらも無ければ固定ロット。
    /// リスク % から求めた値は切り上げない。ブローカーの最小発注量に満たなければ発注側で見送る
    /// （最小まで増やすと、許容したリスクを超えるため）。
    pub fn volume_lots(&self, balance: f64, sl_pips: f64, live_pip_value: Option<f64>) -> f64 {
        match (self.risk_pct, self.pip_value_per_lot.or(live_pip_value)) {
            (Some(r), Some(pv)) if sl_pips > 0.0 && pv > 0.0 => {
                let risk_amount = balance * r / 100.0;
                let lots = risk_amount / (sl_pips * pv);
                (lots * 100.0).floor() / 100.0
            }
            _ => self.fixed_volume_lots,
        }
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
        check(self.sl_atr_min >= 0.0, "sl_atr_min は 0 以上");
        check(self.sl_atr_max > 0.0, "sl_atr_max は 0 より大きい値");
        check(
            self.sl_atr_max_tf != AtrTimeframe::M5 || self.sl_atr_min <= self.sl_atr_max,
            "sl_atr_min は sl_atr_max 以下（どちらも 5M ATR 比のとき）",
        );
        check(self.sl_min_pips >= 0.0, "sl_min_pips は 0 以上");
        check(self.max_positions_per_pair >= 1, "max_positions_per_pair は 1 以上");
        check(self.max_positions_total >= 1, "max_positions_total は 1 以上");
        check(self.daily_loss_limit_pct > 0.0, "daily_loss_limit_pct は 0 より大きい値");
        check(self.plan_max_hours > 0.0, "plan_max_hours は 0 より大きい値");
        check(self.fixed_volume_lots >= 0.01, "fixed_volume_lots は 0.01 以上");
        check(self.risk_pct.is_none_or(|r| r > 0.0 && r <= 10.0), "risk_pct は 0〜10（%）");
        check(self.pip_value_per_lot.is_none_or(|v| v > 0.0), "pip_value_per_lot は 0 より大きい値");
        check(self.max_spread_pips.values().all(|v| *v >= 0.0), "max_spread_pips は 0 以上");
        check(self.max_spread_atr.is_none_or(|v| v > 0.0), "max_spread_atr は 0 より大きい値");
        check(self.max_margin_usage_pct.is_none_or(|v| v > 0.0 && v <= 100.0), "max_margin_usage_pct は 0〜100（%）");
        check(self.max_effective_leverage.is_none_or(|v| v > 0.0), "max_effective_leverage は 0 より大きい値");
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
    /// まだ約定していない予約注文。約定すれば建玉になるので、建玉数の上限に含めて数える
    pub working_orders_pair: usize,
    pub working_orders_total: usize,
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

/// 新規の予約注文を検証する（HOLD なら何も見ない）。既存の注文・建玉の操作は `manage_problem` で見る
pub fn evaluate(decision: &TradeDecision, snapshot: &MarketSnapshot, ctx: &GuardContext, cfg: &GuardConfig) -> GuardVerdict {
    let mut c = Checker { checks: Vec::new() };
    let now = ctx.now.or_else(|| snapshot_time(snapshot)).unwrap_or_else(Utc::now);

    if matches!(decision.action, Action::Buy | Action::Sell) {
        c.add(
            "CONFIDENCE_LOW",
            decision.confidence >= cfg.min_confidence,
            format!("confidence {:.2} < min {:.2}", decision.confidence, cfg.min_confidence),
        );
        match (decision.entry_price, decision.stop_loss, decision.take_profit) {
            (Some(entry), Some(sl), Some(tp)) => {
                check_levels(&mut c, decision.action, entry, sl, tp, snapshot, cfg);
                check_order(&mut c, decision, entry, snapshot, cfg);
            }
            _ => c.add("NO_SL_TP", false, "entry/stop_loss/take_profit must all be set for BUY/SELL"),
        }
        check_environment(&mut c, snapshot, ctx, cfg, now);
    }

    c.finish()
}

/// 予約注文としての妥当性: 種類・現在値に対する置き場所・有効期限
fn check_order(c: &mut Checker, d: &TradeDecision, entry: f64, snapshot: &MarketSnapshot, cfg: &GuardConfig) {
    let kind = d.entry_type.filter(|k| matches!(k, EntryType::Limit | EntryType::Stop));
    c.add("ENTRY_TYPE", kind.is_some(), format!("entry_type {:?} must be LIMIT or STOP", d.entry_type));
    if let Some(kind) = kind {
        let current = snapshot.current_price;
        if current > 0.0 {
            let ok = order_price_on_correct_side(d.action, kind, entry, current);
            c.add("ORDER_PRICE_SIDE", ok, format!("{:?} {:?} at {entry} vs current {current}", d.action, kind));
        }
    }
    let max = (cfg.plan_max_hours * 60.0).round() as u32;
    match d.expiry_minutes {
        Some(m) => c.add("ORDER_EXPIRY", m >= 1 && m <= max, format!("expiry {m} min must be within 1..={max}")),
        None => c.add("ORDER_EXPIRY", false, "expiry_minutes must be set (pending orders always expire)"),
    }
}

/// 指値は現在値より有利な側（BUY は下・SELL は上）、逆指値は不利な側（BUY は上・SELL は下）に置く。
/// 逆側に置くと即座に約定するか、ブローカーに拒否される
pub fn order_price_on_correct_side(action: Action, kind: EntryType, entry: f64, current: f64) -> bool {
    match (action, kind) {
        (Action::Buy, EntryType::Limit) | (Action::Sell, EntryType::Stop) => entry < current,
        (Action::Sell, EntryType::Limit) | (Action::Buy, EntryType::Stop) => entry > current,
        _ => false,
    }
}

/// 既存の注文・建玉への操作が実行できない形なら理由を返す。
///
/// 対象の向き（BUY/SELL）と現在値に対して、SL/TP が逆側にあるとブローカーに拒否されるか即決済になるので、
/// それだけを見る。操作そのものの是非（建値に動かすか・撤退するか）は LLM の裁量に任せる。
pub fn manage_problem(m: &ManageAction, side: Action, entry: Option<f64>, current: f64) -> Option<String> {
    let sign = match side {
        Action::Buy => 1.0,
        Action::Sell => -1.0,
        Action::Hold => return Some("target side unknown".into()),
    };
    if m.op != ManageOp::Modify {
        return None;
    }
    if m.entry_price.is_none() && m.stop_loss.is_none() && m.take_profit.is_none() && m.expiry_minutes.is_none() {
        return Some("MODIFY without any new value".into());
    }
    // 予約注文は新しい価格（無ければ今の価格）、建玉は現在値を基準にする
    let base = m.entry_price.or(entry).unwrap_or(current);
    if let Some(sl) = m.stop_loss {
        if (base - sl) * sign <= 0.0 {
            return Some(format!("stop_loss {sl} is on the wrong side of {base}"));
        }
    }
    if let Some(tp) = m.take_profit {
        if (tp - base) * sign <= 0.0 {
            return Some(format!("take_profit {tp} is on the wrong side of {base}"));
        }
    }
    None
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
    // pips での下限は FX にだけ当てる。他の銘柄は pip の大きさがブローカーごとに違うので ATR 比で見る
    if crate::instrument::is_forex(&snapshot.pair) {
        c.add("SL_TOO_TIGHT", sl_pips >= cfg.sl_min_pips, format!("sl {:.1} pips < min {:.1}", sl_pips, cfg.sl_min_pips));
    }

    // 下限は 5M ATR、上限は `sl_atr_max_tf` の ATR で測る。取れない側は判定を飛ばす
    let vol = &snapshot.volatility;
    let atr_min = vol.atr14_5m_pips.filter(|a| *a > 0.0);
    let atr_max = vol.atr_pips(cfg.sl_atr_max_tf).filter(|a| *a > 0.0);
    if atr_min.is_none() && atr_max.is_none() {
        c.add("SL_ATR_RANGE", true, "ATR unavailable; skipped");
        return;
    }
    let mut out: Vec<String> = Vec::new();
    if let Some(atr) = atr_min {
        let ratio = sl_pips / atr;
        if ratio < cfg.sl_atr_min {
            out.push(format!("sl {sl_pips:.1} pips = {ratio:.2} x ATR5M({atr:.1}) < min {:.1}", cfg.sl_atr_min));
        }
    }
    if let Some(atr) = atr_max {
        let ratio = sl_pips / atr;
        if ratio > cfg.sl_atr_max {
            let tf = cfg.sl_atr_max_tf.label();
            out.push(format!("sl {sl_pips:.1} pips = {ratio:.2} x ATR{tf}({atr:.1}) > max {:.1}", cfg.sl_atr_max));
        }
    }
    c.add("SL_ATR_RANGE", out.is_empty(), out.join("; "));
}

fn check_environment(c: &mut Checker, snapshot: &MarketSnapshot, ctx: &GuardContext, cfg: &GuardConfig, now: DateTime<Utc>) {
    check_spread(c, snapshot, cfg);
    let (pair, total) = (ctx.open_positions_pair + ctx.working_orders_pair, ctx.open_positions_total + ctx.working_orders_total);
    c.add(
        "MAX_POSITIONS",
        pair < cfg.max_positions_per_pair && total < cfg.max_positions_total,
        format!(
            "positions+orders pair={pair} total={total} (orders {}/{}; limits {}/{})",
            ctx.working_orders_pair, ctx.working_orders_total, cfg.max_positions_per_pair, cfg.max_positions_total
        ),
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

/// スプレッドの確認。気配値が取れていない（購読前・休場中）ときは判定できないので通し、その旨を残す
fn check_spread(c: &mut Checker, snapshot: &MarketSnapshot, cfg: &GuardConfig) {
    let Some(spread) = snapshot.spread_pips else {
        c.add("SPREAD_TOO_WIDE", true, "spread unknown (no live quote); skipped");
        return;
    };
    let mut over: Vec<String> = Vec::new();
    if let Some(max) = cfg.max_spread_for(&snapshot.pair) {
        if spread > max {
            over.push(format!("spread {spread:.1} > max {max:.1} pips"));
        }
    }
    if let (Some(ratio), Some(atr)) = (cfg.max_spread_atr, snapshot.volatility.atr14_5m_pips.filter(|a| *a > 0.0)) {
        if spread > ratio * atr {
            over.push(format!("spread {spread:.1} > {ratio:.2} x ATR5M({atr:.1}) pips"));
        }
    }
    c.add("SPREAD_TOO_WIDE", over.is_empty(), over.join("; "));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{AccountState, SnapshotConfig, SnapshotInput};
    use crate::strategy::types::{Analysis, ManageTarget};
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
                spread_pips: Some(0.3),
                current_price: None,
                now: Some(end + Duration::minutes(5)),
                account_state: AccountState::default(),
            },
            &SnapshotConfig::default(),
        )
    }

    /// 現在値（約 154.01）より下の BUY 指値
    fn decision(_snap: &MarketSnapshot) -> TradeDecision {
        TradeDecision {
            action: Action::Buy,
            confidence: 0.8,
            entry_type: Some(EntryType::Limit),
            entry_price: Some(153.90),
            stop_loss: Some(153.82),  // 8 pips
            take_profit: Some(154.06), // RR 2.0
            expiry_minutes: Some(60),
            analysis: Analysis { market_structure: "m".into(), price_levels: "p".into(), invalidation: "c".into(), conflicts: "d".into(), fundamental_view: String::new() },
            manage: Vec::new(),
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
        d.take_profit = Some(153.95); // RR 0.6
        assert!(evaluate(&d, &snap, &GuardContext::default(), &cfg).failed.contains(&"RR_TOO_LOW".to_string()));

        let mut d = decision(&snap);
        d.stop_loss = Some(153.89); // 1 pip
        d.take_profit = Some(153.93);
        let f = evaluate(&d, &snap, &GuardContext::default(), &cfg).failed;
        assert!(f.contains(&"SL_TOO_TIGHT".to_string()) && f.contains(&"SL_ATR_RANGE".to_string()));

        let mut d = decision(&snap);
        d.stop_loss = Some(154.00);
        assert!(evaluate(&d, &snap, &GuardContext::default(), &cfg).failed.contains(&"SL_WRONG_SIDE".to_string()));

        let ctx = GuardContext { open_positions_total: 1, ..Default::default() };
        assert!(evaluate(&decision(&snap), &snap, &ctx, &cfg).failed.contains(&"MAX_POSITIONS".to_string()));
        // 約定待ちの予約注文も建玉数に含める
        let ctx = GuardContext { working_orders_pair: 1, working_orders_total: 1, ..Default::default() };
        assert!(evaluate(&decision(&snap), &snap, &ctx, &cfg).failed.contains(&"MAX_POSITIONS".to_string()));

        let ctx = GuardContext { daily_pnl_pct: -3.5, ..Default::default() };
        assert!(evaluate(&decision(&snap), &snap, &ctx, &cfg).failed.contains(&"DAILY_LOSS_LIMIT".to_string()));

        let mut wide = snap.clone();
        wide.spread_pips = Some(2.0);
        assert!(evaluate(&decision(&snap), &wide, &GuardContext::default(), &cfg).failed.contains(&"SPREAD_TOO_WIDE".to_string()));

        let mut cfg2 = cfg.clone();
        cfg2.news_blackout.push(NewsBlackout { time: "2026-09-15 09:20:00".into(), before_min: 30, after_min: 15, label: "CPI".into() });
        assert!(evaluate(&decision(&snap), &snap, &GuardContext::default(), &cfg2).failed.contains(&"NEWS_BLACKOUT".to_string()));
    }

    #[test]
    fn sl_atr_upper_bound_uses_configured_timeframe() {
        let mut snap = snapshot();
        snap.volatility.atr14_5m_pips = Some(5.0);
        snap.volatility.atr14_1h_pips = Some(15.0);
        let mut d = decision(&snap);
        d.stop_loss = Some(153.60); // 30 pips = 6 x ATR5M, 2 x ATR1H
        d.take_profit = Some(154.50);
        let fails = |d: &TradeDecision, snap: &MarketSnapshot, cfg: &GuardConfig| {
            evaluate(d, snap, &GuardContext::default(), cfg).failed.contains(&"SL_ATR_RANGE".to_string())
        };
        assert!(!fails(&d, &snap, &GuardConfig::default()));
        assert!(fails(&d, &snap, &GuardConfig { sl_atr_max_tf: AtrTimeframe::M5, ..GuardConfig::default() }));
        // 上限の時間足の ATR が無くても、下限（5M）は見る
        snap.volatility.atr14_1h_pips = None;
        d.stop_loss = Some(153.89);
        d.take_profit = Some(153.93);
        assert!(fails(&d, &snap, &GuardConfig::default()));
        // 項目追加前に保存された設定は 1H で測る
        let old: GuardConfig = serde_json::from_str(r#"{"sl_atr_max":4.0}"#).unwrap();
        assert_eq!(old.sl_atr_max_tf, AtrTimeframe::H1);
    }

    #[test]
    fn spread_is_checked_against_atr_and_skipped_when_unknown() {
        let snap = snapshot();
        let cfg = GuardConfig { max_spread_pips: HashMap::new(), max_spread_atr: Some(0.5), ..GuardConfig::default() };
        let atr = snap.volatility.atr14_5m_pips.unwrap();
        let mut wide = snap.clone();
        wide.spread_pips = Some(atr * 0.6);
        assert!(evaluate(&decision(&snap), &wide, &GuardContext::default(), &cfg).failed.contains(&"SPREAD_TOO_WIDE".to_string()));
        let mut unknown = snap.clone();
        unknown.spread_pips = None;
        assert!(evaluate(&decision(&snap), &unknown, &GuardContext::default(), &cfg).passed);
    }

    #[test]
    fn risk_sizing_is_not_rounded_up_to_the_minimum() {
        let cfg = GuardConfig { risk_pct: Some(0.1), pip_value_per_lot: Some(1500.0), ..GuardConfig::default() };
        // 10 万円 × 0.1% = 100 円 / (20 pips × 1,500 円) = 0.0033 lot → 0.00（発注側で見送る）
        assert_eq!(cfg.volume_lots(100_000.0, 20.0, None), 0.0);
    }

    #[test]
    fn pending_order_type_side_and_expiry() {
        let snap = snapshot();
        let cfg = GuardConfig::default();
        let fails = |d: &TradeDecision, name: &str| evaluate(d, &snap, &GuardContext::default(), &cfg).failed.contains(&name.to_string());

        // 成行（旧）は出させない
        let mut d = decision(&snap);
        d.entry_type = Some(EntryType::Market);
        assert!(fails(&d, "ENTRY_TYPE"));

        // BUY 指値を現在値より上に置くと即約定してしまう
        let mut d = decision(&snap);
        d.entry_price = Some(154.10);
        d.stop_loss = Some(154.02);
        d.take_profit = Some(154.26);
        assert!(fails(&d, "ORDER_PRICE_SIDE"));
        // 同じ価格でも逆指値（ブレイク買い）なら正しい
        d.entry_type = Some(EntryType::Stop);
        assert!(!fails(&d, "ORDER_PRICE_SIDE"), "{:?}", evaluate(&d, &snap, &GuardContext::default(), &cfg).failed);

        let mut d = decision(&snap);
        d.expiry_minutes = None;
        assert!(fails(&d, "ORDER_EXPIRY"));
        d.expiry_minutes = Some(9 * 60);
        assert!(fails(&d, "ORDER_EXPIRY"));

        // HOLD は何も見ない
        let mut d = decision(&snap);
        d.action = Action::Hold;
        d.confidence = 0.2;
        assert!(evaluate(&d, &snap, &GuardContext::default(), &cfg).passed);
    }

    #[test]
    fn manage_checks_only_executability() {
        let m = |op, sl: Option<f64>, tp: Option<f64>| ManageAction {
            target: ManageTarget::Position,
            id: "1".into(),
            op,
            entry_price: None,
            stop_loss: sl,
            take_profit: tp,
            expiry_minutes: None,
            reason: String::new(),
        };
        // BUY 建玉の SL を建値 154.00 へ（現在値 154.20）
        assert_eq!(manage_problem(&m(ManageOp::Modify, Some(154.00), None), Action::Buy, None, 154.20), None);
        // 現在値より上の SL は即決済になる
        assert!(manage_problem(&m(ManageOp::Modify, Some(154.30), None), Action::Buy, None, 154.20).is_some());
        assert!(manage_problem(&m(ManageOp::Modify, None, Some(154.10)), Action::Buy, None, 154.20).is_some());
        assert!(manage_problem(&m(ManageOp::Modify, None, None), Action::Buy, None, 154.20).is_some());
        assert_eq!(manage_problem(&m(ManageOp::Close, None, None), Action::Sell, None, 154.20), None);
        // 予約注文は注文の価格を基準にする（SELL 指値 154.50 の SL 154.60）
        assert_eq!(manage_problem(&m(ManageOp::Modify, Some(154.60), None), Action::Sell, Some(154.50), 154.20), None);
    }

    #[test]
    fn config_loads_from_repo_file_and_sizes_lots() {
        let cfg = GuardConfig::load(concat!(env!("CARGO_MANIFEST_DIR"), "/config/guard.toml")).unwrap();
        assert_eq!(cfg.max_spread_for("eurusd"), Some(1.2));
        assert_eq!(cfg.max_spread_for("GBPUSD"), Some(1.0));
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
        assert_eq!(cfg.max_spread_for("EURUSD_z"), Some(1.2));
        assert_eq!(cfg.max_spread_for("eurusd_Z"), Some(1.2));
        assert_eq!(cfg.max_spread_for("EURUSD"), Some(2.0));
        assert_eq!(cfg.max_spread_for("GBPUSD"), Some(2.0));
        // FX 以外には default を当てない（明示したときだけ pips で見る）
        assert_eq!(cfg.max_spread_for("US500"), None);
        cfg.max_spread_pips.insert("US500".into(), 80.0);
        assert_eq!(cfg.max_spread_for("us500"), Some(80.0));
    }
}
