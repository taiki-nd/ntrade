//! 条件執行: LLM が返した条件付きプランを保持し、5M 確定ごとに機械的に評価する。
//!
//! 成立しても機械的には発注しない。成立足の終値はプランの想定価格からずれるので、成立を
//! 判断サイクルに渡し、LLM に今の価格で入るかを判断し直させる（`server::handlers::decide`）。

use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use tracing::{info, warn};

use crate::ctrader::CandleBar;
use crate::strategy::types::{Action, ConditionalPlan, PriceCondition};

/// 1本の確定足に対するプランの評価結果
#[derive(Debug, Clone, PartialEq)]
pub enum PlanStep {
    Waiting,
    /// 成立。エントリー価格は成立足の終値
    Triggered { action: Action, entry: f64 },
    Invalidated,
    Expired,
    /// 構造化条件が無く評価不能
    Unstructured,
}

fn met(cond: PriceCondition, price: f64, close: f64) -> bool {
    match cond {
        PriceCondition::CloseAbove => close > price,
        PriceCondition::CloseBelow => close < price,
    }
}

pub fn plan_expiry(plan: &ConditionalPlan) -> Option<DateTime<Utc>> {
    crate::storage::parse_ts(&plan.expires_at).ok().and_then(|s| Utc.timestamp_opt(s, 0).single())
}

/// 確定した1本の 5M 足でプランを評価する。破棄 → 期限 → 成立 の順。
/// リプレイ（`replay::score`）と本番（`Executor`）はこの関数を共有する。
pub fn step_plan(plan: &ConditionalPlan, bar: &CandleBar) -> PlanStep {
    if !plan.is_structured() {
        return PlanStep::Unstructured;
    }
    if met(plan.invalidate_condition.unwrap(), plan.invalidate_price.unwrap(), bar.close) {
        return PlanStep::Invalidated;
    }
    if let Some(exp) = plan_expiry(plan) {
        if bar.timestamp >= exp {
            return PlanStep::Expired;
        }
    }
    if met(plan.trigger_condition.unwrap(), plan.trigger_price.unwrap(), bar.close) {
        return PlanStep::Triggered { action: plan.then_action, entry: bar.close };
    }
    PlanStep::Waiting
}

/// 発注先の抽象。本番は cTrader、開発中はログのみ。
pub trait OrderSink: Send + Sync {
    fn place_market<'a>(
        &'a self,
        req: &'a OrderRequest,
    ) -> Pin<Box<dyn Future<Output = Result<OrderReceipt>> + Send + 'a>>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrderRequest {
    pub pair: String,
    pub action: Action,
    pub volume_lots: f64,
    pub entry_hint: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrderReceipt {
    pub order_id: String,
    pub filled_price: Option<f64>,
    /// ブローカー側に実際に置かれた SL/TP。要求値と一致するとは限らないので、
    /// ローカルの建玉にはこちらを記録する（決済理由の判定がこの値に依存する）。
    pub stop_loss: f64,
    pub take_profit: f64,
}

/// cTrader 未接続時のプレースホルダー。未接続時の発注はエラーとする。
pub struct DisconnectedOrderSink;

impl OrderSink for DisconnectedOrderSink {
    fn place_market<'a>(
        &'a self,
        _req: &'a OrderRequest,
    ) -> Pin<Box<dyn Future<Output = Result<OrderReceipt>> + Send + 'a>> {
        Box::pin(async move {
            anyhow::bail!("cTrader is not connected; cannot place order");
        })
    }
}

/// cTrader への実発注（サーバーサイド SL/TP 付き成行）
pub struct CTraderOrderSink {
    service: std::sync::Arc<crate::ctrader::CTraderService>,
}

impl CTraderOrderSink {
    pub fn new(service: std::sync::Arc<crate::ctrader::CTraderService>) -> Self {
        Self { service }
    }
}

/// エントリー価格から SL/TP までの距離を cTrader の相対値に変換する。
///
/// 相対 SL/TP の単位は価格の 1/100000。pip 幅ではなく価格差そのものを換算するため、
/// USDJPY（1 pip = 0.01）なら 1 pip = 1,000、EURUSD（1 pip = 0.0001）なら 1 pip = 10 になる。
pub fn relative_points(entry: f64, target: f64) -> i64 {
    ((entry - target).abs() * 100_000.0).round() as i64
}

