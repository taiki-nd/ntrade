use axum::{extract::State, response::Json};
use serde::Serialize;

use crate::live::DeploymentStatus;
use crate::server::state::AppState;
use crate::server::types::ApiResponse;
use crate::settings::Deployment;
use crate::strategy::{self, StrategyInfo};

/// GET /api/strategies … 使える戦略とパラメータの定義
pub async fn list_strategies() -> Json<ApiResponse<Vec<StrategyInfo>>> {
    Json(ApiResponse::ok(strategy::catalog()))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentView {
    #[serde(flatten)]
    pub deployment: Deployment,
    /// 管理している建玉・予約注文の数
    pub open_positions: usize,
    pub working_orders: usize,
    pub status: Option<DeploymentStatus>,
}

/// GET /api/deployments … 設定したデプロイと直近の実行状況
pub async fn list_deployments(State(state): State<AppState>) -> Json<ApiResponse<Vec<DeploymentView>>> {
    let deployments = state.settings.read().await.deployments.clone();
    let positions = state.positions.read().await.clone();
    let orders = state.working_orders.read().await.clone();
    let status = state.deployment_status.read().await.clone();
    let views = deployments
        .into_iter()
        .map(|d| DeploymentView {
            open_positions: positions.iter().filter(|p| p.strategy.as_deref() == Some(d.id.as_str())).count(),
            working_orders: orders.iter().filter(|o| o.strategy.as_deref() == Some(d.id.as_str())).count(),
            status: status.get(&d.id).cloned(),
            deployment: d,
        })
        .collect();
    Json(ApiResponse::ok(views))
}
