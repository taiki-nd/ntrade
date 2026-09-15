use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Json, Response},
};
use chrono::{Duration, Utc};
use std::fs;
use std::path::PathBuf;
use tracing::{info, warn};

use crate::chart::ChartPlotterConfig;
use crate::ctrader::{BarPeriod, CandleBar};
use crate::server::state::AppState;
use crate::server::types::ApiResponse;
use crate::strategy::PriceActionPipeline;

/// GET /api/chart/latest
/// 最新の4分割チャートPNG画像をバイナリストリームで配信
pub async fn get_latest_chart(State(state): State<AppState>) -> Response {
    let path = {
        let lock = state.chart_image_path.read().await;
        lock.clone()
    };

    // チャート画像が存在しない場合はオンデマンド生成
    if !path.exists() {
        info!("Chart image does not exist at {:?}, generating on demand...", path);
        let _ = generate_chart_internal(&state, "USDJPY").await;
    }

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
/// 手動または定期トリガーによるチャート再描画
pub async fn generate_chart(State(state): State<AppState>) -> Json<ApiResponse<String>> {
    info!("Triggering manual chart regeneration for USDJPY...");
    match generate_chart_internal(&state, "USDJPY").await {
        Ok(path_str) => Json(ApiResponse::ok_msg(
            path_str,
            "4分割チャートPNGを新規生成しました",
        )),
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            message: Some(format!("Failed to generate chart: {:?}", e)),
        }),
    }
}

/// 内部用チャート生成ロジック
pub async fn generate_chart_internal(state: &AppState, pair: &str) -> anyhow::Result<String> {
    let ctrader_opt = {
        let lock = state.ctrader_service.read().await;
        lock.clone()
    };

    let (b4h, b1h, b15m, b5m) = if let Some(ctrader) = ctrader_opt {
        info!("Fetching real trendbars from connected cTrader...");
        match (
            ctrader.get_trendbars(pair, BarPeriod::H4, 45).await,
            ctrader.get_trendbars(pair, BarPeriod::H1, 45).await,
            ctrader.get_trendbars(pair, BarPeriod::M15, 45).await,
            ctrader.get_trendbars(pair, BarPeriod::M5, 45).await,
        ) {
            (Ok(b4), Ok(b1), Ok(b15), Ok(b5)) => (b4, b1, b15, b5),
            _ => generate_mock_bars(),
        }
    } else {
        generate_mock_bars()
    };

    let output_dir = PathBuf::from("charts");
    fs::create_dir_all(&output_dir)?;

    let pipeline = PriceActionPipeline::with_plotter_config(
        &output_dir,
        ChartPlotterConfig {
            width: 1600,
            height: 1200,
            bars_to_display: 45,
            dark_mode: true,
        },
    );

    let bundle = pipeline.process(pair, &b4h, &b1h, &b15m, &b5m, 0.2)?;
    let path_str = bundle.chart_path.to_string_lossy().to_string();

    {
        let mut lock = state.chart_image_path.write().await;
        *lock = bundle.chart_path;
    }

    info!("Generated chart saved at {}", path_str);
    Ok(path_str)
}

fn generate_mock_bars() -> (Vec<CandleBar>, Vec<CandleBar>, Vec<CandleBar>, Vec<CandleBar>) {
    let now = Utc::now();
    let mut bars_4h = Vec::new();
    let mut p = 152.50;
    for i in 0..35 {
        let open = p;
        let close = open + 0.08 + ((i as f64 * 0.3).sin() * 0.15);
        let high = open.max(close) + 0.20;
        let low = open.min(close) - 0.12;
        p = close;
        bars_4h.push(CandleBar {
            timestamp: now - Duration::hours((35 - i) * 4),
            open,
            high,
            low,
            close,
            volume: 5000,
        });
    }

    let mut bars_1h = Vec::new();
    let mut p = 153.80;
    for i in 0..40 {
        let open = p;
        let diff = if i > 30 { -0.05 } else { 0.04 };
        let close = open + diff + ((i as f64 * 0.2).cos() * 0.08);
        let high = open.max(close) + 0.10;
        let low = open.min(close) - 0.08;
        p = close;
        bars_1h.push(CandleBar {
            timestamp: now - Duration::hours(40 - i),
            open,
            high,
            low,
            close,
            volume: 1500,
        });
    }

    let mut bars_15m = Vec::new();
    let mut p = 154.30;
    for i in 0..45 {
        let open = p;
        let diff = if i > 35 { 0.02 } else { -0.02 };
        let close = open + diff + ((i as f64 * 0.15).sin() * 0.05);
        let high = open.max(close) + 0.06;
        let low = open.min(close) - 0.06;
        p = close;
        bars_15m.push(CandleBar {
            timestamp: now - Duration::minutes((45 - i) * 15),
            open,
            high,
            low,
            close,
            volume: 400,
        });
    }

    let mut bars_5m = Vec::new();
    let mut p: f64 = 154.25;
    for i in 0..44 {
        let open: f64 = p;
        let diff: f64 = if i > 38 { -0.02 } else { 0.01 };
        let close: f64 = open + diff;
        let high: f64 = open.max(close) + 0.03;
        let low: f64 = open.min(close) - 0.03;
        p = close;
        bars_5m.push(CandleBar {
            timestamp: now - Duration::minutes((45 - i) * 5),
            open,
            high,
            low,
            close,
            volume: 120,
        });
    }

    bars_5m.push(CandleBar {
        timestamp: now,
        open: 154.200,
        high: 154.240,
        low: 154.080,
        close: 154.220,
        volume: 380,
    });

    (bars_4h, bars_1h, bars_15m, bars_5m)
}