/// 損切りの執行方法に応じて、ブローカーに置く SL と、ntrade が終値で判定する損切りラインを決める。
///
/// - `Touch`: LLM の SL をそのままブローカーに置く（終値判定なし）
/// - `Close`: LLM の SL は終値判定のラインにし、ブローカーには `hard_stop_atr` × ATR だけ外側のハードSLを置く。
///   ATR が取れないときはハードSLの幅を決められないので `Touch` と同じにする
///
/// 戻り値は（ブローカーに置く SL, 終値判定ライン）
pub fn protective_stops(action: Action, stop_loss: f64, atr: Option<f64>, cfg: &crate::guard::GuardConfig) -> (f64, Option<f64>) {
    let sign = match action {
        Action::Buy => 1.0,
        Action::Sell => -1.0,
        Action::Hold => return (stop_loss, None),
    };
    match (cfg.stop_mode, atr.filter(|a| *a > 0.0)) {
        (crate::guard::StopMode::Close, Some(atr)) if stop_loss > 0.0 => {
            (stop_loss - sign * cfg.hard_stop_atr * atr, Some(stop_loss))
        }
        _ => (stop_loss, None),
    }
}

/// 終値判定ラインを越えて確定したか（BUY は下抜け、SELL は上抜け）
pub fn close_stop_breached(side: &str, close_stop: f64, bar_close: f64) -> bool {
    if side == "BUY" {
        bar_close < close_stop
    } else {
        bar_close > close_stop
    }
}

/// 約定価格が想定からずれたときに、相対指定の SL/TP が実際に置かれる絶対価格。
///
/// cTrader は成行注文の SL/TP を「約定価格からの距離」として解釈するため、
/// 想定価格からの距離がそのまま約定価格に平行移動する。
pub fn shifted_levels(entry_hint: f64, filled: f64, stop_loss: f64, take_profit: f64) -> (f64, f64) {
    let shift = filled - entry_hint;
    (stop_loss + shift, take_profit + shift)
}

impl OrderSink for CTraderOrderSink {
    fn place_market<'a>(
        &'a self,
        req: &'a OrderRequest,
    ) -> Pin<Box<dyn Future<Output = Result<OrderReceipt>> + Send + 'a>> {
        Box::pin(async move {
            let is_buy = req.action == Action::Buy;
            // 約定価格は発注時点では分からないので、まず想定価格からの相対距離で SL/TP を付ける。
            // 建玉が一瞬でも無防備にならないようにするための保険であって、意図した価格ではない。
            let rel = |p: f64| relative_points(req.entry_hint, p);
            let volume = crate::ctrader::lots_to_volume(req.volume_lots);
            info!(pair = %req.pair, ?req.action, volume, sl = req.stop_loss, tp = req.take_profit, "LIVE order → cTrader");
            let ev = self
                .service
                .place_market_order_with_sltp(&req.pair, is_buy, volume, Some(rel(req.stop_loss)), Some(rel(req.take_profit)))
                .await?;
            if let Some(code) = ev.error_code.as_deref().filter(|c| !c.is_empty()) {
                anyhow::bail!("cTrader rejected order: {code}");
            }
            let position_id = ev
                .position
                .as_ref()
                .map(|p| p.position_id)
                .or_else(|| ev.deal.as_ref().map(|d| d.position_id))
                .ok_or_else(|| anyhow::anyhow!("execution event without position"))?;
            let filled = ev
                .deal
                .as_ref()
                .and_then(|d| d.execution_price)
                .or_else(|| ev.position.as_ref().and_then(|p| p.price));

            // 約定価格が判明したら、意図した絶対価格へ付け替える。
            // これをしないと SL/TP は約定ズレの分だけ平行移動したまま執行される。
            let mut effective = (req.stop_loss, req.take_profit);
            let pip = crate::snapshot::measures::get_pip_size(&req.pair);
            match filled {
                Some(fill)
                    if req.stop_loss > 0.0
                        && req.take_profit > 0.0
                        && (fill - req.entry_hint).abs() >= pip * 0.1 =>
                {
                    match self
                        .service
                        .amend_position_sltp(position_id, Some(req.stop_loss), Some(req.take_profit))
                        .await
                    {
                        Ok(_) => info!(
                            position_id, fill, hint = req.entry_hint, sl = req.stop_loss, tp = req.take_profit,
                            "SL/TP re-anchored to intended prices after fill"
                        ),
                        Err(e) => {
                            // 付け替えに失敗した場合、ブローカー側は相対指定のままなので、
                            // 記録も実際に置かれている価格に合わせる（判定と表示を嘘にしないため）
                            effective = shifted_levels(req.entry_hint, fill, req.stop_loss, req.take_profit);
                            warn!(
                                position_id, fill, sl = effective.0, tp = effective.1,
                                "failed to amend SL/TP after fill; broker keeps the shifted levels: {e:#}"
                            );
                        }
                    }
                }
                _ => {}
            }
            Ok(OrderReceipt {
                order_id: position_id.to_string(),
                filled_price: filled,
                stop_loss: effective.0,
                take_profit: effective.1,
            })
        })
    }
}

