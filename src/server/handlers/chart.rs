//! 監視中の銘柄のチャート画像（ローソク足・EMA20/50・建玉と予約注文の価格）。

use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use std::path::PathBuf;
use tracing::warn;

use crate::chart::{ChartPlotter, PriceLevel, TimeframeChart};
use crate::ctrader::BarPeriod;
use crate::server::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ChartQuery {
    /// 1M / 5M / 15M / 1H / 4H / 1D（省略時は 5M）
    pub tf: Option<String>,
    /// 通貨ペア（省略時は設定の先頭ペア）
    pub pair: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PairQuery {
    /// 通貨ペア（省略時は設定の先頭ペア）
    pub pair: Option<String>,
}

fn not_found(msg: String) -> Response {
    (StatusCode::NOT_FOUND, [("Content-Type", "text/plain")], msg).into_response()
}

/// GET /api/chart/latest?tf=5M&pair=USDJPY_z
/// cTrader の確定足から描いた PNG を返す
pub async fn get_latest_chart(State(state): State<AppState>, Query(q): Query<ChartQuery>) -> Response {
    let tf = q.tf.unwrap_or_else(|| "5M".to_string());
    let Some(period) = BarPeriod::parse(&tf) else { return not_found(format!("unknown timeframe {tf}")) };
    let pair = state.resolve_pair(q.pair.as_deref()).await;
    match render(&state, &pair, period).await {
        Ok(bytes) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "image/png"), (header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")],
            bytes,
        )
            .into_response(),
        Err(e) => {
            warn!(pair, tf, "chart rendering failed: {e:#}");
            not_found(format!("No chart for {pair} {tf}: {e:#}"))
        }
    }
}

async fn render(state: &AppState, pair: &str, period: BarPeriod) -> anyhow::Result<Vec<u8>> {
    let ctrader = state.ctrader_service.read().await.clone().ok_or_else(|| anyhow::anyhow!("cTrader is not connected"))?;
    let bars = ctrader.get_trendbars(pair, period, 160).await?;
    let mut levels: Vec<PriceLevel> = Vec::new();
    for p in state.positions.read().await.iter().filter(|p| p.symbol.eq_ignore_ascii_case(pair)) {
        levels.push(PriceLevel { price: p.entry_price, tag: format!("{} entry", p.side) });
        if p.stop_loss > 0.0 {
            levels.push(PriceLevel { price: p.stop_loss, tag: "SL".into() });
        }
        if p.take_profit > 0.0 {
            levels.push(PriceLevel { price: p.take_profit, tag: "TP".into() });
        }
    }
    for o in state.working_orders.read().await.iter().filter(|o| o.symbol.eq_ignore_ascii_case(pair)) {
        levels.push(PriceLevel { price: o.entry_price, tag: format!("{} {:?}", o.side, o.kind) });
    }
    let current = ctrader.quote(pair, chrono::Duration::minutes(5)).await.map(|q| (q.bid + q.ask) / 2.0);
    let path = PathBuf::from("charts/live").join(format!("{}_{}.png", pair.to_uppercase(), period.as_str()));
    let (pair, tf) = (pair.to_string(), period.as_str().to_string());
    tokio::task::spawn_blocking(move || {
        let chart = TimeframeChart { pair: &pair, timeframe: &tf, bars: &bars, levels: &levels, current_price: current };
        ChartPlotter::with_default().plot_to_file(&chart, &path)?;
        Ok(std::fs::read(&path)?)
    })
    .await?
}
