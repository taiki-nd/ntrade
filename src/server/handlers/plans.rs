use axum::{
    extract::{Path, State},
    response::Json,
};

use crate::executor::PendingPlan;
use crate::guard::{GuardConfig, DEFAULT_GUARD_CONFIG_PATH};
use crate::server::state::AppState;
use crate::server::types::{ApiResponse, RuntimeInfo};

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

/// GET /api/runtime
/// エンジンの稼働設定（読み取り専用）
pub async fn runtime_info(State(state): State<AppState>) -> Json<ApiResponse<RuntimeInfo>> {
    let llm = state.llm.config();
    let ctrader = state.ctrader_config.read().await.clone();
    let info = RuntimeInfo {
        order_mode: "live".into(),
        scheduler_enabled: !std::env::var("NTRADE_SCHEDULER").map(|v| v.trim_matches('"') == "off").unwrap_or(false),
        pairs: crate::scheduler::pairs_from_env(),
        bar_delay_secs: std::env::var("NTRADE_BAR_DELAY_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(15),
        paper_balance: 0.0,
        guard_config_path: DEFAULT_GUARD_CONFIG_PATH.to_string(),
        db_path: state.db_path.to_string_lossy().to_string(),
        llm_cli: llm.cli_binary.clone(),
        llm_timeout_secs: llm.timeout_secs,
        env_file_present: std::path::Path::new(".env").exists(),
        ctrader_token_present: ctrader.as_ref().is_some_and(|c| !c.access_token.is_empty()),
        ctrader_refresh_token_present: ctrader.as_ref().is_some_and(|c| c.refresh_token.is_some()),
        ctrader_token_expires_at: ctrader.and_then(|c| c.token_expires_at).map(crate::storage::fmt_ts),
    };
    Json(ApiResponse::ok(info))
}
