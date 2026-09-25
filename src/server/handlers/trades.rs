use axum::{
    extract::{Path, Query, State},
    response::Json,
};
use tracing::info;

use crate::server::state::AppState;
use crate::server::types::{ApiResponse, DeleteTradesRequest, TradeHistory, TradeQuery};
use crate::storage::Page;

/// GET /api/trades?limit=20&offset=0&q=&symbol=&side=&closeReason=&result=&from=&to=&minPnlPips=&maxPnlPips=&sort=&asc=
/// 決済履歴を条件で絞り込んで返却（既定は決済の新しい順）
pub async fn get_trades(
    State(state): State<AppState>,
    Query(q): Query<TradeQuery>,
) -> Json<ApiResponse<Page<TradeHistory>>> {
    match state.with_db(move |db| db.search_trades(&q)).await {
        Ok(page) => Json(ApiResponse::ok(page)),
        Err(e) => Json(ApiResponse::err(format!("{e:#}"))),
    }
}

/// DELETE /api/trades/:id
/// 決済履歴を1件削除（記録の削除のみ。ブローカー側の約定には影響しない）
pub async fn delete_trade(State(state): State<AppState>, Path(id): Path<String>) -> Json<ApiResponse<()>> {
    info!("Deleting trade record id: {}", id);

    let key = id.clone();
    match state.with_db(move |db| db.delete_trade(&key)).await {
        Ok(true) => Json(ApiResponse::ok_msg((), "約定履歴を削除しました")),
        Ok(false) => Json(ApiResponse::err(format!("Trade {} not found", id))),
        Err(e) => Json(ApiResponse::err(format!("{e:#}"))),
    }
}

/// POST /api/trades/delete
/// 決済履歴をまとめて削除（DELETE はボディを持てないため POST）
pub async fn delete_trades(
    State(state): State<AppState>,
    Json(payload): Json<DeleteTradesRequest>,
) -> Json<ApiResponse<usize>> {
    if payload.ids.is_empty() {
        return Json(ApiResponse::err("削除対象が指定されていません".to_string()));
    }
    info!("Deleting {} trade records", payload.ids.len());

    match state.with_db(move |db| db.delete_trades(&payload.ids)).await {
        Ok(n) => Json(ApiResponse::ok_msg(n, format!("約定履歴を {n} 件削除しました"))),
        Err(e) => Json(ApiResponse::err(format!("{e:#}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::types::{CloseReason, TradeSort};
    use axum::http::Uri;

    #[test]
    fn query_string_maps_to_search_conditions() {
        let uri: Uri = "/api/trades?limit=50&q=usdjpy&side=SELL&closeReason=STOP_LOSS\
            &result=loss&from=2026-09-01&to=2026-09-30&minPnlPips=-30&maxPnlPips=0\
            &sort=pnlAmount&asc=true"
            .parse()
            .unwrap();
        let Query(q) = Query::<TradeQuery>::try_from_uri(&uri).expect("query");

        assert_eq!((q.limit(), q.offset()), (50, 0));
        assert_eq!(q.q.as_deref(), Some("usdjpy"));
        assert_eq!(q.side.as_deref(), Some("SELL"));
        assert!(matches!(q.close_reason, Some(CloseReason::StopLoss)));
        assert_eq!(q.win_only(), Some(false));
        assert_eq!((q.from.as_deref(), q.to.as_deref()), (Some("2026-09-01"), Some("2026-09-30")));
        assert_eq!((q.min_pnl_pips, q.max_pnl_pips), (Some(-30.0), Some(0.0)));
        assert!(q.asc && q.sort == TradeSort::PnlAmount);

        // 既定値（条件なし）
        let uri: Uri = "/api/trades".parse().unwrap();
        let Query(q) = Query::<TradeQuery>::try_from_uri(&uri).expect("query");
        assert!(!q.asc && q.sort == TradeSort::CloseTime && q.win_only().is_none());
    }
}
