//! 1回分の判断サイクル: Snapshot → LLM → 事後ガード → 条件執行/発注（ペーパー） → CoT ログ。
//! Step 9 の常駐ループはこのハンドラの中身をスケジューラから呼ぶ。

use axum::{extract::State, response::Json};
use chrono::{TimeZone, Utc};
use serde::Serialize;
use tracing::{info, warn};

use crate::ctrader::CandleBar;
use crate::executor::{OrderRequest, PlanEvent};
use crate::guard::{self, GuardContext, GuardVerdict};
use crate::server::handlers::chart::generate_snapshot_internal;
use crate::server::state::AppState;
use crate::server::types::{ApiResponse, CoTLog, Position};
use crate::snapshot::{MarketSnapshot, SnapshotBundle};
use crate::strategy::types::{Action, TradeDecision};
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

/// POST /api/decide
pub async fn decide_now(State(state): State<AppState>) -> Json<ApiResponse<DecideResponse>> {
    let _serial = state.decide_lock.lock().await;
    match run_decision_cycle(&state, "USDJPY").await {
        Ok(r) => Json(ApiResponse::ok(r)),
        Err(e) => {
            warn!("decision cycle failed: {e:#}");
            Json(ApiResponse::err(format!("{e:#}")))
        }
    }
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

async fn place_paper_order(state: &AppState, req: OrderRequest, reason: &str) -> anyhow::Result<Position> {
    let receipt = state.order_sink.place_market(&req).await?;
    let entry = receipt.filled_price.unwrap_or(req.entry_hint);
    let pos = Position {
        id: receipt.order_id,
        symbol: req.pair.clone(),
        side: action_str(req.action),
        volume_lots: req.volume_lots,
        entry_price: entry,
        current_price: entry,
        stop_loss: req.stop_loss,
        take_profit: req.take_profit,
        pnl_pips: 0.0,
        pnl_amount: 0.0,
        open_time: Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        invalidation_reason: reason.to_string(),
    };
    state.positions.write().await.push(pos.clone());
    Ok(pos)
}

/// 保持中の条件付きプランを最新の確定 5M 足で評価し、成立したものはガードを通して発注する
async fn process_pending_plans(state: &AppState, pair: &str, bundle: &SnapshotBundle) -> anyhow::Result<Vec<PlanEvent>> {
    let Some(bar) = last_closed_5m(&bundle.snapshot) else {
        return Ok(Vec::new());
    };
    let events = state.plan_book.write().await.on_bar_close(pair, &bar);
    let mut out = Vec::new();
    for (event, pending) in events {
        if let (PlanEvent::Triggered { entry, .. }, Some(p)) = (&event, pending) {
            let cfg = state.guard_config.read().await.clone();
            let ctx = guard_context(state, pair).await;
            let verdict = guard::evaluate_plan_trigger(&p.plan, *entry, &bundle.snapshot, &ctx, &cfg);
            let balance = state.metrics.read().await.balance;
            let sl_pips = (entry - p.plan.stop_loss.unwrap_or(*entry)).abs() / bundle.snapshot.pip_size.max(1e-9);
            let executed = if verdict.passed {
                let req = OrderRequest {
                    pair: pair.to_string(),
                    action: p.plan.then_action,
                    volume_lots: cfg.volume_lots(balance, sl_pips),
                    entry_hint: *entry,
                    stop_loss: p.plan.stop_loss.unwrap_or_default(),
                    take_profit: p.plan.take_profit.unwrap_or_default(),
                    reason: format!("conditional plan {} triggered: {}", p.id, p.plan.wait_for),
                };
                place_paper_order(state, req, &p.plan.invalidate_if).await.is_ok()
            } else {
                false
            };
            let log = CoTLog {
                id: format!("cot-plan-{}", Utc::now().timestamp_millis()),
                timestamp: Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                symbol: pair.to_string(),
                action: action_str(p.plan.then_action),
                confidence: 1.0,
                entry_type: Some("CONDITIONAL".into()),
                entry_price: Some(*entry),
                stop_loss: p.plan.stop_loss,
                take_profit: p.plan.take_profit,
                risk_reward_ratio: None,
                macro_context: format!("条件付きプラン {} が成立（元ログ: {:?}）", p.id, p.cot_log_id),
                order_flow: p.plan.wait_for.clone(),
                invalidation: p.plan.invalidate_if.clone(),
                conflicts: String::new(),
                guard_result: (!verdict.passed).then(|| verdict.summary()),
                reasoning: format!("5M確定足 {} 終値 {:.5} で成立条件を満たした", bar.timestamp.format("%H:%M"), bar.close),
                executed,
                spread_pips: bundle.snapshot.spread_pips,
            };
            state.cot_logs.write().await.insert(0, log);
        } else {
            info!(?event, "plan event");
        }
        out.push(event);
    }
    Ok(out)
}

pub async fn run_decision_cycle(state: &AppState, pair: &str) -> anyhow::Result<DecideResponse> {
    // 1. Snapshot（口座状態と直近判断を含む）
    generate_snapshot_internal(state, pair).await?;
    let bundle = state
        .latest_snapshot
        .read()
        .await
        .clone()
        .ok_or_else(|| anyhow::anyhow!("snapshot missing after generation"))?;

    // 2. 保持中プランの評価（LLM を呼ぶ前に、確定足で機械的に処理）
    let plan_events = process_pending_plans(state, pair, &bundle).await?;

    // 3. LLM 判断
    let lessons: Vec<String> = state.lessons.read().await.iter().filter(|l| l.active).map(|l| l.rule.clone()).collect();
    let prompt = PromptBuilder::new().with_lessons(lessons).build(&bundle.snapshot, &bundle.charts);
    let decision = state.llm.infer(&prompt).await?;

    // 4. 事後ガード
    let cfg = state.guard_config.read().await.clone();
    let ctx = guard_context(state, pair).await;
    let verdict = guard::evaluate(&decision, &bundle.snapshot, &ctx, &cfg);

    // 5. 執行（ペーパー）/ プラン登録
    let cot_id = format!("cot-{}", Utc::now().timestamp_millis());
    let is_trade = matches!(decision.action, Action::Buy | Action::Sell);
    let mut executed = false;
    let mut plan_id = None;
    if verdict.passed {
        if is_trade {
            let (entry, sl, tp) = (
                decision.entry_price.unwrap_or(bundle.snapshot.current_price),
                decision.stop_loss.unwrap_or_default(),
                decision.take_profit.unwrap_or_default(),
            );
            let balance = state.metrics.read().await.balance;
            let sl_pips = (entry - sl).abs() / bundle.snapshot.pip_size.max(1e-9);
            let req = OrderRequest {
                pair: pair.to_string(),
                action: decision.action,
                volume_lots: cfg.volume_lots(balance, sl_pips),
                entry_hint: entry,
                stop_loss: sl,
                take_profit: tp,
                reason: cot_id.clone(),
            };
            executed = place_paper_order(state, req, &decision.analysis.invalidation).await.is_ok();
        } else if let Some(plan) = decision.conditional_plan.clone().filter(|p| p.then_action != Action::Hold) {
            plan_id = Some(state.plan_book.write().await.add(pair, plan, Some(cot_id.clone())));
        }
    }

    // 6. CoT ログ
    let rr = match (decision.entry_price, decision.stop_loss, decision.take_profit) {
        (Some(e), Some(sl), Some(tp)) if (e - sl).abs() > 0.0 => Some(((tp - e) / (e - sl)).abs()),
        _ => None,
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
        guard_result: (!verdict.passed).then(|| verdict.summary()),
        reasoning: decision.reasoning.clone(),
        executed,
        spread_pips: bundle.snapshot.spread_pips,
    };
    state.cot_logs.write().await.insert(0, log);
    info!(action = ?decision.action, guard = %verdict.summary(), executed, ?plan_id, "decision cycle done");

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
