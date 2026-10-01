//! 1回分の判断サイクル: Snapshot → LLM → 事後ガード → 条件執行/発注（ペーパー） → CoT ログ。
//! Step 9 の常駐ループはこのハンドラの中身をスケジューラから呼ぶ。

use axum::{
    extract::{Query, State},
    response::Json,
};
use chrono::{TimeZone, Utc};
use serde::Serialize;
use tracing::{info, warn};

use crate::ctrader::CandleBar;
use crate::executor::{close_stop_breached, protective_stops, OrderRequest, PlanEvent};
use crate::guard::{self, GuardContext, GuardVerdict};
use crate::server::handlers::chart::{generate_snapshot_internal, PairQuery};
use crate::server::state::AppState;
use crate::server::types::{ApiResponse, CloseReason, CoTLog, Position, TradeHistory};
use crate::snapshot::{MarketSnapshot, SnapshotBundle, TriggeredPlan};
use crate::strategy::types::{Action, ConditionalPlan, EntryType, PriceCondition, TradeDecision};
use crate::strategy::PromptBuilder;

#[derive(Debug, Serialize)]
pub struct DecideResponse {
    pub cot_log_id: String,
    pub decision: TradeDecision,
    pub guard: GuardVerdict,
    pub executed: bool,
    pub plan_id: Option<String>,
    pub plan_events: Vec<PlanEvent>,
    pub chart_dir: String,
}

/// POST /api/decide?pair=USDJPY（省略時は設定の先頭ペア）
pub async fn decide_now(State(state): State<AppState>, Query(q): Query<PairQuery>) -> Json<ApiResponse<DecideResponse>> {
    let pair = state.resolve_pair(q.pair.as_deref()).await;
    match run_cycle_locked(&state, &pair).await {
        Ok(r) => Json(ApiResponse::ok(r)),
        Err(e) => {
            warn!("decision cycle failed: {e:#}");
            Json(ApiResponse::err(format!("{e:#}")))
        }
    }
}

/// ロックを取って判断サイクルを回す。
///
/// ペアが違えば並行に走る（LLM 推論が1回数十秒かかるため、直列だと後ろのペアほど足確定から遅れる）。
/// 同じペアはスケジューラと手動実行が重ならないよう直列にし、cTrader 接続の差し替え中は待つ。
/// ロックの順序は「接続ゲート → ペア」で固定する（逆順だと接続差し替えの待ちと循環しうる）。
pub async fn run_cycle_locked(state: &AppState, pair: &str) -> anyhow::Result<DecideResponse> {
    let _gate = state.cycle_gate.read().await;
    let pair_lock = state.pair_lock(pair);
    let _pair = pair_lock.lock().await;
    run_decision_cycle(state, pair).await
}

fn action_str(a: Action) -> String {
    serde_json::to_value(a).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default()
}

fn last_closed_5m(snapshot: &MarketSnapshot) -> Option<CandleBar> {
    let row = snapshot.bars_5m.last()?;
    let secs = crate::storage::parse_ts(&row.t).ok()?;
    Some(CandleBar {
        timestamp: Utc.timestamp_opt(secs, 0).single()?,
        open: row.o,
        high: row.h,
        low: row.l,
        close: row.c,
        volume: 0,
    })
}

async fn guard_context(state: &AppState, pair: &str) -> GuardContext {
    let positions = state.positions.read().await;
    let metrics = state.metrics.read().await;
    GuardContext {
        open_positions_pair: positions.iter().filter(|p| p.symbol.eq_ignore_ascii_case(pair)).count(),
        open_positions_total: positions.len(),
        daily_pnl_pct: metrics.daily_pnl_percent,
        now: None,
    }
}

/// 判断時点の 5M ATR（価格単位）。終値判定モードのハードSL幅に使う
fn atr_5m(snapshot: &MarketSnapshot) -> Option<f64> {
    snapshot.volatility.atr14_5m_pips.map(|p| p * snapshot.pip_size)
}

