use axum::{
    extract::{Path, State},
    response::Json,
};

use crate::executor::PendingPlan;
use crate::guard::GuardConfig;
use crate::server::state::AppState;
use crate::server::types::ApiResponse;

/// GET /api/plans
pub async fn list_plans(State(state): State<AppState>) -> Json<ApiResponse<Vec<PendingPlan>>> {
    Json(ApiResponse::ok(state.plan_book.read().await.plans.clone()))
}

/// DELETE /api/plans/{id}
pub async fn discard_plan(State(state): State<AppState>, Path(id): Path<String>) -> Json<ApiResponse<String>> {
    match state.plan_book.write().await.discard(&id) {
        Some(_) => Json(ApiResponse::ok_msg(id, "条件付きプランを破棄しました")),
        None => Json(ApiResponse::err(format!("plan {id} not found"))),
    }
}

/// GET /api/guard/config
pub async fn guard_config(State(state): State<AppState>) -> Json<ApiResponse<GuardConfig>> {
    Json(ApiResponse::ok(state.guard_config.read().await.clone()))
}
