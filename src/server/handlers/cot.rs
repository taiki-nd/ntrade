use axum::{extract::State, response::Json};

use crate::server::state::AppState;
use crate::server::types::{ApiResponse, CoTLog};

/// GET /api/cot
/// LLM思考プロセス（CoT）ログ一覧を返却
pub async fn get_cot_logs(State(state): State<AppState>) -> Json<ApiResponse<Vec<CoTLog>>> {
    let lock = state.cot_logs.read().await;
    Json(ApiResponse::ok(lock.clone()))
}