/// `req.stop_loss` はブローカーに置く SL。`close_stop` は終値判定の損切りライン（`stop_mode = close` のときだけ）
async fn execute_order(
    state: &AppState,
    req: OrderRequest,
    reason: &str,
    cot_log_id: &str,
    close_stop: Option<f64>,
) -> anyhow::Result<Position> {
    let sink = state.order_sink.read().await.clone();
    let receipt = sink.place_market(&req).await?;
    let entry = receipt.filled_price.unwrap_or(req.entry_hint);
    if (receipt.stop_loss - req.stop_loss).abs() > 1e-9 || (receipt.take_profit - req.take_profit).abs() > 1e-9 {
        warn!(
            position = %receipt.order_id, requested_sl = req.stop_loss, actual_sl = receipt.stop_loss,
            requested_tp = req.take_profit, actual_tp = receipt.take_profit,
            "broker placed SL/TP at different levels than requested"
        );
    }
    let pos = Position {
        id: receipt.order_id,
        symbol: req.pair.clone(),
        side: action_str(req.action),
        volume_lots: req.volume_lots,
        entry_price: entry,
        current_price: entry,
        // ブローカーに実際に置かれた値を記録する。要求値を書くと決済理由の判定がずれる
        stop_loss: receipt.stop_loss,
        take_profit: receipt.take_profit,
        pnl_pips: 0.0,
        pnl_amount: 0.0,
        open_time: Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        invalidation_reason: reason.to_string(),
        cot_log_id: Some(cot_log_id.to_string()),
        close_stop,
    };
    state.positions.write().await.push(pos.clone());
    state.persist_positions().await;
    Ok(pos)
}

/// 指値がすでに成行で約定できる位置にあるか。
/// SELL 指値は現在値が指値以上、BUY 指値は指値以下なら、指値を出しても即約定するので成行と同じ。
fn limit_is_marketable(action: Action, limit: f64, current: f64) -> bool {
    match action {
        Action::Buy => current <= limit,
        Action::Sell => current >= limit,
        Action::Hold => false,
    }
}

/// LIMIT 判断を「指値到達待ち」の条件付きプランに変換する。
///
/// 成行に落とすと指値から離れた価格で約定し、SL/TP までの距離ごと狂う。
/// 到達待ちに回すことで、成立時に実際の価格でガードを通し直せる。
/// ただし成立判定は 5M 確定足の終値なので、指値へのヒゲだけでは約定しない（本来の指値より保守的）。
fn plan_from_limit(decision: &TradeDecision, entry: f64, sl: f64, tp: f64, max_hours: f64) -> ConditionalPlan {
    let toward = if decision.action == Action::Buy {
        PriceCondition::CloseBelow
    } else {
        PriceCondition::CloseAbove
    };
    let expires = Utc::now() + chrono::Duration::minutes((max_hours * 60.0) as i64);
    ConditionalPlan {
        wait_for: format!("指値 {entry} への到達（5M確定足の終値で判定）"),
        then_action: decision.action,
        invalidate_if: decision.analysis.invalidation.clone(),
        expires_at: expires.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        trigger_price: Some(entry),
        trigger_condition: Some(toward),
        // 指値に届く前に SL 水準を抜けたら、そのシナリオはもう成立していない
        invalidate_price: Some(sl),
        invalidate_condition: Some(toward),
        stop_loss: Some(sl),
        take_profit: Some(tp),
    }
}

/// プラン成立を受けた再判断の材料
struct PlanTrigger {
    notice: TriggeredPlan,
    /// プランを立てた判断。再判断の CoT ログの origin になる
    origin_cot_log_id: Option<String>,
}

/// 保持中の条件付きプランを最新の確定 5M 足で評価する。
///
/// 成立しても発注はしない。成立足の終値はプランを立てた時の想定価格からずれ、プランの SL/TP のままでは
/// リスクリワードが崩れるため。成立を返し、同じサイクルの LLM 判断に「今の価格で入るか」を判断させる。
async fn process_pending_plans(state: &AppState, pair: &str, bundle: &SnapshotBundle) -> (Vec<PlanEvent>, Option<PlanTrigger>) {
    let Some(bar) = last_closed_5m(&bundle.snapshot) else {
        return (Vec::new(), None);
    };
    let events = state.plan_book.write().await.on_bar_close(pair, &bar);
    let mut out = Vec::new();
    let mut trigger = None;
    for (event, pending) in events {
        info!(?event, "plan event");
        if let (PlanEvent::Triggered { .. }, Some(p)) = (&event, pending) {
            trigger = Some(PlanTrigger {
                notice: TriggeredPlan {
                    planned_at: p.created_at,
                    triggered_bar: bar.timestamp.format("%Y-%m-%d %H:%M").to_string(),
                    triggered_close: bar.close,
                    plan: p.plan,
                },
                origin_cot_log_id: p.cot_log_id,
            });
        }
        out.push(event);
    }
    (out, trigger)
}

