//! 1回分の判断サイクル: Snapshot → LLM → 既存の注文・建玉の操作 → 事後ガード → 予約注文 → CoT ログ。
//! 常駐ループはこのハンドラの中身をスケジューラから呼ぶ。

use axum::{
    extract::{Query, State},
    response::Json,
};
use chrono::{DateTime, Duration, TimeZone, Utc};
use serde::Serialize;
use tracing::{info, warn};

use crate::ctrader::{CandleBar, PendingKind};
use crate::executor::{close_stop_breached, protective_stops, OrderAmend, PendingOrderRequest};
use crate::guard::{self, GuardConfig, GuardContext, GuardVerdict};
use crate::server::handlers::chart::{generate_snapshot_internal, PairQuery};
use crate::server::state::AppState;
use crate::server::types::{ApiResponse, CloseReason, CoTLog, Position, TradeHistory, WorkingOrder};
use crate::snapshot::{MarketSnapshot, SnapshotBundle};
use crate::strategy::types::{Action, EntryType, ManageAction, ManageOp, ManageTarget, TradeDecision};
use crate::strategy::PromptBuilder;

#[derive(Debug, Serialize)]
pub struct DecideResponse {
    pub cot_log_id: String,
    pub decision: TradeDecision,
    pub guard: GuardVerdict,
    /// 新しい予約注文を置いたか
    pub executed: bool,
    /// 置いた予約注文の ID
    pub order_id: Option<String>,
    /// 既存の注文・建玉への操作の結果（1 操作 1 行）
    pub manage_results: Vec<String>,
    pub chart_dir: String,
}

