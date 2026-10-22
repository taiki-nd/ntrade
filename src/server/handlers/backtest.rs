//! バックテストの実行と結果の参照（CLI の `bin/backtest` と同じ処理）。

use axum::{
    extract::{Path, Query, State},
    response::Json,
};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::backtest::config::{self, BacktestFileConfig, RunRequest, DEFAULT_BACKTEST_CONFIG_PATH};
use crate::backtest::{self, BacktestTrade, EquityPoint};
use crate::server::jobs::{JobInfo, JobRequest};
use crate::server::state::AppState;
use crate::server::types::ApiResponse;
use crate::storage::{BacktestRunRow, Coverage};

#[derive(Debug, Deserialize)]
pub struct RunsQuery {
    pub limit: Option<usize>,
}

/// GET /api/backtest/runs
pub async fn list_runs(State(state): State<AppState>, Query(q): Query<RunsQuery>) -> Json<ApiResponse<Vec<BacktestRunRow>>> {
    let limit = q.limit.unwrap_or(100).clamp(1, 1000);
    match state.with_db(move |db| db.backtest_runs(limit)).await {
        Ok(rows) => Json(ApiResponse::ok(rows)),
        Err(e) => Json(ApiResponse::err(format!("{e:#}"))),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunDetail {
    pub run: BacktestRunRow,
    pub trades: Vec<BacktestTrade>,
    pub equity: Vec<EquityPoint>,
}

/// GET /api/backtest/runs/{id}
pub async fn run_detail(State(state): State<AppState>, Path(id): Path<i64>) -> Json<ApiResponse<RunDetail>> {
    let res = state
        .with_db(move |db| {
            let Some(run) = db.backtest_run(id)? else { return Ok(None) };
            Ok(Some(RunDetail { trades: db.backtest_trades(id)?, equity: db.backtest_equity(id)?, run }))
        })
        .await;
    match res {
        Ok(Some(d)) => Json(ApiResponse::ok(d)),
        Ok(None) => Json(ApiResponse::err(format!("run {id} not found"))),
        Err(e) => Json(ApiResponse::err(format!("{e:#}"))),
    }
}

/// DELETE /api/backtest/runs/{id}
pub async fn delete_run(State(state): State<AppState>, Path(id): Path<i64>) -> Json<ApiResponse<()>> {
    match state.with_db(move |db| db.delete_backtest_run(id)).await {
        Ok(true) => Json(ApiResponse::ok_msg((), "バックテスト結果を削除しました")),
        Ok(false) => Json(ApiResponse::err(format!("run {id} not found"))),
        Err(e) => Json(ApiResponse::err(format!("{e:#}"))),
    }
}

/// GET /api/backtest/coverage … DB にある足の範囲
pub async fn coverage(State(state): State<AppState>) -> Json<ApiResponse<Vec<Coverage>>> {
    match state.with_db(|db| db.coverage()).await {
        Ok(rows) => Json(ApiResponse::ok(rows)),
        Err(e) => Json(ApiResponse::err(format!("{e:#}"))),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunCreated {
    pub id: i64,
    pub metrics: backtest::Metrics,
}

/// POST /api/backtest/run … 単発のバックテストを実行して保存する
pub async fn run(State(state): State<AppState>, Json(req): Json<RunRequest>) -> Json<ApiResponse<RunCreated>> {
    info!(pair = %req.pair, strategy = %req.strategy.label(), from = %req.from, to = %req.to, "backtest requested from dashboard");
    let res = state
        .with_db(move |db| {
            let file = BacktestFileConfig::load_or_default(DEFAULT_BACKTEST_CONFIG_PATH)?;
            let (data, cfg) = config::prepare(db, &file, &req, 0)?;
            let result = backtest::run(&data, &cfg)?;
            let id = config::save_single(db, &result, &req.label)?;
            Ok(RunCreated { id, metrics: result.metrics })
        })
        .await;
    match res {
        Ok(r) => Json(ApiResponse::ok_msg(r, "バックテストが完了しました")),
        Err(e) => Json(ApiResponse::err(format!("{e:#}"))),
    }
}

/// GET /api/backtest/jobs … ジョブの一覧（新しい順）
pub async fn list_jobs(State(state): State<AppState>) -> Json<ApiResponse<Vec<JobInfo>>> {
    Json(ApiResponse::ok(state.jobs.list()))
}

/// POST /api/backtest/jobs … 過去データの取得・グリッドサーチ・ウォークフォワードを順番待ちに入れる
pub async fn submit_job(State(state): State<AppState>, Json(req): Json<JobRequest>) -> Json<ApiResponse<JobInfo>> {
    // 名前や組み合わせ数の誤りは、順番を待たせずにここで返す
    let check = match &req {
        JobRequest::Fetch(r) if r.pairs.is_empty() => Err(anyhow::anyhow!("銘柄を 1 つ以上指定してください")),
        JobRequest::Fetch(_) => Ok(()),
        JobRequest::Sweep(g) | JobRequest::WalkForward { grid: g, .. } => config::grid_combos(&g.run.strategy, &g.grid).map(|_| ()),
    };
    if let Err(e) = check {
        return Json(ApiResponse::err(format!("{e:#}")));
    }
    let job = state.jobs.submit(state.clone(), req);
    info!(job = job.id, title = %job.title, "backtest job queued");
    Json(ApiResponse::ok_msg(job, "ジョブを受け付けました"))
}

/// POST /api/backtest/jobs/{id}/cancel
pub async fn cancel_job(State(state): State<AppState>, Path(id): Path<u64>) -> Json<ApiResponse<()>> {
    if state.jobs.cancel(id) {
        Json(ApiResponse::ok_msg((), "中止を指示しました"))
    } else {
        Json(ApiResponse::err(format!("job {id} is not running")))
    }
}
