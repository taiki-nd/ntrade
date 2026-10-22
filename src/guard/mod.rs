//! リスクチェック: 戦略が出した新規注文を、発注前に機械的に検証する。
//!
//! バックテストと本番で同じ `evaluate` を通すので、ここで弾いた注文はどちらでも発注されない。
//! 弾いた場合はどのチェックで弾いたかを必ず記録する（バックテストの集計・シグナルログに残る）。

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::ctrader::{BarPeriod, CandleBar, PendingKind};
use crate::strategy::{OrderIntent, Side};

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

/// SL 幅を測る ATR の時間足
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum AtrTimeframe {
    #[serde(rename = "5m")]
    M5,
    #[serde(rename = "15m")]
    M15,
    #[default]
    #[serde(rename = "1h")]
    H1,
    #[serde(rename = "4h")]
    H4,
}

impl AtrTimeframe {
    pub fn label(self) -> &'static str {
        self.period().as_str()
    }

    pub fn period(self) -> BarPeriod {
        match self {
            Self::M5 => BarPeriod::M5,
            Self::M15 => BarPeriod::M15,
            Self::H1 => BarPeriod::H1,
            Self::H4 => BarPeriod::H4,
        }
    }
}

/// ATR(14)（pips）を時間足ごとに持つ。取れない時間足は None（そのチェックは飛ばす）
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AtrSet(pub HashMap<AtrTimeframe, f64>);

impl AtrSet {
    pub fn get(&self, tf: AtrTimeframe) -> Option<f64> {
        self.0.get(&tf).copied().filter(|v| *v > 0.0)
    }

