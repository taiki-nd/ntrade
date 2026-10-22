use axum::{
    extract::{Path, Query, State},
    response::Json,
};

use crate::server::state::AppState;
use crate::server::types::{ApiResponse, SignalLog, SignalQuery};
use crate::storage::Page;

/// GET /api/signals?limit=20&offset=0&symbol=&deployment=&status=&q=
/// 戦略が出した操作の記録（新しい順）
pub async fn list_signals(State(state): State<AppState>, Query(q): Query<SignalQuery>) -> Json<ApiResponse<Page<SignalLog>>> {
    match state.with_db(move |db| db.signals(&q)).await {
        Ok(page) => Json(ApiResponse::ok(page)),
        Err(e) => Json(ApiResponse::err(format!("{e:#}"))),
    }
}

/// GET /api/signals/{id}
pub async fn get_signal(State(state): State<AppState>, Path(id): Path<String>) -> Json<ApiResponse<SignalLog>> {
    let key = id.clone();
    match state.with_db(move |db| db.signal(&key)).await {
        Ok(Some(s)) => Json(ApiResponse::ok(s)),
        Ok(None) => Json(ApiResponse::err(format!("signal {id} not found"))),
        Err(e) => Json(ApiResponse::err(format!("{e:#}"))),
    }
}
