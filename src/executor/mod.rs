//! 執行: LLM が決めた予約注文（指値・逆指値）をそのままブローカーに置き、既存の注文・建玉への操作を伝える。
//!
//! 約定の判定はブローカーが行う（ntrade は約定待ちの間、価格を見張らない）。
//! 約定・取消・期限切れは照合（`AppState::reconcile_positions`）で取り込む。

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;

use crate::ctrader::PendingKind;
use crate::strategy::types::Action;

type BoxFut<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

/// 新規の予約注文
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PendingOrderRequest {
    pub pair: String,
    pub action: Action,
    pub kind: PendingKind,
    pub volume_lots: f64,
    pub entry_price: f64,
    /// ブローカーに置く SL（`stop_mode = close` ならハードSL）
    pub stop_loss: f64,
    pub take_profit: f64,
    pub expires_at: DateTime<Utc>,
    /// 判断 ID。建玉に引き継がれ、照合で判断へ辿るのに使う
    pub comment: String,
}

/// 予約注文の変更（None の項目は変えない）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrderAmend {
    pub pair: String,
    pub order_id: String,
    pub kind: PendingKind,
    pub entry_price: Option<f64>,
    pub stop_loss: Option<f64>,
    pub take_profit: Option<f64>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// 発注先の抽象。本番は cTrader、未接続時はエラー
pub trait OrderSink: Send + Sync {
    /// 予約注文を置き、ブローカーの注文 ID を返す
    fn place_pending<'a>(&'a self, req: &'a PendingOrderRequest) -> BoxFut<'a, String>;
    fn amend_pending<'a>(&'a self, req: &'a OrderAmend) -> BoxFut<'a, ()>;
    fn cancel_pending<'a>(&'a self, order_id: &'a str) -> BoxFut<'a, ()>;
    /// 建玉の SL/TP を絶対価格で付け替える（None の項目は今の値のまま）
    fn amend_position<'a>(&'a self, position_id: &'a str, stop_loss: Option<f64>, take_profit: Option<f64>) -> BoxFut<'a, ()>;
}

/// cTrader 未接続時のプレースホルダー。未接続時の発注はエラーとする。
pub struct DisconnectedOrderSink;

fn disconnected<'a, T: Send + 'a>() -> BoxFut<'a, T> {
    Box::pin(async { anyhow::bail!("cTrader is not connected; cannot place order") })
}

impl OrderSink for DisconnectedOrderSink {
    fn place_pending<'a>(&'a self, _req: &'a PendingOrderRequest) -> BoxFut<'a, String> {
        disconnected()
    }
    fn amend_pending<'a>(&'a self, _req: &'a OrderAmend) -> BoxFut<'a, ()> {
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
    fn place_pending<'a>(&'a self, req: &'a PendingOrderRequest) -> BoxFut<'a, String> {
        Box::pin(async move {
            let volume = self.service.lots_to_volume(&req.pair, req.volume_lots).await?;
            let id = self
                .service
                .place_pending_order(
                    &req.pair,
                    req.action == Action::Buy,
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

    fn amend_pending<'a>(&'a self, req: &'a OrderAmend) -> BoxFut<'a, ()> {
        Box::pin(async move {
            self.service
                .amend_pending_order(
                    &req.pair,
                    broker_id(&req.order_id)?,
                    req.kind,
                    req.entry_price,
                    req.stop_loss,
                    req.take_profit,
                    req.expires_at,
                )
                .await
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

/// 損切りの執行方法に応じて、ブローカーに置く SL と、ntrade が終値で判定する損切りラインを決める。
///
/// - `Touch`: LLM の SL をそのままブローカーに置く（終値判定なし）
/// - `Close`: LLM の SL は終値判定のラインにし、ブローカーには `hard_stop_atr` × ATR だけ外側のハードSLを置く。
///   ATR が取れないときはハードSLの幅を決められないので `Touch` と同じにする
///
/// 戻り値は（ブローカーに置く SL, 終値判定ライン）
pub fn protective_stops(action: Action, stop_loss: f64, atr: Option<f64>, cfg: &crate::guard::GuardConfig) -> (f64, Option<f64>) {
    let sign = match action {
        Action::Buy => 1.0,
        Action::Sell => -1.0,
        Action::Hold => return (stop_loss, None),
    };
    match (cfg.stop_mode, atr.filter(|a| *a > 0.0)) {
        (crate::guard::StopMode::Close, Some(atr)) if stop_loss > 0.0 => {
            (stop_loss - sign * cfg.hard_stop_atr * atr, Some(stop_loss))
        }
        _ => (stop_loss, None),
    }
}

/// 終値判定ラインを越えて確定したか（BUY は下抜け、SELL は上抜け）
pub fn close_stop_breached(side: &str, close_stop: f64, bar_close: f64) -> bool {
    if side == "BUY" {
        bar_close < close_stop
    } else {
        bar_close > close_stop
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protective_stops_follow_stop_mode() {
        use crate::guard::{GuardConfig, StopMode};
        let touch = GuardConfig::default();
        assert_eq!(protective_stops(Action::Sell, 157.70, Some(0.08), &touch), (157.70, None));

        let close = GuardConfig { stop_mode: StopMode::Close, hard_stop_atr: 2.0, ..GuardConfig::default() };
        // SELL: ハードSLは上へ 2 × 0.08 = 16 pips
        let (hard, soft) = protective_stops(Action::Sell, 157.70, Some(0.08), &close);
        assert!((hard - 157.86).abs() < 1e-9);
        assert_eq!(soft, Some(157.70));
        // BUY: 下へ
        let (hard, soft) = protective_stops(Action::Buy, 1.1360, Some(0.0004), &close);
        assert!((hard - 1.1352).abs() < 1e-9);
        assert_eq!(soft, Some(1.1360));
        // ATR が無いと幅を決められないので、LLM の SL をそのまま置く
        assert_eq!(protective_stops(Action::Sell, 157.70, None, &close), (157.70, None));
    }

    #[test]
    fn close_stop_breach_needs_a_close_beyond_the_line() {
        assert!(close_stop_breached("SELL", 157.70, 157.71));
        assert!(!close_stop_breached("SELL", 157.70, 157.70));
        assert!(!close_stop_breached("SELL", 157.70, 157.65));
        assert!(close_stop_breached("BUY", 1.1360, 1.1359));
        assert!(!close_stop_breached("BUY", 1.1360, 1.1361));
    }
}
