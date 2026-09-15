use axum::{
    extract::{Path, State},
    response::Json,
};
use serde::Serialize;

use crate::replay::report::{build_report, Report};
use crate::server::state::AppState;
use crate::server::types::ApiResponse;
use crate::storage::{Coverage, Db, ReplayDecisionRow, ReplayRunRow};

#[derive(Debug, Serialize)]
pub struct RunDetail {
    pub run: ReplayRunRow,
    pub report: Report,
    pub decisions: Vec<ReplayDecisionRow>,
}

/// GET /api/replay/runs
pub async fn list_runs(State(state): State<AppState>) -> Json<ApiResponse<Vec<ReplayRunRow>>> {
    let path = state.db_path.clone();
    let res = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<ReplayRunRow>> {
        Db::open(path)?.list_runs()
    })
    .await;
    match res {
        Ok(Ok(rows)) => Json(ApiResponse::ok(rows)),
        Ok(Err(e)) => Json(ApiResponse::err(format!("{e:#}"))),
        Err(e) => Json(ApiResponse::err(format!("{e}"))),
    }
}

/// GET /api/replay/runs/{id}
pub async fn run_detail(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Json<ApiResponse<RunDetail>> {
    let path = state.db_path.clone();
    let res = tokio::task::spawn_blocking(move || -> anyhow::Result<Option<RunDetail>> {
        let db = Db::open(path)?;
        let Some(run) = db.get_run(id)? else {
            return Ok(None);
        };
        let decisions = db.decisions(id)?;
        let report = build_report(id, &decisions);
        Ok(Some(RunDetail { run, report, decisions }))
    })
    .await;
    match res {
        Ok(Ok(Some(d))) => Json(ApiResponse::ok(d)),
        Ok(Ok(None)) => Json(ApiResponse::err(format!("run {id} not found"))),
        Ok(Err(e)) => Json(ApiResponse::err(format!("{e:#}"))),
        Err(e) => Json(ApiResponse::err(format!("{e}"))),
    }
}

/// GET /api/replay/coverage
pub async fn coverage(State(state): State<AppState>) -> Json<ApiResponse<Vec<Coverage>>> {
    let path = state.db_path.clone();
    let res = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<Coverage>> {
        Db::open(path)?.coverage()
    })
    .await;
    match res {
        Ok(Ok(rows)) => Json(ApiResponse::ok(rows)),
        Ok(Err(e)) => Json(ApiResponse::err(format!("{e:#}"))),
        Err(e) => Json(ApiResponse::err(format!("{e}"))),
    }
}
