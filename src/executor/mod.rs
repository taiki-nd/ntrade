//! 執行: 戦略の注文をブローカーに出す。
//!
//! 予約注文の約定と SL/TP の執行はブローカーが行う（ntrade は約定待ちの間、価格を見張らない）。
//! 約定・取消・期限切れ・決済は照合（`AppState::reconcile_positions`）で取り込む。

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;

use crate::ctrader::PendingKind;
use crate::strategy::Side;

type BoxFut<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

/// 新規の成行注文
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MarketOrderRequest {
    pub pair: String,
    pub side: Side,
    pub volume_lots: f64,
    /// 発注直前の気配（買いは ask・売りは bid。無ければ仲値）。SL/TP の相対距離の基準
    pub reference_price: f64,
    pub stop_loss: f64,
    pub take_profit: Option<f64>,
    /// シグナル ID とデプロイ ID（`signal_comment`）。建玉に引き継がれ、照合で辿るのに使う
    pub comment: String,
}

/// 成行注文の約定
#[derive(Debug, Clone, PartialEq)]
pub struct MarketFill {
    pub position_id: String,
    pub price: f64,
}

/// 新規の予約注文
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PendingOrderRequest {
    pub pair: String,
    pub side: Side,
    pub kind: PendingKind,
    pub volume_lots: f64,
    pub entry_price: f64,
    pub stop_loss: f64,
    pub take_profit: Option<f64>,
    pub expires_at: DateTime<Utc>,
    pub comment: String,
}

/// 発注先の抽象。本番は cTrader、未接続時はエラー
pub trait OrderSink: Send + Sync {
    /// 成行で建て、約定後に SL/TP を絶対価格に付け直す
    fn place_market<'a>(&'a self, req: &'a MarketOrderRequest) -> BoxFut<'a, MarketFill>;
    /// 予約注文を置き、ブローカーの注文 ID を返す
    fn place_pending<'a>(&'a self, req: &'a PendingOrderRequest) -> BoxFut<'a, String>;
    fn cancel_pending<'a>(&'a self, order_id: &'a str) -> BoxFut<'a, ()>;
    /// 建玉の SL/TP を絶対価格で付け替える（None の項目は今の値のまま）
    fn amend_position<'a>(&'a self, position_id: &'a str, stop_loss: Option<f64>, take_profit: Option<f64>) -> BoxFut<'a, ()>;
}

/// 発注の comment。`sig-<ミリ秒>@<デプロイ ID>` の形で、ブローカー側の注文・建玉からシグナルと戦略へ辿れるようにする
pub fn signal_comment(signal_id: &str, deployment: &str) -> String {
    format!("{signal_id}@{deployment}")
}

/// comment からシグナル ID とデプロイ ID を取り出す（ntrade が付けたものでなければ None）
pub fn parse_signal_comment(comment: Option<&str>) -> Option<(String, Option<String>)> {
    let c = comment?.trim();
    if !c.starts_with("sig-") {
        return None;
    }
    Some(match c.split_once('@') {
        Some((sig, dep)) => (sig.to_string(), Some(dep.to_string()).filter(|d| !d.is_empty())),
        None => (c.to_string(), None),
    })
}

/// cTrader 未接続時のプレースホルダー。未接続時の発注はエラーとする。
pub struct DisconnectedOrderSink;

fn disconnected<'a, T: Send + 'a>() -> BoxFut<'a, T> {
    Box::pin(async { anyhow::bail!("cTrader is not connected; cannot place order") })
}

impl OrderSink for DisconnectedOrderSink {
    fn place_market<'a>(&'a self, _req: &'a MarketOrderRequest) -> BoxFut<'a, MarketFill> {
        disconnected()
    }
    fn place_pending<'a>(&'a self, _req: &'a PendingOrderRequest) -> BoxFut<'a, String> {
        disconnected()
    }
    fn cancel_pending<'a>(&'a self, _order_id: &'a str) -> BoxFut<'a, ()> {
        disconnected()
    }
    fn amend_position<'a>(&'a self, _id: &'a str, _sl: Option<f64>, _tp: Option<f64>) -> BoxFut<'a, ()> {
        disconnected()
    }
}

/// cTrader への実発注
pub struct CTraderOrderSink {
    service: std::sync::Arc<crate::ctrader::CTraderService>,
}

impl CTraderOrderSink {
    pub fn new(service: std::sync::Arc<crate::ctrader::CTraderService>) -> Self {
        Self { service }
    }
}

fn broker_id(id: &str) -> Result<i64> {
    id.parse().map_err(|_| anyhow::anyhow!("{id} is not a cTrader id"))
}

impl OrderSink for CTraderOrderSink {
    fn place_market<'a>(&'a self, req: &'a MarketOrderRequest) -> BoxFut<'a, MarketFill> {
        Box::pin(async move {
            let volume = self.service.lots_to_volume(&req.pair, req.volume_lots).await?;
            let (position_id, price) = self
                .service
                .place_market_order(
                    &req.pair,
                    req.side == Side::Buy,
                    volume,
                    req.reference_price,
                    req.stop_loss,
                    req.take_profit,
                    &req.comment,
                )
                .await?;
            // 相対距離で付けた SL/TP を、戦略が決めた絶対価格に直す。失敗しても相対の SL/TP は残っている
            if let Err(e) = self.service.amend_position_sltp(position_id, Some(req.stop_loss), req.take_profit).await {
                tracing::warn!(position_id, "failed to set absolute SL/TP after market fill (relative ones remain): {e:#}");
            }
            Ok(MarketFill { position_id: position_id.to_string(), price })
        })
    }

    fn place_pending<'a>(&'a self, req: &'a PendingOrderRequest) -> BoxFut<'a, String> {
        Box::pin(async move {
            let volume = self.service.lots_to_volume(&req.pair, req.volume_lots).await?;
            let id = self
                .service
                .place_pending_order(
                    &req.pair,
                    req.side == Side::Buy,
                    req.kind,
                    volume,
                    req.entry_price,
                    req.stop_loss,
                    req.take_profit,
                    req.expires_at,
                    &req.comment,
                )
                .await?;
            Ok(id.to_string())
        })
    }

    fn cancel_pending<'a>(&'a self, order_id: &'a str) -> BoxFut<'a, ()> {
        Box::pin(async move { self.service.cancel_pending_order(broker_id(order_id)?).await })
    }

    fn amend_position<'a>(&'a self, position_id: &'a str, stop_loss: Option<f64>, take_profit: Option<f64>) -> BoxFut<'a, ()> {
        Box::pin(async move {
            self.service.amend_position_sltp(broker_id(position_id)?, stop_loss, take_profit).await?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_comment_roundtrip() {
        let c = signal_comment("sig-1790000000000", "ema-usdjpy");
        assert_eq!(parse_signal_comment(Some(&c)), Some(("sig-1790000000000".into(), Some("ema-usdjpy".into()))));
        assert_eq!(parse_signal_comment(Some("sig-1")), Some(("sig-1".into(), None)));
        // LLM 時代の判断 ID や他ツールの comment は対象外
        assert_eq!(parse_signal_comment(Some("cot-1")), None);
        assert_eq!(parse_signal_comment(None), None);
    }
}