/// 保持中の条件付きプラン
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PendingPlan {
    pub id: String,
    pub pair: String,
    pub created_at: String,
    /// 元になった CoT ログの ID
    pub cot_log_id: Option<String>,
    pub plan: ConditionalPlan,
}

/// プランの状態遷移イベント
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PlanEvent {
    Triggered { plan_id: String, action: Action, entry: f64 },
    Invalidated { plan_id: String },
    Expired { plan_id: String },
    Discarded { plan_id: String },
}

/// 条件付きプランの保持と評価。発注そのものは呼び出し側が `OrderSink` で行う
/// （成立時にガードを通す必要があり、ガードは口座状態を必要とするため）。
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PlanBook {
    pub plans: Vec<PendingPlan>,
}

impl PlanBook {
    pub fn add(&mut self, pair: &str, plan: ConditionalPlan, cot_log_id: Option<String>) -> String {
        // 同一ペアのプランは最新1件だけ保持する（前回のシナリオは上書き）
        self.plans.retain(|p| p.pair != pair);
        let id = format!("plan-{}", Utc::now().timestamp_millis());
        self.plans.push(PendingPlan {
            id: id.clone(),
            pair: pair.to_string(),
            created_at: Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            cot_log_id,
            plan,
        });
        id
    }

    pub fn discard(&mut self, id: &str) -> Option<PlanEvent> {
        let before = self.plans.len();
        self.plans.retain(|p| p.id != id);
        (self.plans.len() < before).then(|| PlanEvent::Discarded { plan_id: id.to_string() })
    }

    pub fn for_pair(&self, pair: &str) -> Option<&PendingPlan> {
        self.plans.iter().find(|p| p.pair == pair)
    }

