use axum::{
    extract::{Path, Query, State},
    response::Json,
};

use crate::server::state::AppState;
use crate::server::types::{ApiResponse, CoTDetail, CoTLog, PageQuery, MAX_PAGE_LIMIT};
use crate::storage::Page;

/// GET /api/cot?limit=20&offset=0&symbol=USDJPY&q=ブレイク
/// LLM思考プロセス（CoT）ログを新しい順に返却（`q` は本文のフリーワード検索）
pub async fn get_cot_logs(
    State(state): State<AppState>,
    Query(q): Query<PageQuery>,
) -> Json<ApiResponse<Page<CoTLog>>> {
    let (limit, offset) = (q.limit(), q.offset());
    match state.with_db(move |db| db.cot_logs(q.symbol.as_deref(), q.q.as_deref(), limit, offset)).await {
        Ok(page) => Json(ApiResponse::ok(page)),
        Err(e) => Json(ApiResponse::err(format!("{e:#}"))),
    }
}

/// GET /api/cot/{id}
/// 判断1件と、その判断から建てたポジション・決済履歴を返却
///
/// 条件付きプランは「発注した記録（成立ログ）」と「根拠になった LLM 判断」が別ログに分かれる。
/// 取引は成立ログに紐づくので、元の判断を開いたときも成立ログ経由の取引を拾えるようにする。
pub async fn get_cot_detail(State(state): State<AppState>, Path(id): Path<String>) -> Json<ApiResponse<CoTDetail>> {
    let key = id.clone();
    let res = state
        .with_db(move |db| {
            let Some(log) = db.cot_log(&key)? else { return Ok(None) };
            // この判断自身と、この判断から生まれた成立ログ
            let mut ids = vec![key.clone()];
            ids.extend(db.cot_logs_by_origin(&key)?.into_iter().map(|l| l.id));
            let origin = match log.origin_cot_log_id.as_deref() {
                Some(origin_id) => db.cot_log(origin_id)?,
                None => None,
            };
            let mut trades = Vec::new();
            for id in &ids {
                trades.extend(db.trades(Some(id), MAX_PAGE_LIMIT, 0)?.items);
            }
            trades.sort_by(|a, b| b.close_time.cmp(&a.close_time));
            Ok(Some((log, trades, ids, origin)))
        })
        .await;
    match res {
        Ok(Some((log, trades, ids, origin))) => {
            let open_positions = state
                .positions
                .read()
                .await
                .iter()
                .filter(|p| p.cot_log_id.as_ref().is_some_and(|c| ids.iter().any(|id| id == c)))
                .cloned()
                .collect();
            Json(ApiResponse::ok(CoTDetail { log, trades, open_positions, origin }))
        }
        Ok(None) => Json(ApiResponse::err(format!("CoT log {id} not found"))),
        Err(e) => Json(ApiResponse::err(format!("{e:#}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::handlers::trades::get_trades;
    use crate::server::types::{CloseReason, Position, TradeHistory, TradeQuery};

    fn cot(id: &str) -> CoTLog {
        CoTLog {
            id: id.into(),
            timestamp: "2026-09-15 09:00:00".into(),
            symbol: "USDJPY".into(),
            action: "BUY".into(),
            confidence: 0.8,
            entry_type: None,
            entry_price: None,
            stop_loss: None,
            take_profit: None,
            risk_reward_ratio: None,
            macro_context: "m".into(),
            order_flow: "o".into(),
            invalidation: "i".into(),
            conflicts: String::new(),
            guard_result: None,
            reasoning: "r".into(),
            executed: true,
            spread_pips: 0.3,
            origin_cot_log_id: None,
        }
    }

    #[tokio::test]
    async fn detail_lists_trades_and_open_positions_of_the_decision() {
        let mut state = AppState::new();
        state.db_path = std::env::temp_dir().join(format!("ntrade-test-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&state.db_path);

        state.record_cot_log(cot("cot-1")).await;
        state.record_cot_log(cot("cot-2")).await;
        *state.positions.write().await = vec![Position {
            id: "pos-2".into(),
            symbol: "USDJPY".into(),
            side: "BUY".into(),
            volume_lots: 0.1,
            entry_price: 154.2,
            current_price: 154.2,
            stop_loss: 154.1,
            take_profit: 154.4,
            pnl_pips: 0.0,
            pnl_amount: 0.0,
            open_time: "2026-09-15 09:05:00".into(),
            invalidation_reason: "i".into(),
            cot_log_id: Some("cot-1".into()),
        }];
        state.persist_positions().await;
        state
            .record_trades(vec![TradeHistory {
                id: "trd-1".into(),
                symbol: "USDJPY".into(),
                side: "BUY".into(),
                volume_lots: 0.1,
                entry_price: 154.2,
                close_price: 154.4,
                stop_loss: 154.1,
                take_profit: 154.4,
                pnl_pips: 20.0,
                pnl_amount: 2000.0,
                close_reason: CloseReason::TakeProfit,
                open_time: "2026-09-15 09:00:00".into(),
                close_time: "2026-09-15 09:30:00".into(),
                cot_log_id: Some("cot-1".into()),
            }])
            .await;

        let Json(res) = get_cot_detail(State(state.clone()), Path("cot-1".into())).await;
        let detail = res.data.expect("detail");
        assert_eq!(detail.trades.len(), 1);
        assert_eq!(detail.open_positions.len(), 1);

        let Json(res) = get_cot_detail(State(state.clone()), Path("cot-2".into())).await;
        let detail = res.data.expect("detail");
        assert!(detail.trades.is_empty() && detail.open_positions.is_empty());

        let q = PageQuery { limit: Some(1), ..Default::default() };
        let Json(res) = get_cot_logs(State(state.clone()), Query(q)).await;
        let page = res.data.expect("page");
        assert_eq!((page.items.len(), page.total), (1, 2));

        let q = TradeQuery { cot_log_id: Some("cot-2".into()), ..Default::default() };
        let Json(res) = get_trades(State(state.clone()), Query(q)).await;
        assert_eq!(res.data.expect("page").total, 0);

        // 再起動後もポジションが判断と紐付いたまま復元される
        let restored = state.with_db(|db| db.positions()).await.unwrap();
        assert_eq!(restored[0].cot_log_id.as_deref(), Some("cot-1"));

        let _ = std::fs::remove_file(&state.db_path);
    }

    /// 条件付きプランは「元の判断（HOLD）」と「成立ログ（発注した記録）」に分かれる。
    /// 取引は成立ログに紐づき、どちらを開いても取引と根拠の両方に辿り着けること。
    #[tokio::test]
    async fn conditional_plan_links_trade_to_trigger_log_and_back_to_origin() {
        let mut state = AppState::new();
        state.db_path = std::env::temp_dir().join(format!("ntrade-origin-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&state.db_path);

        let mut origin = cot("cot-origin");
        origin.action = "HOLD".into();
        origin.executed = false;
        let mut trigger = cot("cot-plan-1");
        trigger.action = "SELL".into();
        trigger.origin_cot_log_id = Some("cot-origin".into());
        state.record_cot_log(origin).await;
        state.record_cot_log(trigger).await;

        let t = TradeHistory {
            id: "trd-1".into(),
            symbol: "USDJPY".into(),
            side: "SELL".into(),
            volume_lots: 0.01,
            entry_price: 157.755,
            close_price: 157.475,
            stop_loss: 157.95,
            take_profit: 157.5,
            pnl_pips: 28.0,
            pnl_amount: 264.0,
            close_reason: CloseReason::TakeProfit,
            open_time: "2026-09-25 10:35:22".into(),
            close_time: "2026-09-25 13:00:04".into(),
            cot_log_id: Some("cot-plan-1".into()),
        };
        state.record_trades(vec![t]).await;

        // 成立ログ: 取引が紐づき、元の判断へ辿れる
        let Json(res) = get_cot_detail(State(state.clone()), Path("cot-plan-1".into())).await;
        let detail = res.data.expect("detail");
        assert_eq!(detail.log.action, "SELL");
        assert_eq!(detail.trades.len(), 1);
        assert_eq!(detail.origin.expect("origin").id, "cot-origin");

        // 元の判断: 自分に直接紐づく取引は無いが、成立ログ経由で拾える
        let Json(res) = get_cot_detail(State(state.clone()), Path("cot-origin".into())).await;
        let detail = res.data.expect("detail");
        assert_eq!(detail.log.action, "HOLD");
        assert_eq!(detail.trades.len(), 1);
        assert!(detail.origin.is_none());

        let _ = std::fs::remove_file(&state.db_path);
    }
}
