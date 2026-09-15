//! 条件執行: LLM が返した条件付きプランを保持し、5M 確定ごとに機械的に評価して発注する。
//!
//! LLM を再度呼ばずに執行するが、成立時には事後ガード（`guard::evaluate_plan_trigger`）を必ず通す。

use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use tracing::info;

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
}

/// 発注せずログだけ残す（ペーパー）。
pub struct PaperOrderSink;

impl OrderSink for PaperOrderSink {
    fn place_market<'a>(
        &'a self,
        req: &'a OrderRequest,
    ) -> Pin<Box<dyn Future<Output = Result<OrderReceipt>> + Send + 'a>> {
        Box::pin(async move {
            info!(?req, "PAPER order (not sent to broker)");
            Ok(OrderReceipt {
                order_id: format!("paper-{}", Utc::now().timestamp_millis()),
                filled_price: Some(req.entry_hint),
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