/// 終値判定の建玉を最新の確定 5M 足で評価し、損切りラインを越えて確定していれば成行で決済する。
/// 判定できるのはサイクルが回ったときだけなので、bot 停止中や接続断の間はブローカーのハードSLだけが守る。
async fn process_close_stops(state: &AppState, pair: &str, bundle: &SnapshotBundle) -> Vec<TradeHistory> {
    let Some(bar) = last_closed_5m(&bundle.snapshot) else {
        return Vec::new();
    };
    let bar_end = (bar.timestamp + chrono::Duration::minutes(5)).timestamp();
    let targets: Vec<Position> = state
        .positions
        .read()
        .await
        .iter()
        .filter(|p| p.symbol.eq_ignore_ascii_case(pair))
        .filter(|p| p.close_stop.is_some_and(|line| close_stop_breached(&p.side, line, bar.close)))
        // 建玉より前に確定した足（エントリーを決めた足など）では判定しない
        .filter(|p| crate::storage::parse_ts(&p.open_time).is_ok_and(|opened| bar_end > opened))
        .cloned()
        .collect();

    let mut closed = Vec::new();
    for p in targets {
        info!(
            position = %p.id, pair, side = %p.side, close_stop = ?p.close_stop, bar_close = bar.close,
            bar = %bar.timestamp.format("%H:%M"), "5M close beyond the stop line; closing at market"
        );
        match state.close_position_as(&p.id, Some(CloseReason::Invalidated)).await {
            Ok(t) => closed.push(t),
            // 失敗しても建玉はローカルに残るので、次の足で再判定される。ハードSLも置いてある
            Err(e) => warn!(position = %p.id, "failed to close invalidated position: {e:#}"),
        }
    }
    closed
}

