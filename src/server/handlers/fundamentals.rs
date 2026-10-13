//! ファンダメンタルズの事実（判断に渡すものと同じ）と、情報源ごとの取得状況。

use axum::{
    extract::{Query, State},
    response::Json,
};
use chrono::Utc;

use crate::fundamentals::{FundamentalFacts, SourceStatus};
use crate::server::handlers::chart::PairQuery;
use crate::server::state::AppState;
use crate::server::types::ApiResponse;

/// GET /api/fundamentals?pair=XAUUSD_z
pub async fn get_facts(State(state): State<AppState>, Query(q): Query<PairQuery>) -> Json<ApiResponse<FundamentalFacts>> {
    let pair = state.resolve_pair(q.pair.as_deref()).await;
    let ctrader = state.ctrader_service.read().await.clone();
    if let Some(c) = &ctrader {
        // 基軸・建値の解決に銘柄仕様を使う
        let _ = c.symbol_spec(&pair).await;
    }
    Json(ApiResponse::ok(state.fundamentals.facts_for(&pair, ctrader.as_deref(), Utc::now()).await))
}

/// GET /api/fundamentals/status
pub async fn get_status(State(state): State<AppState>) -> Json<ApiResponse<Vec<SourceStatus>>> {
    Json(ApiResponse::ok(state.fundamentals.status().await))
}

/// POST /api/fundamentals/refresh … 期限の来た情報源を今すぐ取り直す
pub async fn refresh(State(state): State<AppState>) -> Json<ApiResponse<Vec<SourceStatus>>> {
    state.fundamentals.refresh_due().await;
    Json(ApiResponse::ok(state.fundamentals.status().await))
}
