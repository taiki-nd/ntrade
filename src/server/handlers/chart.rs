use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Json, Response},
};
use anyhow::Context;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use tracing::{info, warn};

use crate::ctrader::BarPeriod;
use crate::server::state::AppState;
use crate::server::types::ApiResponse;
use crate::snapshot::{
    AccountState, MarketSnapshot, OpenPositionSummary, RecentDecision, SnapshotInput, SnapshotPipeline,
};

#[derive(Debug, Deserialize)]
pub struct ChartQuery {
    /// 4H / 1H / 15M / 5M（省略時は 5M）
    pub tf: Option<String>,
}

/// GET /api/chart/latest?tf=5M
/// 直近 Snapshot の時間足別チャートPNGを配信
pub async fn get_latest_chart(
    State(state): State<AppState>,
    Query(q): Query<ChartQuery>,
) -> Response {
    let tf = q.tf.unwrap_or_else(|| "5M".to_string());

    if state.latest_snapshot.read().await.is_none() {
        info!("No snapshot yet, generating on demand...");
        if let Err(e) = generate_snapshot_internal(&state, "USDJPY").await {
            warn!("On-demand snapshot generation failed: {:?}", e);
        }
    }

    let path = {
        let lock = state.latest_snapshot.read().await;
        lock.as_ref()
            .and_then(|b| b.charts.by_timeframe(&tf).map(|p| p.to_path_buf()))
    };

    let Some(path) = path else {
        return (
            StatusCode::NOT_FOUND,
            [("Content-Type", "text/plain")],
            format!("No chart for timeframe {tf}"),
        )
            .into_response();
    };

    match fs::read(&path) {
        Ok(bytes) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "image/png"),
                (header::CACHE_CONTROL, "no-cache, no-store, must-revalidate"),
            ],
            bytes,
        )
            .into_response(),
        Err(e) => {
            warn!("Failed to read chart image at {:?}: {:?}", path, e);
            (
                StatusCode::NOT_FOUND,
                [("Content-Type", "text/plain")],
                format!("Chart image not found: {}", e),
            )
                .into_response()
        }
    }
}

/// POST /api/chart/generate
/// Snapshot（画像4枚 + 客観的事実）を再生成
pub async fn generate_chart(State(state): State<AppState>) -> Json<ApiResponse<String>> {
    info!("Triggering snapshot regeneration for USDJPY...");
    match generate_snapshot_internal(&state, "USDJPY").await {
        Ok(dir) => Json(ApiResponse::ok_msg(dir, "Snapshot（4H/1H/15M/5M 画像 + 事実JSON）を再生成しました")),
        Err(e) => Json(ApiResponse::err(format!("Failed to generate snapshot: {:?}", e))),
    }
}

/// GET /api/snapshot/latest
/// 直近 Snapshot の客観的事実 JSON
pub async fn get_latest_snapshot(
    State(state): State<AppState>,
) -> Json<ApiResponse<MarketSnapshot>> {
    if state.latest_snapshot.read().await.is_none() {
        if let Err(e) = generate_snapshot_internal(&state, "USDJPY").await {
            return Json(ApiResponse::err(format!("Failed to generate snapshot: {:?}", e)));
        }
    }
    let lock = state.latest_snapshot.read().await;
    match lock.as_ref() {
        Some(b) => Json(ApiResponse::ok(b.snapshot.clone())),
        None => Json(ApiResponse::err("No snapshot available")),
    }
}

/// 内部用: バー取得 → Snapshot 生成 → 状態更新。生成先ディレクトリを返す。
pub async fn generate_snapshot_internal(state: &AppState, pair: &str) -> anyhow::Result<String> {
    let ctrader_opt = {
        let lock = state.ctrader_service.read().await;
        lock.clone()
    };

    // 実データが取れないときは作らない（模擬データのチャートを LLM の判断や画面に使わないため）
    let ctrader = ctrader_opt.ok_or_else(|| anyhow::anyhow!("cTrader is not connected; snapshot needs live bars"))?;
    info!("Fetching real trendbars from connected cTrader...");
    let b4h = ctrader.get_trendbars(pair, BarPeriod::H4, 200).await.context("failed to fetch 4H bars")?;
    let b1h = ctrader.get_trendbars(pair, BarPeriod::H1, 200).await.context("failed to fetch 1H bars")?;
    let b15m = ctrader.get_trendbars(pair, BarPeriod::M15, 200).await.context("failed to fetch 15M bars")?;
    let b5m = ctrader.get_trendbars(pair, BarPeriod::M5, 200).await.context("failed to fetch 5M bars")?;

    let spread_pips = {
        let m = state.metrics.read().await;
        if pair.to_uppercase().contains("JPY") { m.usdjpy_spread } else { m.eurusd_spread }
    };

    // 口座状態: 保有ポジションと直近の判断（フリップフロップ防止のため LLM に渡す）
    let recent_cot = {
        let pair = pair.to_string();
        match state.with_db(move |db| db.cot_logs(Some(&pair), None, 3, 0)).await {
            Ok(page) => page.items,
            Err(e) => {
                warn!("Failed to load recent decisions from SQLite: {e:#}");
                Vec::new()
            }
        }
    };
    let account_state = {
        let positions = state.positions.read().await;
        AccountState {
            open_positions: positions
                .iter()
                .filter(|p| p.symbol.eq_ignore_ascii_case(pair))
                .map(|p| OpenPositionSummary {
                    side: p.side.clone(),
                    entry_price: p.entry_price,
                    stop_loss: Some(p.stop_loss),
                    take_profit: Some(p.take_profit),
                    open_time: p.open_time.clone(),
                })
                .collect(),
            recent_decisions: recent_cot
                .iter()
                .map(|c| RecentDecision {
                    time: c.timestamp.clone(),
                    action: c.action.clone(),
                    summary: c.order_flow.chars().take(120).collect(),
                })
                .collect(),
        }
    };

    let pipeline = SnapshotPipeline::new(PathBuf::from("charts"));
    let bundle = pipeline.build(&SnapshotInput {
        pair,
        bars_4h: &b4h,
        bars_1h: &b1h,
        bars_15m: &b15m,
        bars_5m: &b5m,
        spread_pips,
        current_price: None,
        now: None,
        account_state,
    })?;

    let dir = bundle.charts.dir.to_string_lossy().to_string();
    {
        let mut lock = state.latest_snapshot.write().await;
        *lock = Some(bundle);
    }
    info!("Snapshot generated at {}", dir);
    Ok(dir)
}