    /// 5M 足が確定したときに呼ぶ。成立/破棄/期限切れのプランは取り除き、イベントを返す。
    /// 成立したプランは呼び出し側がガードを通して発注する。
    pub fn on_bar_close(&mut self, pair: &str, bar: &CandleBar) -> Vec<(PlanEvent, Option<PendingPlan>)> {
        let mut events = Vec::new();
        let mut keep = Vec::new();
        for p in self.plans.drain(..) {
            if p.pair != pair {
                keep.push(p);
                continue;
            }
            match step_plan(&p.plan, bar) {
                PlanStep::Waiting => keep.push(p),
                PlanStep::Triggered { action, entry } => {
                    events.push((PlanEvent::Triggered { plan_id: p.id.clone(), action, entry }, Some(p)));
                }
                PlanStep::Invalidated => events.push((PlanEvent::Invalidated { plan_id: p.id.clone() }, None)),
                PlanStep::Expired | PlanStep::Unstructured => events.push((PlanEvent::Expired { plan_id: p.id.clone() }, None)),
            }
        }
        self.plans = keep;
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn plan() -> ConditionalPlan {
        ConditionalPlan {
            wait_for: "w".into(),
            then_action: Action::Buy,
            invalidate_if: "i".into(),
            expires_at: "2026-09-15 10:00:00 UTC".into(),
            trigger_price: Some(154.24),
            trigger_condition: Some(PriceCondition::CloseAbove),
            invalidate_price: Some(154.07),
            invalidate_condition: Some(PriceCondition::CloseBelow),
            stop_loss: Some(154.07),
            take_profit: Some(154.60),
        }
    }

    fn bar(t: DateTime<Utc>, close: f64) -> CandleBar {
        CandleBar { timestamp: t, open: close, high: close + 0.02, low: close - 0.02, close, volume: 1 }
    }

    #[test]
    fn relative_points_uses_price_units_not_pips() {
        // USDJPY: 11 pips = 0.110 → 11,000
        assert_eq!(relative_points(157.150, 157.040), 11_000);
        // USDJPY: 33 pips = 0.330 → 33,000
        assert_eq!(relative_points(157.150, 157.480), 33_000);
        // EURUSD: 11 pips = 0.0011 → 110
        assert_eq!(relative_points(1.08500, 1.08390), 110);
        // 向きに依らず絶対距離
        assert_eq!(relative_points(157.040, 157.150), 11_000);
    }

    #[test]
    fn shifted_levels_follow_the_fill() {
        // 実例（trd-10871878）: 指値 157.450 想定で SL 157.600 / TP 156.950 を送ったが、
        // 成行約定は 157.229 だったため、ブローカー側の SL は 157.379 に置かれていた
        let (sl, tp) = shifted_levels(157.450, 157.229, 157.600, 156.950);
        assert!((sl - 157.379).abs() < 1e-9, "sl was {sl}");
        assert!((tp - 156.729).abs() < 1e-9, "tp was {tp}");
        // 約定が想定どおりなら動かない
        let (sl, tp) = shifted_levels(157.450, 157.450, 157.600, 156.950);
        assert!((sl - 157.600).abs() < 1e-9 && (tp - 156.950).abs() < 1e-9);
    }

    #[test]
    fn protective_stops_follow_stop_mode() {
        use crate::guard::{GuardConfig, StopMode};
        let touch = GuardConfig::default();
        assert_eq!(protective_stops(Action::Sell, 157.70, Some(0.08), &touch), (157.70, None));

        let close = GuardConfig { stop_mode: StopMode::Close, hard_stop_atr: 2.0, ..GuardConfig::default() };
        // SELL: ハードSLは上へ 2 × 0.08 = 16 pips
        let (hard, soft) = protective_stops(Action::Sell, 157.70, Some(0.08), &close);
        assert!((hard - 157.86).abs() < 1e-9);
        assert_eq!(soft, Some(157.70));
        // BUY: 下へ
        let (hard, soft) = protective_stops(Action::Buy, 1.1360, Some(0.0004), &close);
        assert!((hard - 1.1352).abs() < 1e-9);
        assert_eq!(soft, Some(1.1360));
        // ATR が無いと幅を決められないので、LLM の SL をそのまま置く
        assert_eq!(protective_stops(Action::Sell, 157.70, None, &close), (157.70, None));
    }

    #[test]
    fn close_stop_breach_needs_a_close_beyond_the_line() {
        assert!(close_stop_breached("SELL", 157.70, 157.71));
        assert!(!close_stop_breached("SELL", 157.70, 157.70));
        assert!(!close_stop_breached("SELL", 157.70, 157.65));
        assert!(close_stop_breached("BUY", 1.1360, 1.1359));
        assert!(!close_stop_breached("BUY", 1.1360, 1.1361));
    }

    #[test]
    fn step_plan_transitions() {
        let t = Utc.with_ymd_and_hms(2026, 9, 15, 9, 0, 0).unwrap();
        assert_eq!(step_plan(&plan(), &bar(t, 154.20)), PlanStep::Waiting);
        assert_eq!(step_plan(&plan(), &bar(t, 154.26)), PlanStep::Triggered { action: Action::Buy, entry: 154.26 });
        assert_eq!(step_plan(&plan(), &bar(t, 154.05)), PlanStep::Invalidated);
        assert_eq!(step_plan(&plan(), &bar(t + Duration::hours(2), 154.20)), PlanStep::Expired);
        let mut p = plan();
        p.trigger_condition = None;
        assert_eq!(step_plan(&p, &bar(t, 154.26)), PlanStep::Unstructured);
    }

    #[test]
    fn plan_book_holds_one_plan_per_pair_and_emits_events() {
        let mut book = PlanBook::default();
        book.add("USDJPY", plan(), None);
        book.add("USDJPY", plan(), Some("cot-1".into()));
        book.add("EURUSD", plan(), None);
        assert_eq!(book.plans.len(), 2);
        assert_eq!(book.for_pair("USDJPY").unwrap().cot_log_id.as_deref(), Some("cot-1"));

        let t = Utc.with_ymd_and_hms(2026, 9, 15, 9, 0, 0).unwrap();
        let ev = book.on_bar_close("USDJPY", &bar(t, 154.20));
        assert!(ev.is_empty());
        let ev = book.on_bar_close("USDJPY", &bar(t, 154.30));
        assert_eq!(ev.len(), 1);
        assert!(matches!(ev[0].0, PlanEvent::Triggered { entry, .. } if (entry - 154.30).abs() < 1e-9));
        assert!(ev[0].1.is_some());
        assert_eq!(book.plans.len(), 1); // EURUSD だけ残る
        let id = book.plans[0].id.clone();
        assert!(book.discard(&id).is_some());
        assert!(book.plans.is_empty());
    }
}