pub async fn run_decision_cycle(state: &AppState, pair: &str) -> anyhow::Result<DecideResponse> {
    // 1. Snapshot（口座状態と直近判断を含む）
    let mut bundle = generate_snapshot_internal(state, pair).await?;

    // 2. 保持中プランの評価と終値判定の損切り（LLM を呼ぶ前に、確定足で機械的に処理）
    let (plan_events, plan_trigger) = process_pending_plans(state, pair, &bundle).await;
    let invalidated = process_close_stops(state, pair, &bundle).await;
    crate::scheduler::reflect_on_closed(state, invalidated).await;
    // 成立したプランは、この判断で入るかどうかを決めさせる
    if let Some(t) = &plan_trigger {
        bundle.snapshot.account_state.triggered_plan = Some(t.notice.clone());
    }

    // 3. LLM 判断
    let lessons: Vec<String> =
        state.with_db(|db| db.lessons()).await?.into_iter().filter(|l| l.active).map(|l| l.rule).collect();
    let prompt = PromptBuilder::new().with_lessons(lessons).build(&bundle.snapshot, &bundle.charts);
    let decision = state.llm.infer(&prompt).await?;

    // 4. 事後ガード。建玉数の確認から建玉の記録までを他ペアの発注と重ねない
    let cfg = state.guard_config().await;
    let order_guard = state.order_lock.lock().await;
    let ctx = guard_context(state, pair).await;
    let verdict = guard::evaluate(&decision, &bundle.snapshot, &ctx, &cfg);

    // 5. 執行（実発注）/ プラン登録
    let cot_id = format!("cot-{}", Utc::now().timestamp_millis());
    let is_trade = matches!(decision.action, Action::Buy | Action::Sell);
    let mut executed = false;
    let mut plan_id = None;
    let mut execution_error = None;
    if verdict.passed {
        if is_trade {
            let (entry, sl, tp) = (
                decision.entry_price.unwrap_or(bundle.snapshot.current_price),
                decision.stop_loss.unwrap_or_default(),
                decision.take_profit.unwrap_or_default(),
            );
            let current = bundle.snapshot.current_price;
            let is_pending_limit = decision.entry_type == Some(EntryType::Limit)
                && !limit_is_marketable(decision.action, entry, current);
            if is_pending_limit {
                // 指値未到達。成行で埋めると約定価格も SL/TP も指値の想定からずれるので、到達待ちにする
                let plan = plan_from_limit(&decision, entry, sl, tp, cfg.plan_max_hours);
                let id = state.plan_book.write().await.add(pair, plan, Some(cot_id.clone()));
                info!(pair, entry, current, plan_id = %id, "LIMIT entry not marketable; held as a pending plan");
                execution_error = Some(format!("LIMIT_PENDING: 指値 {entry} 未到達（現在値 {current}）のためプラン {id} として保持"));
                plan_id = Some(id);
            } else {
                let (broker_sl, close_stop) = protective_stops(decision.action, sl, atr_5m(&bundle.snapshot), &cfg);
                let sl_pips = (entry - broker_sl).abs() / bundle.snapshot.pip_size.max(1e-9);
                let req = OrderRequest {
                    pair: pair.to_string(),
                    action: decision.action,
                    volume_lots: state.order_volume(&cfg, pair, sl_pips).await,
                    entry_hint: entry,
                    stop_loss: broker_sl,
                    take_profit: tp,
                    reason: cot_id.clone(),
                };
                match execute_order(state, req, &decision.analysis.invalidation, &cot_id, close_stop).await {
                    Ok(_) => executed = true,
                    Err(e) => {
                        warn!("Failed to place order: {e:#}");
                        execution_error = Some(format!("ORDER_FAILED: {e:#}"));
                    }
                }
            }
        } else if let Some(plan) = decision.conditional_plan.clone().filter(|p| p.then_action != Action::Hold) {
            plan_id = Some(state.plan_book.write().await.add(pair, plan, Some(cot_id.clone())));
        }
    }
    drop(order_guard);

    // 6. CoT ログ
    let rr = match (decision.entry_price, decision.stop_loss, decision.take_profit) {
        (Some(e), Some(sl), Some(tp)) if (e - sl).abs() > 0.0 => Some(((tp - e) / (e - sl)).abs()),
        _ => None,
    };
    let guard_result = if !verdict.passed {
        Some(verdict.summary())
    } else {
        execution_error
    };
    let log = CoTLog {
        id: cot_id.clone(),
        timestamp: Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        symbol: pair.to_string(),
        action: action_str(decision.action),
        confidence: decision.confidence,
        entry_type: decision.entry_type.and_then(|e| serde_json::to_value(e).ok()).and_then(|v| v.as_str().map(String::from)),
        entry_price: decision.entry_price,
        stop_loss: decision.stop_loss,
        take_profit: decision.take_profit,
        risk_reward_ratio: rr,
        macro_context: decision.analysis.macro_context.clone(),
        order_flow: decision.analysis.order_flow.clone(),
        invalidation: decision.analysis.invalidation.clone(),
        conflicts: decision.analysis.conflicts.clone(),
        guard_result,
        reasoning: decision.reasoning.clone(),
        executed,
        spread_pips: bundle.snapshot.spread_pips,
        // プラン成立を受けた再判断なら、プランを立てた判断へ辿れるようにする
        origin_cot_log_id: plan_trigger.and_then(|t| t.origin_cot_log_id),
    };
    state.record_cot_log(log).await;
    info!(pair, action = ?decision.action, guard = %verdict.summary(), executed, ?plan_id, "decision cycle done");

    Ok(DecideResponse {
        cot_log_id: cot_id,
        decision,
        guard: verdict,
        executed,
        plan_id,
        plan_events,
        chart_dir: bundle.charts.dir.to_string_lossy().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::{plan_expiry, step_plan, PlanStep};
    use crate::strategy::types::{Analysis, Observed};

    fn decision(action: Action, entry: f64) -> TradeDecision {
        TradeDecision {
            action,
            confidence: 0.7,
            entry_type: Some(EntryType::Limit),
            entry_price: Some(entry),
            stop_loss: Some(157.600),
            take_profit: Some(156.950),
            analysis: Analysis {
                macro_context: String::new(),
                order_flow: String::new(),
                invalidation: "157.600 を実体で上抜けたら無効".into(),
                conflicts: String::new(),
            },
            conditional_plan: None,
            observed: Observed::default(),
            reasoning: String::new(),
        }
    }

    fn bar(close: f64) -> CandleBar {
        CandleBar {
            timestamp: Utc::now(),
            open: close,
            high: close + 0.02,
            low: close - 0.02,
            close,
            volume: 0,
        }
    }

    #[test]
    fn limit_is_marketable_only_when_price_already_reached_it() {
        // 実例: SELL 指値 157.450 に対して現在値 157.229。まだ届いていない
        assert!(!limit_is_marketable(Action::Sell, 157.450, 157.229));
        assert!(limit_is_marketable(Action::Sell, 157.450, 157.480));
        assert!(limit_is_marketable(Action::Sell, 157.450, 157.450));
        assert!(!limit_is_marketable(Action::Buy, 157.000, 157.229));
        assert!(limit_is_marketable(Action::Buy, 157.000, 156.900));
    }

    #[test]
    fn pending_limit_becomes_a_plan_that_waits_for_the_limit_price() {
        let d = decision(Action::Sell, 157.450);
        let plan = plan_from_limit(&d, 157.450, 157.600, 156.950, 4.0);
        assert!(plan.is_structured());
        assert!(plan_expiry(&plan).is_some(), "expires_at must be parsable: {}", plan.expires_at);

        // 指値未到達なら待ち、到達したら成立
        assert_eq!(step_plan(&plan, &bar(157.300)), PlanStep::Waiting);
        assert!(matches!(step_plan(&plan, &bar(157.480)), PlanStep::Triggered { action: Action::Sell, .. }));
        // 指値に届く前に SL 水準を抜けたらシナリオごと破棄
        assert_eq!(step_plan(&plan, &bar(157.650)), PlanStep::Invalidated);
    }

    #[test]
    fn buy_limit_plan_waits_below_and_invalidates_lower() {
        let mut d = decision(Action::Buy, 157.000);
        d.stop_loss = Some(156.800);
        d.take_profit = Some(157.600);
        let plan = plan_from_limit(&d, 157.000, 156.800, 157.600, 4.0);
        assert_eq!(step_plan(&plan, &bar(157.200)), PlanStep::Waiting);
        assert!(matches!(step_plan(&plan, &bar(156.950)), PlanStep::Triggered { action: Action::Buy, .. }));
        assert_eq!(step_plan(&plan, &bar(156.700)), PlanStep::Invalidated);
    }

    /// プランが成立しても発注はせず、成立を再判断の材料として返す
    #[tokio::test]
    async fn triggered_plan_is_handed_to_the_llm_instead_of_being_ordered() {
        use crate::snapshot::{AccountState, ChartSet, SnapshotConfig, SnapshotInput};

        let state = AppState::new();
        let plan = plan_from_limit(&decision(Action::Sell, 157.400), 157.400, 157.600, 156.950, 8.0);
        state.plan_book.write().await.add("USDJPY", plan.clone(), Some("cot-origin".into()));

        // 5M 足が指値 157.400 を上抜けて確定 → 成立
        let mut b = bar(157.430);
        b.timestamp = Utc::now() - chrono::Duration::minutes(10);
        let snapshot = MarketSnapshot::build(
            &SnapshotInput {
                pair: "USDJPY",
                bars_4h: &[],
                bars_1h: &[],
                bars_15m: &[],
                bars_5m: &[b.clone()],
                spread_pips: 0.0,
                current_price: None,
                now: None,
                account_state: AccountState::default(),
            },
            &SnapshotConfig::default(),
        );
        let charts = ChartSet {
            dir: "x".into(),
            h4: "x/4H.png".into(),
            h1: "x/1H.png".into(),
            m15: "x/15M.png".into(),
            m5: "x/5M.png".into(),
        };
        let (events, trigger) = process_pending_plans(&state, "USDJPY", &SnapshotBundle { snapshot, charts }).await;

        assert!(matches!(events.as_slice(), [PlanEvent::Triggered { action: Action::Sell, .. }]));
        let t = trigger.expect("triggered plan");
        assert_eq!(t.origin_cot_log_id.as_deref(), Some("cot-origin"));
        assert_eq!(t.notice.triggered_close, 157.430);
        assert_eq!(t.notice.plan, plan);
        assert!(state.positions.read().await.is_empty(), "a triggered plan must not place an order by itself");
        assert!(state.plan_book.read().await.for_pair("USDJPY").is_none());
    }
}
