use axum::{
    extract::{Query, State},
    response::Json,
};

use crate::server::state::AppState;
use crate::server::types::{ApiResponse, PageQuery, TradeHistory};
use crate::storage::Page;

/// GET /api/trades?limit=20&offset=0&cotLogId=cot-...
/// 決済履歴を新しい順に返却
pub async fn get_trades(
    State(state): State<AppState>,
    Query(q): Query<PageQuery>,
) -> Json<ApiResponse<Page<TradeHistory>>> {
    let (limit, offset) = (q.limit(), q.offset());
    match state.with_db(move |db| db.trades(q.cot_log_id.as_deref(), limit, offset)).await {
        Ok(page) => Json(ApiResponse::ok(page)),
        Err(e) => Json(ApiResponse::err(format!("{e:#}"))),
    }
}
