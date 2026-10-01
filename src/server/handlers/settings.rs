//! 取引設定（対象ペア・足確定後の待ち秒数・事後ガード）の参照と更新。
//! 保存先は SQLite の app_settings。保存すると次の判断サイクルから反映される（再起動不要）。

use axum::{extract::State, response::Json};
use tracing::warn;

use crate::server::state::AppState;
use crate::server::types::ApiResponse;
use crate::settings::TradingSettings;

/// GET /api/settings
pub async fn get_settings(State(state): State<AppState>) -> Json<ApiResponse<TradingSettings>> {
    Json(ApiResponse::ok(state.settings.read().await.clone()))
}

/// PUT /api/settings
pub async fn update_settings(
    State(state): State<AppState>,
    Json(next): Json<TradingSettings>,
) -> Json<ApiResponse<TradingSettings>> {
    match state.save_settings(next).await {
        Ok(saved) => Json(ApiResponse::ok_msg(saved, "設定を保存しました。次の判断サイクルから反映されます")),
        Err(e) => {
            warn!("settings update rejected: {e:#}");
            Json(ApiResponse::err(format!("{e:#}")))
        }
    }
}