/// POST /api/decide?pair=USDJPY（省略時は設定の先頭ペア）
pub async fn decide_now(State(state): State<AppState>, Query(q): Query<PairQuery>) -> Json<ApiResponse<DecideResponse>> {
    let pair = state.resolve_pair(q.pair.as_deref()).await;
    match run_cycle_locked(&state, &pair, true).await {
        Ok(Some(r)) => Json(ApiResponse::ok(r)),
        Ok(None) => Json(ApiResponse::err("decision skipped")),
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
///
/// `decide = false` のときは終値判定の損切りだけを行い、LLM は呼ばない（判断間隔の合間の 5M 足）。戻り値はそのとき None。
pub async fn run_cycle_locked(state: &AppState, pair: &str, decide: bool) -> anyhow::Result<Option<DecideResponse>> {
    let _gate = state.cycle_gate.read().await;
    let pair_lock = state.pair_lock(pair);
    let _pair = pair_lock.lock().await;
    run_decision_cycle(state, pair, decide).await
}

fn enum_str<T: Serialize>(v: T) -> String {
    serde_json::to_value(v).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default()
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
    let orders = state.working_orders.read().await;
    let metrics = state.metrics.read().await;
    GuardContext {
        open_positions_pair: positions.iter().filter(|p| p.symbol.eq_ignore_ascii_case(pair)).count(),
        open_positions_total: positions.len(),
        working_orders_pair: orders.iter().filter(|o| o.symbol.eq_ignore_ascii_case(pair)).count(),
        working_orders_total: orders.len(),
        daily_pnl_pct: metrics.daily_pnl_percent,
        now: None,
    }
}

/// 判断時点の 5M ATR（価格単位）。終値判定モードのハードSL幅に使う
fn atr_5m(snapshot: &MarketSnapshot) -> Option<f64> {
    snapshot.volatility.atr14_5m_pips.map(|p| p * snapshot.pip_size)
}

fn ts_string(t: DateTime<Utc>) -> String {
    t.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// 予約注文の期限。上限（`plan_max_hours`）を超える指定は上限に丸める（ガードを通った新規注文は上限内）
fn order_expiry(now: DateTime<Utc>, minutes: u32, cfg: &GuardConfig) -> DateTime<Utc> {
    let max = (cfg.plan_max_hours * 60.0).round() as i64;
    now + Duration::minutes((minutes as i64).clamp(1, max.max(1)))
}

fn side_of(s: &str) -> Action {
    match s {
        "BUY" => Action::Buy,
        "SELL" => Action::Sell,
        _ => Action::Hold,
    }
}

/// 終値判定の建玉を最新の確定 5M 足で評価し、損切りラインを越えて確定していれば成行で決済する。
/// 判定できるのはサイクルが回ったときだけなので、bot 停止中や接続断の間はブローカーのハードSLだけが守る。
async fn process_close_stops(state: &AppState, pair: &str, bundle: &SnapshotBundle) -> Vec<TradeHistory> {
    let Some(bar) = last_closed_5m(&bundle.snapshot) else {
        return Vec::new();
    };
    let bar_end = (bar.timestamp + Duration::minutes(5)).timestamp();
    let targets: Vec<Position> = state
        .positions
        .read()
        .await
        .iter()
        .filter(|p| p.symbol.eq_ignore_ascii_case(pair))
        .filter(|p| p.close_stop.is_some_and(|line| close_stop_breached(&p.side, line, bar.close)))
        // 建玉より前に確定した足では判定しない
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

/// LLM の SL を、損切りの執行方法に応じた（ブローカーの SL, 終値判定ライン）にする
fn stops_for(action: Action, sl: f64, snapshot: &MarketSnapshot, cfg: &GuardConfig) -> (f64, Option<f64>) {
    protective_stops(action, sl, atr_5m(snapshot), cfg)
}

/// 既存の予約注文・建玉への操作を 1 件実行し、結果を 1 行で返す
async fn apply_manage(
    state: &AppState,
    pair: &str,
    m: &ManageAction,
    snapshot: &MarketSnapshot,
    cfg: &GuardConfig,
    closed: &mut Vec<TradeHistory>,
) -> String {
    let label = format!("{} {} {}", enum_str(m.op), enum_str(m.target), m.id);
    let sink = state.order_sink.read().await.clone();
    let result: anyhow::Result<String> = async {
        match m.target {
            ManageTarget::Order => {
                let order = state
                    .working_orders
                    .read()
                    .await
                    .iter()
                    .find(|o| o.id == m.id && o.symbol.eq_ignore_ascii_case(pair))
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("NOT_FOUND: この銘柄の予約注文に無い"))?;
                let side = side_of(&order.side);
                if let Some(p) = guard::manage_problem(m, side, Some(order.entry_price), snapshot.current_price) {
                    anyhow::bail!("MANAGE_INVALID: {p}");
                }
                match m.op {
                    ManageOp::Cancel => {
                        sink.cancel_pending(&order.id).await?;
                        state.working_orders.write().await.retain(|o| o.id != order.id);
                        Ok("cancelled".to_string())
                    }
                    ManageOp::Modify => {
                        let stops = m.stop_loss.map(|sl| stops_for(side, sl, snapshot, cfg));
                        let expires = m.expiry_minutes.map(|min| order_expiry(Utc::now(), min, cfg));
                        // 変えない項目も今の値で送る。cTrader は省いた価格を 0 とみなして拒否する
                        // （SL だけ直した逆指値が「stopPrice 0.0」で弾かれた）
                        let current_expiry = crate::storage::parse_ts(&order.expires_at).ok().and_then(|t| Utc.timestamp_opt(t, 0).single());
                        sink.amend_pending(&OrderAmend {
                            pair: pair.to_string(),
                            order_id: order.id.clone(),
                            kind: order.kind,
                            entry_price: Some(m.entry_price.unwrap_or(order.entry_price)),
                            stop_loss: Some(stops.map(|(broker, _)| broker).unwrap_or(order.stop_loss)),
                            take_profit: Some(m.take_profit.unwrap_or(order.take_profit)),
                            expires_at: expires.or(current_expiry),
                        })
                        .await?;
                        if let Some(o) = state.working_orders.write().await.iter_mut().find(|o| o.id == order.id) {
                            if let Some(p) = m.entry_price {
                                o.entry_price = p;
                            }
                            if let Some((broker, close)) = stops {
                                o.stop_loss = broker;
                                o.close_stop = close;
                            }
                            if let Some(tp) = m.take_profit {
                                o.take_profit = tp;
                            }
                            if let Some(t) = expires {
                                o.expires_at = ts_string(t);
                            }
                        }
                        Ok("modified".to_string())
                    }
                    ManageOp::Close => anyhow::bail!("MANAGE_INVALID: 予約注文は CLOSE ではなく CANCEL"),
                }
            }
            ManageTarget::Position => {
                let pos = state
                    .positions
                    .read()
                    .await
                    .iter()
                    .find(|p| p.id == m.id && p.symbol.eq_ignore_ascii_case(pair))
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("NOT_FOUND: この銘柄の建玉に無い"))?;
                let side = side_of(&pos.side);
                if let Some(p) = guard::manage_problem(m, side, None, snapshot.current_price) {
                    anyhow::bail!("MANAGE_INVALID: {p}");
                }
                match m.op {
                    ManageOp::Close => {
                        let t = state.close_position_as(&pos.id, Some(CloseReason::Discretionary)).await?;
                        let pnl = t.pnl_pips;
                        closed.push(t);
                        Ok(format!("closed ({pnl:+.1} pips)"))
                    }
                    ManageOp::Modify => {
                        let stops = m.stop_loss.map(|sl| stops_for(side, sl, snapshot, cfg));
                        sink.amend_position(&pos.id, stops.map(|(broker, _)| broker).or(Some(pos.stop_loss)), m.take_profit.or(Some(pos.take_profit)))
                            .await?;
                        if let Some(p) = state.positions.write().await.iter_mut().find(|p| p.id == pos.id) {
                            if let Some((broker, close)) = stops {
                                p.stop_loss = broker;
                                p.close_stop = close;
                            }
                            if let Some(tp) = m.take_profit {
                                p.take_profit = tp;
                            }
                        }
                        Ok("modified".to_string())
                    }
                    ManageOp::Cancel => anyhow::bail!("MANAGE_INVALID: 建玉は CANCEL ではなく CLOSE"),
                }
            }
        }
    }
    .await;
    match result {
        Ok(r) => {
            info!(pair, %label, result = %r, reason = %m.reason, "manage action applied");
            format!("{label}: {r}")
        }
        Err(e) => {
            warn!(pair, %label, reason = %m.reason, "manage action failed: {e:#}");
            format!("{label}: FAILED {e:#}")
        }
    }
}

/// 新規の予約注文を置く。置けたら注文 ID、置けなかったら理由（`ORDER_FAILED: ...` など）
async fn place_new_order(
    state: &AppState,
    pair: &str,
    decision: &TradeDecision,
    snapshot: &MarketSnapshot,
    cfg: &GuardConfig,
    cot_id: &str,
) -> Result<String, String> {
    let kind = match decision.entry_type {
        Some(EntryType::Limit) => PendingKind::Limit,
        Some(EntryType::Stop) => PendingKind::Stop,
        other => return Err(format!("ENTRY_TYPE: {other:?}")),
    };
    let (entry, sl, tp) = (
        decision.entry_price.unwrap_or_default(),
        decision.stop_loss.unwrap_or_default(),
        decision.take_profit.unwrap_or_default(),
    );
    let (broker_sl, close_stop) = stops_for(decision.action, sl, snapshot, cfg);
    let sl_pips = (entry - broker_sl).abs() / snapshot.pip_size.max(1e-9);
    let volume_lots = state.order_volume(cfg, pair, sl_pips).await;
    // レバレッジを掛けるので、置く前に約定したときの必要証拠金と実効レバレッジを確かめる
    let detail = state.margin_check(cfg, pair, decision.action, volume_lots, entry).await?;
    info!(pair, volume_lots, %detail, "margin check passed");

    let now = Utc::now();
    let expires_at = order_expiry(now, decision.expiry_minutes.unwrap_or(60), cfg);
    let req = PendingOrderRequest {
        pair: pair.to_string(),
        action: decision.action,
        kind,
        volume_lots,
        entry_price: entry,
        stop_loss: broker_sl,
        take_profit: tp,
        expires_at,
        comment: cot_id.to_string(),
    };
    let sink = state.order_sink.read().await.clone();
    let id = sink.place_pending(&req).await.map_err(|e| format!("ORDER_FAILED: {e:#}"))?;
    state.working_orders.write().await.push(WorkingOrder {
        id: id.clone(),
        symbol: pair.to_string(),
        side: enum_str(decision.action),
        kind,
        volume_lots,
        entry_price: entry,
        stop_loss: broker_sl,
        take_profit: tp,
        close_stop,
        created_at: ts_string(now),
        expires_at: ts_string(expires_at),
        invalidation_reason: decision.analysis.invalidation.clone(),
        cot_log_id: Some(cot_id.to_string()),
    });
    Ok(id)
}

pub async fn run_decision_cycle(state: &AppState, pair: &str, decide: bool) -> anyhow::Result<Option<DecideResponse>> {
    // 1. Snapshot（口座状態・予約注文と直近判断を含む）
    let bundle = generate_snapshot_internal(state, pair).await?;

    // 2. 終値判定の損切り（LLM を呼ぶ前に、確定足で機械的に処理）
    let invalidated = process_close_stops(state, pair, &bundle).await;
    crate::scheduler::reflect_on_closed(state, invalidated).await;
    if !decide {
        return Ok(None);
    }

    // 3. LLM 判断
    let lessons: Vec<String> =
        state.with_db(|db| db.lessons()).await?.into_iter().filter(|l| l.active).map(|l| l.rule).collect();
    let prompt = PromptBuilder::new().with_lessons(lessons).build(&bundle.snapshot);
    let decision = state.llm.infer(&prompt).await?;
    let snapshot = &bundle.snapshot;

    // 4. 執行。操作から注文の記録までを他ペアの発注と重ねない
    let cfg = state.guard_config().await;
    let order_guard = state.order_lock.lock().await;
    let cot_id = format!("cot-{}", Utc::now().timestamp_millis());

    // 4a. 既存の注文・建玉の操作を先に行う（取り消した注文の枠を新しい注文に使えるように）
    let mut manage_results = Vec::new();
    let mut discretionary = Vec::new();
    for m in &decision.manage {
        manage_results.push(apply_manage(state, pair, m, snapshot, &cfg, &mut discretionary).await);
    }
    if !decision.manage.is_empty() {
        state.persist_positions().await;
    }

    // 4b. 新規の予約注文
    let ctx = guard_context(state, pair).await;
    let verdict = guard::evaluate(&decision, snapshot, &ctx, &cfg);
    let mut order_id = None;
    let mut execution_error = None;
    if verdict.passed && matches!(decision.action, Action::Buy | Action::Sell) {
        match place_new_order(state, pair, &decision, snapshot, &cfg, &cot_id).await {
            Ok(id) => {
                info!(pair, order = %id, "pending order placed");
                order_id = Some(id);
            }
            Err(reason) => {
                warn!(pair, %reason, "pending order not placed");
                execution_error = Some(reason);
            }
        }
    }
    state.persist_working_orders().await;
    drop(order_guard);
    crate::scheduler::reflect_on_closed(state, discretionary).await;

    // 5. CoT ログ
    let rr = match (decision.entry_price, decision.stop_loss, decision.take_profit) {
        (Some(e), Some(sl), Some(tp)) if (e - sl).abs() > 0.0 => Some(((tp - e) / (e - sl)).abs()),
        _ => None,
    };
    let mut notes: Vec<String> = Vec::new();
    if !verdict.passed {
        notes.push(verdict.summary());
    }
    notes.extend(execution_error);
    notes.extend(manage_results.iter().map(|r| format!("MANAGE {r}")));
    let executed = order_id.is_some();
    let log = CoTLog {
        id: cot_id.clone(),
        timestamp: ts_string(Utc::now()),
        symbol: pair.to_string(),
        action: enum_str(decision.action),
        confidence: decision.confidence,
        entry_type: decision.entry_type.map(enum_str),
        entry_price: decision.entry_price,
        stop_loss: decision.stop_loss,
        take_profit: decision.take_profit,
        risk_reward_ratio: rr,
        fundamental_view: String::new(),
        // macro_context 列には流れの読み、order_flow 列には価格の根拠を入れる
        macro_context: decision.analysis.market_structure.clone(),
        order_flow: decision.analysis.price_levels.clone(),
        invalidation: decision.analysis.invalidation.clone(),
        conflicts: decision.analysis.conflicts.clone(),
        guard_result: (!notes.is_empty()).then(|| notes.join(" / ")),
        reasoning: decision.reasoning.clone(),
        executed,
        spread_pips: snapshot.spread_pips.unwrap_or(0.0),
        origin_cot_log_id: None,
    };
    state.record_cot_log(log).await;
    info!(pair, action = ?decision.action, guard = %verdict.summary(), executed, ?order_id, manage = manage_results.len(), "decision cycle done");

    Ok(Some(DecideResponse {
        cot_log_id: cot_id,
        decision,
        guard: verdict,
        executed,
        order_id,
        manage_results,
        chart_dir: bundle.charts.dir.to_string_lossy().to_string(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_expiry_is_capped_by_the_configured_maximum() {
        let now = Utc.with_ymd_and_hms(2026, 10, 13, 9, 0, 0).unwrap();
        let cfg = GuardConfig { plan_max_hours: 2.0, ..GuardConfig::default() };
        assert_eq!(order_expiry(now, 45, &cfg), now + Duration::minutes(45));
        assert_eq!(order_expiry(now, 600, &cfg), now + Duration::minutes(120));
        assert_eq!(order_expiry(now, 0, &cfg), now + Duration::minutes(1));
    }

    #[test]
    fn side_strings_map_to_actions() {
        assert_eq!(side_of("BUY"), Action::Buy);
        assert_eq!(side_of("SELL"), Action::Sell);
        assert_eq!(side_of(""), Action::Hold);
        assert_eq!(enum_str(Action::Sell), "SELL");
        assert_eq!(enum_str(PendingKind::Stop), "STOP");
    }
}
