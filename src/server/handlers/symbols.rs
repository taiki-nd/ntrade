//! ブローカーの銘柄一覧と銘柄仕様。対象銘柄を画面で選ぶときに使う。

use axum::{
    extract::{Query, State},
    response::Json,
};
use serde::Deserialize;

use crate::ctrader::SymbolCatalogEntry;
use crate::instrument::InstrumentSpec;
use crate::server::handlers::chart::PairQuery;
use crate::server::state::AppState;
use crate::server::types::ApiResponse;

#[derive(Debug, Deserialize)]
pub struct SymbolsQuery {
    /// 銘柄名・説明・資産クラスの部分一致（大文字小文字は無視）
    pub q: Option<String>,
}

/// GET /api/symbols?q=xau
pub async fn list_symbols(State(state): State<AppState>, Query(q): Query<SymbolsQuery>) -> Json<ApiResponse<Vec<SymbolCatalogEntry>>> {
    let Some(ctrader) = state.ctrader_service.read().await.clone() else {
        return Json(ApiResponse::err("cTrader is not connected"));
    };
    let needle = q.q.map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty());
    let list = ctrader
        .catalog()
        .await
        .into_iter()
        .filter(|s| {
            needle.as_ref().is_none_or(|n| {
                [Some(&s.symbol_name), s.description.as_ref(), s.asset_class.as_ref(), s.category.as_ref()]
                    .into_iter()
                    .flatten()
                    .any(|v| v.to_lowercase().contains(n))
            })
        })
        .collect();
    Json(ApiResponse::ok(list))
}

/// GET /api/symbols/spec?pair=XAUUSD_z
pub async fn symbol_spec(State(state): State<AppState>, Query(q): Query<PairQuery>) -> Json<ApiResponse<InstrumentSpec>> {
    let pair = state.resolve_pair(q.pair.as_deref()).await;
    let Some(ctrader) = state.ctrader_service.read().await.clone() else {
        return Json(ApiResponse::err("cTrader is not connected"));
    };
    match ctrader.symbol_spec(&pair).await {
        Ok(spec) => Json(ApiResponse::ok(spec)),
        Err(e) => Json(ApiResponse::err(format!("{e:#}"))),
    }
}