    /// 確定足から ATR(14) を求めて pips で入れる
    pub fn insert_from_bars(&mut self, tf: AtrTimeframe, bars: &[CandleBar], pip_size: f64) {
        if let Some(atr) = crate::indicators::calculate_atr(bars, 14) {
            self.0.insert(tf, atr / pip_size.max(1e-12));
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GuardConfig {
    /// リスクリワードの下限（TP を置く注文だけに当てる）
    pub min_rr: f64,
    /// SL 幅の下限（5M ATR 比）。日々の値動きだけで刈られる狭さを弾く
    pub sl_atr_min: f64,
    /// SL 幅の上限（`sl_atr_max_tf` の ATR 比）
    pub sl_atr_max: f64,
    pub sl_atr_max_tf: AtrTimeframe,
    pub sl_min_pips: f64,
    pub max_positions_per_pair: usize,
    pub max_positions_total: usize,
    pub daily_loss_limit_pct: f64,
    /// 予約注文の有効期限の上限（時間）
    #[serde(alias = "plan_max_hours")]
    pub order_max_hours: f64,
    pub fixed_volume_lots: f64,
    pub risk_pct: Option<f64>,
    pub pip_value_per_lot: Option<f64>,
    /// 銘柄ごとのスプレッド上限（pips）。`default` は FX にだけ適用する（pip の定義が銘柄で違うため）
    pub max_spread_pips: HashMap<String, f64>,
    /// スプレッド上限を 5M ATR に対する比で持つ（全銘柄に適用。None なら使わない）
    pub max_spread_atr: Option<f64>,
    /// 発注後の証拠金使用率（使用証拠金 / 有効証拠金, %）の上限。None なら見ない（本番のみ）
    pub max_margin_usage_pct: Option<f64>,
    /// 発注後の実効レバレッジ（建玉の想定元本合計 / 有効証拠金）の上限。None なら見ない（本番のみ）
    pub max_effective_leverage: Option<f64>,
    pub news_blackout: Vec<NewsBlackout>,
}

impl Default for GuardConfig {
    fn default() -> Self {
        Self {
            min_rr: 1.5,
            sl_atr_min: 0.5,
            sl_atr_max: 4.0,
            sl_atr_max_tf: AtrTimeframe::H1,
            sl_min_pips: 3.0,
            max_positions_per_pair: 1,
            max_positions_total: 1,
            daily_loss_limit_pct: 3.0,
            order_max_hours: 8.0,
            fixed_volume_lots: 0.01,
            risk_pct: None,
            pip_value_per_lot: None,
            max_spread_pips: HashMap::from([("default".to_string(), 1.0)]),
            max_spread_atr: None,
            max_margin_usage_pct: None,
            max_effective_leverage: None,
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

    /// 銘柄のスプレッド上限（pips）。ペア名は大文字小文字を区別せずに照合する（キーは画面で入力した表記のまま保存するため）。
    /// `default` は FX にだけ当てる。XAUUSD や株価指数は pip の大きさが違い、FX の上限では常に弾かれるため
    pub fn max_spread_for(&self, pair: &str) -> Option<f64> {
        let explicit = self.max_spread_pips.iter().find(|(k, _)| k.eq_ignore_ascii_case(pair)).map(|(_, v)| *v);
        explicit.or_else(|| {
            crate::instrument::is_forex(pair).then(|| self.max_spread_pips.get("default").copied()).flatten()
        })
    }

    /// チェックに使う ATR の時間足（呼び出し側はこの時間足の確定足を用意する）
    pub fn atr_timeframes(&self) -> Vec<AtrTimeframe> {
        let mut tfs = Vec::new();
        if self.sl_atr_min > 0.0 || self.max_spread_atr.is_some() {
            tfs.push(AtrTimeframe::M5);
        }
        if self.sl_atr_max > 0.0 && !tfs.contains(&self.sl_atr_max_tf) {
            tfs.push(self.sl_atr_max_tf);
        }
        tfs
    }

    /// 発注ロット。risk_pct が設定されていれば SL 幅から算出する。
    ///
    /// 1 lot・1 pip の価値は `pip_value_per_lot`（設定での固定値）を優先し、無ければ
    /// レートから求めた `live_pip_value` を使う。どちらも無ければ固定ロット。
    /// リスク % から求めた値は切り上げない。最小発注量に満たなければ発注側で見送る
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
        check(self.min_rr >= 0.0, "min_rr は 0 以上");
        check(self.sl_atr_min >= 0.0, "sl_atr_min は 0 以上");
        check(self.sl_atr_max >= 0.0, "sl_atr_max は 0 以上（0 で上限なし）");
        check(
            self.sl_atr_max_tf != AtrTimeframe::M5 || self.sl_atr_max == 0.0 || self.sl_atr_min <= self.sl_atr_max,
            "sl_atr_min は sl_atr_max 以下（どちらも 5M ATR 比のとき）",
        );
        check(self.sl_min_pips >= 0.0, "sl_min_pips は 0 以上");
        check(self.max_positions_per_pair >= 1, "max_positions_per_pair は 1 以上");
        check(self.max_positions_total >= 1, "max_positions_total は 1 以上");
        check(self.daily_loss_limit_pct > 0.0, "daily_loss_limit_pct は 0 より大きい値");
        check(self.order_max_hours > 0.0, "order_max_hours は 0 より大きい値");
        check(self.fixed_volume_lots >= 0.01, "fixed_volume_lots は 0.01 以上");
        check(self.risk_pct.is_none_or(|r| r > 0.0 && r <= 10.0), "risk_pct は 0〜10（%）");
        check(self.pip_value_per_lot.is_none_or(|v| v > 0.0), "pip_value_per_lot は 0 より大きい値");
        check(self.max_spread_pips.values().all(|v| *v >= 0.0), "max_spread_pips は 0 以上");
        check(self.max_spread_atr.is_none_or(|v| v > 0.0), "max_spread_atr は 0 より大きい値");
        check(self.max_margin_usage_pct.is_none_or(|v| v > 0.0 && v <= 100.0), "max_margin_usage_pct は 0〜100（%）");
        check(self.max_effective_leverage.is_none_or(|v| v > 0.0), "max_effective_leverage は 0 より大きい値");
        check(self.news_blackout.iter().all(|b| parse_utc(&b.time).is_some()), "news_blackout.time は YYYY-MM-DD HH:MM:SS（UTC）");
        check(self.news_blackout.iter().all(|b| b.before_min >= 0 && b.after_min >= 0), "news_blackout の前後分数は 0 以上");
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

    /// 予約注文の期限。指定が無いか上限を超える指定は上限に丸める
    pub fn order_expiry(&self, now: DateTime<Utc>, minutes: Option<u32>) -> DateTime<Utc> {
        let max = ((self.order_max_hours * 60.0).round() as i64).max(1);
        now + Duration::minutes(minutes.map(|m| (m as i64).clamp(1, max)).unwrap_or(max))
    }
}

/// 注文を判断した時点の相場
#[derive(Debug, Clone)]
pub struct MarketView<'a> {
    pub pair: &'a str,
    pub now: DateTime<Utc>,
    /// 判断に使う現在値（気配の仲値。無ければ直近の終値）。成行の建値の見込みと、予約注文の置き場所の確認に使う
    pub price: f64,
    /// 現在のスプレッド（pips）。分からなければ None（スプレッドのチェックは飛ばす）
    pub spread_pips: Option<f64>,
    pub pip_size: f64,
    pub atr_pips: AtrSet,
}

/// ガード評価に必要な口座・環境の状態
#[derive(Debug, Clone, Default)]
pub struct GuardContext {
    pub open_positions_pair: usize,
    pub open_positions_total: usize,
    /// まだ約定していない予約注文。約定すれば建玉になるので、建玉数の上限に含めて数える
    pub working_orders_pair: usize,
    pub working_orders_total: usize,
    /// 当日（UTC）の確定損益（資金比 %、損失は負）
    pub daily_pnl_pct: f64,
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

    /// 失敗したチェックの詳細（ログ用）
    pub fn failure_details(&self) -> String {
        self.checks.iter().filter(|c| !c.passed).map(|c| format!("{}: {}", c.name, c.detail)).collect::<Vec<_>>().join(" / ")
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

/// 注文の建値の見込み（成行は現在値、予約注文は注文価格）
pub fn expected_entry(intent: &OrderIntent, market: &MarketView) -> f64 {
    intent.entry.pending().map(|(_, p)| p).unwrap_or(market.price)
}

/// 新規注文を検証する
pub fn evaluate(intent: &OrderIntent, market: &MarketView, ctx: &GuardContext, cfg: &GuardConfig) -> GuardVerdict {
    let mut c = Checker { checks: Vec::new() };
    let entry = expected_entry(intent, market);
    check_levels(&mut c, intent, entry, market, cfg);
    check_order(&mut c, intent, market, cfg);
    check_environment(&mut c, market, ctx, cfg);
    c.finish()
}

/// 予約注文としての妥当性: 現在値に対する置き場所・有効期限
fn check_order(c: &mut Checker, intent: &OrderIntent, market: &MarketView, cfg: &GuardConfig) {
    let Some((kind, price)) = intent.entry.pending() else { return };
    if market.price > 0.0 {
        let ok = order_price_on_correct_side(intent.side, kind, price, market.price);
        c.add("ORDER_PRICE_SIDE", ok, format!("{} {:?} at {price} vs current {}", intent.side.as_str(), kind, market.price));
    }
    let max = (cfg.order_max_hours * 60.0).round() as u32;
    if let Some(m) = intent.expiry_minutes {
        c.add("ORDER_EXPIRY", m >= 1 && m <= max, format!("expiry {m} min must be within 1..={max}"));
    }
}

/// 指値は現在値より有利な側（BUY は下・SELL は上）、逆指値は不利な側（BUY は上・SELL は下）に置く。
/// 逆側に置くと即座に約定するか、ブローカーに拒否される
pub fn order_price_on_correct_side(side: Side, kind: PendingKind, entry: f64, current: f64) -> bool {
    match (side, kind) {
        (Side::Buy, PendingKind::Limit) | (Side::Sell, PendingKind::Stop) => entry < current,
        (Side::Sell, PendingKind::Limit) | (Side::Buy, PendingKind::Stop) => entry > current,
    }
}

fn check_levels(c: &mut Checker, intent: &OrderIntent, entry: f64, market: &MarketView, cfg: &GuardConfig) {
    let pip = market.pip_size.max(1e-12);
    let sign = intent.side.sign();
    let sl_dist = (entry - intent.stop_loss) * sign;
    c.add("SL_WRONG_SIDE", sl_dist > 0.0, format!("entry {entry} sl {} {}", intent.stop_loss, intent.side.as_str()));
    let tp_dist = intent.take_profit.map(|tp| (tp - entry) * sign);
    if let Some(d) = tp_dist {
        c.add("TP_WRONG_SIDE", d > 0.0, format!("entry {entry} tp {:?} {}", intent.take_profit, intent.side.as_str()));
    }
    if sl_dist <= 0.0 || tp_dist.is_some_and(|d| d <= 0.0) {
        return;
    }

    if let Some(d) = tp_dist {
        let rr = d / sl_dist;
        c.add("RR_TOO_LOW", rr + 1e-9 >= cfg.min_rr, format!("rr {:.2} < min {:.2}", rr, cfg.min_rr));
    }
    let sl_pips = sl_dist / pip;
    // pips での下限は FX にだけ当てる。他の銘柄は pip の大きさがブローカーごとに違うので ATR 比で見る
    if crate::instrument::is_forex(market.pair) {
        c.add("SL_TOO_TIGHT", sl_pips + 1e-9 >= cfg.sl_min_pips, format!("sl {:.1} pips < min {:.1}", sl_pips, cfg.sl_min_pips));
    }

    // 下限は 5M ATR、上限は `sl_atr_max_tf` の ATR で測る。取れない側は判定を飛ばす
    let atr_min = market.atr_pips.get(AtrTimeframe::M5).filter(|_| cfg.sl_atr_min > 0.0);
    let atr_max = market.atr_pips.get(cfg.sl_atr_max_tf).filter(|_| cfg.sl_atr_max > 0.0);
    if atr_min.is_none() && atr_max.is_none() {
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

fn check_environment(c: &mut Checker, market: &MarketView, ctx: &GuardContext, cfg: &GuardConfig) {
    check_spread(c, market, cfg);
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
    let now = market.now;
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

/// スプレッドの確認。気配値が取れていない（購読前・休場中）ときは判定できないので通す
fn check_spread(c: &mut Checker, market: &MarketView, cfg: &GuardConfig) {
    let Some(spread) = market.spread_pips else { return };
    let mut over: Vec<String> = Vec::new();
    if let Some(max) = cfg.max_spread_for(market.pair) {
        if spread > max {
            over.push(format!("spread {spread:.1} > max {max:.1} pips"));
        }
    }
    if let (Some(ratio), Some(atr)) = (cfg.max_spread_atr, market.atr_pips.get(AtrTimeframe::M5)) {
        if spread > ratio * atr {
            over.push(format!("spread {spread:.1} > {ratio:.2} x ATR5M({atr:.1}) pips"));
        }
    }
    c.add("SPREAD_TOO_WIDE", over.is_empty(), over.join("; "));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategy::Entry;

    fn market() -> MarketView<'static> {
        MarketView {
            pair: "USDJPY",
            now: Utc.with_ymd_and_hms(2026, 9, 15, 9, 5, 0).unwrap(),
            price: 154.01,
            spread_pips: Some(0.3),
            pip_size: 0.01,
            atr_pips: AtrSet(HashMap::from([(AtrTimeframe::M5, 8.0), (AtrTimeframe::H1, 20.0)])),
        }
    }

    /// 現在値（154.01）より下の BUY 指値。SL 8 pips / RR 2.0
    fn intent() -> OrderIntent {
        OrderIntent {
            side: Side::Buy,
            entry: Entry::Limit { price: 153.90 },
            stop_loss: 153.82,
            take_profit: Some(154.06),
            expiry_minutes: Some(60),
            reason: "t".into(),
        }
    }

    fn fails(i: &OrderIntent, m: &MarketView, ctx: &GuardContext, cfg: &GuardConfig, name: &str) -> bool {
        evaluate(i, m, ctx, cfg).failed.contains(&name.to_string())
    }

    #[test]
    fn good_trade_passes() {
        let v = evaluate(&intent(), &market(), &GuardContext::default(), &GuardConfig::default());
        assert!(v.passed, "{:?}", v.failed);
        assert_eq!(v.summary(), "PASS");
    }

    #[test]
    fn each_guard_fires() {
        let (m, cfg, none) = (market(), GuardConfig::default(), GuardContext::default());

        let mut i = intent();
        i.take_profit = Some(153.95); // RR 0.6
        assert!(fails(&i, &m, &none, &cfg, "RR_TOO_LOW"));

        let mut i = intent();
        i.stop_loss = 153.89; // 1 pip
        i.take_profit = Some(153.93);
        let f = evaluate(&i, &m, &none, &cfg).failed;
        assert!(f.contains(&"SL_TOO_TIGHT".to_string()) && f.contains(&"SL_ATR_RANGE".to_string()), "{f:?}");

        let mut i = intent();
        i.stop_loss = 154.00;
        assert!(fails(&i, &m, &none, &cfg, "SL_WRONG_SIDE"));

        let ctx = GuardContext { open_positions_total: 1, ..Default::default() };
        assert!(fails(&intent(), &m, &ctx, &cfg, "MAX_POSITIONS"));
        // 約定待ちの予約注文も建玉数に含める
        let ctx = GuardContext { working_orders_pair: 1, working_orders_total: 1, ..Default::default() };
        assert!(fails(&intent(), &m, &ctx, &cfg, "MAX_POSITIONS"));

        let ctx = GuardContext { daily_pnl_pct: -3.5, ..Default::default() };
        assert!(fails(&intent(), &m, &ctx, &cfg, "DAILY_LOSS_LIMIT"));

        let wide = MarketView { spread_pips: Some(2.0), ..market() };
        assert!(fails(&intent(), &wide, &none, &cfg, "SPREAD_TOO_WIDE"));

        let mut cfg2 = cfg.clone();
        cfg2.news_blackout.push(NewsBlackout { time: "2026-09-15 09:20:00".into(), before_min: 30, after_min: 15, label: "CPI".into() });
        assert!(fails(&intent(), &m, &none, &cfg2, "NEWS_BLACKOUT"));
    }

    #[test]
    fn market_orders_use_the_current_price_and_tp_is_optional() {
        let i = OrderIntent { entry: Entry::Market, stop_loss: 153.85, take_profit: None, expiry_minutes: None, ..intent() };
        let v = evaluate(&i, &market(), &GuardContext::default(), &GuardConfig::default());
        assert!(v.passed, "{:?}", v.failed);
        assert!(!v.checks.iter().any(|c| c.name == "RR_TOO_LOW"));
        // 成行の SL が現在値より上なら逆側
        let bad = OrderIntent { stop_loss: 154.05, ..i };
        assert!(fails(&bad, &market(), &GuardContext::default(), &GuardConfig::default(), "SL_WRONG_SIDE"));
    }

    #[test]
    fn sl_atr_upper_bound_uses_configured_timeframe() {
        let mut m = market();
        m.atr_pips = AtrSet(HashMap::from([(AtrTimeframe::M5, 5.0), (AtrTimeframe::H1, 15.0)]));
        let mut i = intent();
        i.stop_loss = 153.60; // 30 pips = 6 x ATR5M, 2 x ATR1H
        i.take_profit = Some(154.50);
        let none = GuardContext::default();
        assert!(!fails(&i, &m, &none, &GuardConfig::default(), "SL_ATR_RANGE"));
        assert!(fails(&i, &m, &none, &GuardConfig { sl_atr_max_tf: AtrTimeframe::M5, ..GuardConfig::default() }, "SL_ATR_RANGE"));
        // 上限の時間足の ATR が無くても、下限（5M）は見る
        m.atr_pips.0.remove(&AtrTimeframe::H1);
        i.stop_loss = 153.89;
        i.take_profit = Some(153.93);
        assert!(fails(&i, &m, &none, &GuardConfig::default(), "SL_ATR_RANGE"));
        // 0 にすれば見ない
        let off = GuardConfig { sl_atr_min: 0.0, sl_atr_max: 0.0, ..GuardConfig::default() };
        assert!(!fails(&i, &m, &none, &off, "SL_ATR_RANGE"));
        assert!(off.atr_timeframes().is_empty());
    }

    #[test]
    fn spread_is_checked_against_atr_and_skipped_when_unknown() {
        let cfg = GuardConfig { max_spread_pips: HashMap::new(), max_spread_atr: Some(0.5), ..GuardConfig::default() };
        let wide = MarketView { spread_pips: Some(8.0 * 0.6), ..market() };
        assert!(fails(&intent(), &wide, &GuardContext::default(), &cfg, "SPREAD_TOO_WIDE"));
        let unknown = MarketView { spread_pips: None, ..market() };
        assert!(evaluate(&intent(), &unknown, &GuardContext::default(), &cfg).passed);
    }

    #[test]
    fn risk_sizing_is_not_rounded_up_to_the_minimum() {
        let cfg = GuardConfig { risk_pct: Some(0.1), pip_value_per_lot: Some(1500.0), ..GuardConfig::default() };
        // 10 万円 × 0.1% = 100 円 / (20 pips × 1,500 円) = 0.0033 lot → 0.00（発注側で見送る）
        assert_eq!(cfg.volume_lots(100_000.0, 20.0, None), 0.0);
    }

    #[test]
    fn pending_order_side_and_expiry() {
        let (m, cfg, none) = (market(), GuardConfig::default(), GuardContext::default());
        // BUY 指値を現在値より上に置くと即約定してしまう
        let mut i = intent();
        i.entry = Entry::Limit { price: 154.10 };
        i.stop_loss = 154.02;
        i.take_profit = Some(154.26);
        assert!(fails(&i, &m, &none, &cfg, "ORDER_PRICE_SIDE"));
        // 同じ価格でも逆指値（ブレイク買い）なら正しい
        i.entry = Entry::Stop { price: 154.10 };
        assert!(!fails(&i, &m, &none, &cfg, "ORDER_PRICE_SIDE"));

        let mut i = intent();
        i.expiry_minutes = Some(9 * 60);
        assert!(fails(&i, &m, &none, &cfg, "ORDER_EXPIRY"));
        // 期限の指定が無ければ上限まで
        i.expiry_minutes = None;
        assert!(!fails(&i, &m, &none, &cfg, "ORDER_EXPIRY"));
        let now = m.now;
        assert_eq!(cfg.order_expiry(now, None), now + Duration::hours(8));
        assert_eq!(cfg.order_expiry(now, Some(45)), now + Duration::minutes(45));
        assert_eq!(cfg.order_expiry(now, Some(600)), now + Duration::hours(8));
    }

    #[test]
    fn config_loads_from_repo_file_and_sizes_lots() {
        let cfg = GuardConfig::load(concat!(env!("CARGO_MANIFEST_DIR"), "/config/guard.toml")).unwrap();
        assert_eq!(cfg.max_spread_for("eurusd"), Some(1.2));
        assert_eq!(cfg.max_spread_for("GBPUSD"), Some(1.0));
        assert_eq!(cfg.volume_lots(1_000_000.0, 10.0, Some(1000.0)), 0.01);
        let sized = GuardConfig { risk_pct: Some(0.5), pip_value_per_lot: Some(1000.0), ..cfg.clone() };
        assert!((sized.volume_lots(1_000_000.0, 10.0, None) - 0.5).abs() < 1e-9);
        // 固定値が無ければレートから求めた pip 価値で算出する（EURUSD・USDJPY=150 なら 1500 円）
        let live = GuardConfig { risk_pct: Some(0.5), pip_value_per_lot: None, ..cfg.clone() };
        assert!((live.volume_lots(1_000_000.0, 10.0, Some(1500.0)) - 0.33).abs() < 1e-9);
        assert_eq!(live.volume_lots(1_000_000.0, 10.0, None), 0.01);
    }

    #[test]
    fn settings_saved_in_the_llm_era_still_load() {
        let old: GuardConfig = serde_json::from_str(
            r#"{"observed_check":true,"min_confidence":0.7,"plan_max_hours":4.0,"stop_mode":"close","hard_stop_atr":2.0,"sl_atr_max_tf":"1h"}"#,
        )
        .unwrap();
        assert_eq!(old.order_max_hours, 4.0);
        assert_eq!(old.sl_atr_max_tf, AtrTimeframe::H1);
    }

    #[test]
    fn validate_rejects_unusable_values() {
        assert!(GuardConfig::default().validate().is_ok());
        let bad = GuardConfig { min_rr: -1.0, max_positions_total: 0, ..GuardConfig::default() };
        let err = bad.validate().unwrap_err().to_string();
        assert!(err.contains("min_rr") && err.contains("max_positions_total"), "{err}");

        let mut cfg = GuardConfig::default();
        cfg.max_spread_pips.clear();
        cfg.max_spread_pips.insert(" EURUSD_z ".into(), 1.2);
        cfg.max_spread_pips.insert("DEFAULT".into(), 2.0);
        cfg.normalize();
        assert!(cfg.max_spread_pips.contains_key("EURUSD_z"));
        assert_eq!(cfg.max_spread_for("eurusd_Z"), Some(1.2));
        assert_eq!(cfg.max_spread_for("GBPUSD"), Some(2.0));
        // FX 以外には default を当てない（明示したときだけ pips で見る）
        assert_eq!(cfg.max_spread_for("US500"), None);
    }
}
