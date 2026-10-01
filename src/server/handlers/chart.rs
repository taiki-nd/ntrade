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
    AccountState, MarketSnapshot, OpenPositionSummary, RecentDecision, SnapshotBundle, SnapshotInput, SnapshotPipeline,
};

#[derive(Debug, Deserialize)]
pub struct ChartQuery {
    /// 4H / 1H / 15M / 5M（省略時は 5M）
    pub tf: Option<String>,
    /// 通貨ペア（省略時は設定の先頭ペア）
    pub pair: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PairQuery {
    /// 通貨ペア（省略時は設定の先頭ペア）
    pub pair: Option<String>,
}

/// 指定ペアの直近 Snapshot。まだ無ければその場で生成する
async fn snapshot_or_generate(state: &AppState, pair: &str) -> anyhow::Result<SnapshotBundle> {
    if let Some(b) = state.latest_snapshots.read().await.get(pair) {
        return Ok(b.clone());
    }
    info!(pair, "No snapshot yet, generating on demand...");
    generate_snapshot_internal(state, pair).await
}

/// GET /api/chart/latest?tf=5M&pair=USDJPY
/// 直近 Snapshot の時間足別チャートPNGを配信
pub async fn get_latest_chart(
    State(state): State<AppState>,
    Query(q): Query<ChartQuery>,
) -> Response {
    let tf = q.tf.unwrap_or_else(|| "5M".to_string());
    let pair = state.resolve_pair(q.pair.as_deref()).await;

    let path = match snapshot_or_generate(&state, &pair).await {
        Ok(b) => b.charts.by_timeframe(&tf).map(|p| p.to_path_buf()),
        Err(e) => {
            warn!(pair, "On-demand snapshot generation failed: {e:#}");
            None
        }
    };

    let Some(path) = path else {
        return (
            StatusCode::NOT_FOUND,
            [("Content-Type", "text/plain")],
            format!("No chart for {pair} timeframe {tf}"),
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

/// POST /api/chart/generate?pair=USDJPY
/// Snapshot（画像4枚 + 客観的事実）を再生成
pub async fn generate_chart(State(state): State<AppState>, Query(q): Query<PairQuery>) -> Json<ApiResponse<String>> {
    let pair = state.resolve_pair(q.pair.as_deref()).await;
    info!("Triggering snapshot regeneration for {pair}...");
    match generate_snapshot_internal(&state, &pair).await {
        Ok(b) => Json(ApiResponse::ok_msg(
            b.charts.dir.to_string_lossy().to_string(),
            format!("{pair} の Snapshot（4H/1H/15M/5M 画像 + 事実JSON）を再生成しました"),
        )),
        Err(e) => Json(ApiResponse::err(format!("Failed to generate snapshot: {:?}", e))),
    }
}

/// GET /api/snapshot/latest?pair=USDJPY
/// 直近 Snapshot の客観的事実 JSON
pub async fn get_latest_snapshot(
    State(state): State<AppState>,
    Query(q): Query<PairQuery>,
) -> Json<ApiResponse<MarketSnapshot>> {
    let pair = state.resolve_pair(q.pair.as_deref()).await;
    match snapshot_or_generate(&state, &pair).await {
        Ok(b) => Json(ApiResponse::ok(b.snapshot)),
        Err(e) => Json(ApiResponse::err(format!("Failed to generate snapshot: {:?}", e))),
    }
}

/// 内部用: バー取得 → Snapshot 生成 → ペア別の直近 Snapshot を更新。生成した Snapshot を返す。
pub async fn generate_snapshot_internal(state: &AppState, pair: &str) -> anyhow::Result<SnapshotBundle> {
    let ctrader_opt = {
        let lock = state.ctrader_service.read().await;
        lock.clone()
    };

    // 実データが取れないときは作らない（模擬データのチャートを LLM の判断や画面に使わないため）
    let ctrader = ctrader_opt.ok_or_else(|| anyhow::anyhow!("cTrader is not connected; snapshot needs live bars"))?;
    info!(pair, "Fetching real trendbars from connected cTrader...");
    let b4h = ctrader.get_trendbars(pair, BarPeriod::H4, 200).await.context("failed to fetch 4H bars")?;
    let b1h = ctrader.get_trendbars(pair, BarPeriod::H1, 200).await.context("failed to fetch 1H bars")?;
    let b15m = ctrader.get_trendbars(pair, BarPeriod::M15, 200).await.context("failed to fetch 15M bars")?;
    let b5m = ctrader.get_trendbars(pair, BarPeriod::M5, 200).await.context("failed to fetch 5M bars")?;

    let spread_pips = state.metrics.read().await.spreads.get(pair).copied().unwrap_or_default();

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
                    // LLM に見せるのは自分が決めた損切りライン。終値判定の建玉ではハードSLではなくそちら
                    stop_loss: Some(p.close_stop.unwrap_or(p.stop_loss)),
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
            // 成立したプランは判断サイクルがプラン評価の後で差し込む
            triggered_plan: None,
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

    info!(pair, "Snapshot generated at {}", bundle.charts.dir.display());
    state.latest_snapshots.write().await.insert(pair.to_string(), bundle.clone());
    Ok(bundle)
}
