//! FIX 経由の発注・決済
//!
//! # SL/TP の扱い
//!
//! cTrader の FIX API は NewOrderSingle(35=D) に SL/TP を添付できない
//! （`AbsoluteSL`(1002) などのタグは ExecutionReport / PositionReport 側にしか存在しない）。
//! そのため成行の約定後に、反対サイドの Stop 注文（SL）と Limit 注文（TP）を別途発注して
//! 保護注文とする。
//!
//! # なぜ OCO 管理が必須か
//!
//! ネッティング口座では保有ポジションが無い状態で保護注文が発動すると、決済ではなく
//! **新規の逆ポジションが立つ**。したがって「片方が約定したらもう片方を必ず取り消す」
//! 処理と、「ポジションが無くなったら保護注文を取り消す」処理が無いと、
//! 意図しない建玉を生む。前者は [`FixTradingClient`] 内の OCO 監視タスクが、
//! 後者は [`FixTradingClient::cancel_protective_orders`] が担う。

use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, Mutex};
use tracing::{info, warn};

use super::config::FixConfig;
use super::message::{exec_type, format_timestamp, msg_type, ord_status, ord_type, side, tag, FixMessage};
use super::session::{new_cl_ord_id, reject_reason, FixSession};

/// 成行注文の結果を待つ上限
const ORDER_TIMEOUT: Duration = Duration::from_secs(15);

/// 1 lot に相当する通貨単位数。FIX の OrderQty(38) は units（cents ではない）。
const UNITS_PER_LOT: f64 = 100_000.0;

/// ロット数を FIX の OrderQty(38) に変換する
pub fn lots_to_units(lots: f64) -> f64 {
    (lots * UNITS_PER_LOT * 100.0).round() / 100.0
}

/// 成行注文の約定結果
#[derive(Debug, Clone, PartialEq)]
pub struct FixFill {
    /// cTrader のポジション ID（PosMaintRptID(721)）
    pub position_id: String,
    /// cTrader の注文 ID（OrderID(37)）
    pub order_id: String,
    /// 約定価格（AvgPx(6)）
    pub avg_px: Option<f64>,
    /// 約定数量（units）
    pub filled_units: f64,
}

/// ポジションに紐づけた保護注文
#[derive(Debug, Clone, PartialEq)]
struct ProtectiveOrder {
    cl_ord_id: String,
    order_id: Option<String>,
    kind: ProtectiveKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProtectiveKind {
    StopLoss,
    TakeProfit,
}

/// FIX 取引セッションを使う発注クライアント
pub struct FixTradingClient {
    session: Arc<FixSession>,
    /// position_id → その建玉を守る保護注文
    protective: Arc<Mutex<HashMap<String, Vec<ProtectiveOrder>>>>,
}

impl FixTradingClient {
    pub async fn connect(config: FixConfig) -> Result<Self> {
        let session = FixSession::connect(config).await?;
        let client = Self {
            session: session.clone(),
            protective: Arc::new(Mutex::new(HashMap::new())),
        };
        tokio::spawn(oco_loop(session, client.protective.clone()));
        Ok(client)
    }

    pub fn is_connected(&self) -> bool {
        self.session.is_connected()
    }

    pub fn session(&self) -> &Arc<FixSession> {
        &self.session
    }

    /// 成行注文を出し、約定（ExecType=F）まで待つ
    pub async fn place_market_order(&self, symbol_id: i64, is_buy: bool, units: f64) -> Result<FixFill> {
        let cl_ord_id = new_cl_ord_id("mkt");
        let mut rx = self.session.subscribe();

        let mut m = FixMessage::new(msg_type::NEW_ORDER_SINGLE);
        m.set(tag::CL_ORD_ID, cl_ord_id.clone())
            .set(tag::SYMBOL, symbol_id.to_string())
            .set(tag::SIDE, if is_buy { side::BUY } else { side::SELL })
            .set(tag::TRANSACT_TIME, format_timestamp(Utc::now()))
            .set(tag::ORDER_QTY, format_qty(units))
            .set(tag::ORD_TYPE, ord_type::MARKET);
        self.session.send(&mut m).await?;
        info!(symbol_id, is_buy, units, %cl_ord_id, "FIX market order sent");

        let report = await_order_report(&mut rx, &cl_ord_id).await?;
        let position_id = report
            .get(tag::POS_MAINT_RPT_ID)
            .ok_or_else(|| anyhow!("FIX execution report has no position id (PosMaintRptID)"))?
            .to_string();
        Ok(FixFill {
            position_id,
            order_id: report.get(tag::ORDER_ID).unwrap_or_default().to_string(),
            avg_px: report.get_f64(tag::AVG_PX),
            filled_units: report.get_f64(tag::CUM_QTY).unwrap_or(units),
        })
    }

    /// 約定済みポジションに SL / TP を保護注文として張る。
    ///
    /// `entry_is_buy` はエントリー側の売買方向。保護注文はその反対サイドで出す。
    /// 片方でも発注に失敗した場合は、既に出した方を取り消してからエラーを返す
    /// （孤立した保護注文を残さないため）。
    pub async fn attach_protective_orders(
        &self,
        position_id: &str,
        symbol_id: i64,
        entry_is_buy: bool,
        units: f64,
        stop_loss: Option<f64>,
        take_profit: Option<f64>,
    ) -> Result<()> {
        let exit_side = if entry_is_buy { side::SELL } else { side::BUY };
        let mut placed: Vec<ProtectiveOrder> = Vec::new();

        if let Some(sl) = stop_loss.filter(|p| *p > 0.0) {
            match self.send_protective(symbol_id, exit_side, units, ord_type::STOP, tag::STOP_PX, sl).await {
                Ok(o) => placed.push(ProtectiveOrder { kind: ProtectiveKind::StopLoss, ..o }),
                Err(e) => return Err(e.context("Failed to place stop loss order")),
            }
        }
        if let Some(tp) = take_profit.filter(|p| *p > 0.0) {
            match self.send_protective(symbol_id, exit_side, units, ord_type::LIMIT, tag::PRICE, tp).await {
                Ok(o) => placed.push(ProtectiveOrder { kind: ProtectiveKind::TakeProfit, ..o }),
                Err(e) => {
                    // SL だけが残ると、決済されないまま逆建玉を作る危険がある
                    for o in &placed {
                        if let Err(e2) = self.cancel(&o.cl_ord_id, o.order_id.as_deref()).await {
                            warn!("Failed to roll back protective order {}: {e2:#}", o.cl_ord_id);
                        }
                    }
                    return Err(e.context("Failed to place take profit order"));
                }
            }
        }

        if !placed.is_empty() {
            info!(position_id, count = placed.len(), "FIX protective orders attached");
            self.protective.lock().await.insert(position_id.to_string(), placed);
        }
        Ok(())
    }

    /// ポジションに紐づく保護注文をすべて取り消す。
    ///
    /// 建玉を手動・成行で決済したときは必ず呼ぶこと。放置すると保護注文が孤立し、
    /// ネッティング口座では発動時に逆建玉を作ってしまう。
    pub async fn cancel_protective_orders(&self, position_id: &str) -> Result<()> {
        let orders = self.protective.lock().await.remove(position_id);
        let Some(orders) = orders else { return Ok(()) };
        let mut failures = Vec::new();
        for o in &orders {
            if let Err(e) = self.cancel(&o.cl_ord_id, o.order_id.as_deref()).await {
                failures.push(format!("{}: {e:#}", o.cl_ord_id));
            }
        }
        if failures.is_empty() {
            info!(position_id, "FIX protective orders cancelled");
            Ok(())
        } else {
            bail!("Failed to cancel protective orders: {}", failures.join(", "))
        }
    }

    /// 成行でポジションを決済する（保護注文の取り消しも行う）
    pub async fn close_position(&self, position_id: &str, symbol_id: i64, entry_is_buy: bool, units: f64) -> Result<FixFill> {
        // 先に保護注文を外さないと、決済後に孤立した注文が逆建玉を作りうる
        if let Err(e) = self.cancel_protective_orders(position_id).await {
            warn!("{e:#}");
        }
        self.place_market_order(symbol_id, !entry_is_buy, units).await
    }

    async fn send_protective(
        &self,
        symbol_id: i64,
        exit_side: &str,
        units: f64,
        order_type: &str,
        price_tag: u32,
        price: f64,
    ) -> Result<ProtectiveOrder> {
        let cl_ord_id = new_cl_ord_id(if order_type == ord_type::STOP { "sl" } else { "tp" });
        let mut rx = self.session.subscribe();

        let mut m = FixMessage::new(msg_type::NEW_ORDER_SINGLE);
        m.set(tag::CL_ORD_ID, cl_ord_id.clone())
            .set(tag::SYMBOL, symbol_id.to_string())
            .set(tag::SIDE, exit_side)
            .set(tag::TRANSACT_TIME, format_timestamp(Utc::now()))
            .set(tag::ORDER_QTY, format_qty(units))
            .set(tag::ORD_TYPE, order_type)
            .set(price_tag, format_price(price));
        self.session.send(&mut m).await?;

        // 保護注文は約定ではなく「受理(New)」まで待つ
        let report = await_order_accepted(&mut rx, &cl_ord_id).await?;
        Ok(ProtectiveOrder {
            cl_ord_id,
            order_id: report.get(tag::ORDER_ID).map(|s| s.to_string()),
            kind: ProtectiveKind::StopLoss, // 呼び出し側で上書きする
        })
    }

    async fn cancel(&self, orig_cl_ord_id: &str, order_id: Option<&str>) -> Result<()> {
        let mut m = FixMessage::new(msg_type::ORDER_CANCEL_REQUEST);
        m.set(tag::ORIG_CL_ORD_ID, orig_cl_ord_id)
            .set(tag::CL_ORD_ID, new_cl_ord_id("cxl"));
        if let Some(id) = order_id {
            m.set(tag::ORDER_ID, id);
        }
        self.session.send(&mut m).await.context("Failed to send FIX cancel request")?;
        Ok(())
    }

    pub async fn logout(&self) {
        self.session.logout().await;
    }
}

/// 成行注文の ExecutionReport（約定 or 拒否）を待つ
async fn await_order_report(rx: &mut broadcast::Receiver<FixMessage>, cl_ord_id: &str) -> Result<FixMessage> {
    let deadline = tokio::time::Instant::now() + ORDER_TIMEOUT;
    loop {
        let msg = recv_until(rx, deadline).await?;
        if !is_report_for(&msg, cl_ord_id) {
            continue;
        }
        match msg.msg_type() {
            msg_type::BUSINESS_MESSAGE_REJECT => bail!("cTrader rejected order: {}", reject_reason(&msg)),
            msg_type::EXECUTION_REPORT => match msg.get(tag::EXEC_TYPE).unwrap_or_default() {
                // 成行は New → Trade の順に来るので、受理だけの通知は待ち続ける
                exec_type::NEW => continue,
                exec_type::TRADE => {
                    if msg.get(tag::ORD_STATUS) == Some(ord_status::PARTIALLY_FILLED) {
                        continue;
                    }
                    return Ok(msg);
                }
                exec_type::REJECTED => bail!("cTrader rejected order: {}", reject_reason(&msg)),
                exec_type::CANCELED => bail!("cTrader cancelled order: {}", reject_reason(&msg)),
                exec_type::EXPIRED => bail!("cTrader expired order: {}", reject_reason(&msg)),
                _ => continue,
            },
            _ => continue,
        }
    }
}

/// 保護注文が受理された（ExecType=New）ことを確認する
async fn await_order_accepted(rx: &mut broadcast::Receiver<FixMessage>, cl_ord_id: &str) -> Result<FixMessage> {
    let deadline = tokio::time::Instant::now() + ORDER_TIMEOUT;
    loop {
        let msg = recv_until(rx, deadline).await?;
        if !is_report_for(&msg, cl_ord_id) {
            continue;
        }
        match msg.msg_type() {
            msg_type::BUSINESS_MESSAGE_REJECT => bail!("cTrader rejected order: {}", reject_reason(&msg)),
            msg_type::EXECUTION_REPORT => match msg.get(tag::EXEC_TYPE).unwrap_or_default() {
                exec_type::NEW => return Ok(msg),
                // 逆指値・指値が即座に条件を満たしていた場合はそのまま約定する
                exec_type::TRADE => return Ok(msg),
                exec_type::REJECTED => bail!("cTrader rejected order: {}", reject_reason(&msg)),
                exec_type::EXPIRED | exec_type::CANCELED => {
                    bail!("cTrader did not accept order: {}", reject_reason(&msg))
                }
                _ => continue,
            },
            _ => continue,
        }
    }
}

async fn recv_until(
    rx: &mut broadcast::Receiver<FixMessage>,
    deadline: tokio::time::Instant,
) -> Result<FixMessage> {
    loop {
        match tokio::time::timeout_at(deadline, rx.recv()).await {
            Ok(Ok(m)) => return Ok(m),
            Ok(Err(broadcast::error::RecvError::Lagged(n))) => {
                warn!("Missed {n} FIX messages while waiting for an order report");
                continue;
            }
            Ok(Err(broadcast::error::RecvError::Closed)) => bail!("FIX session closed while waiting for order report"),
            Err(_) => bail!(
                "No FIX execution report within {}s; order state at broker is unknown",
                ORDER_TIMEOUT.as_secs()
            ),
        }
    }
}

fn is_report_for(msg: &FixMessage, cl_ord_id: &str) -> bool {
    msg.get(tag::CL_ORD_ID) == Some(cl_ord_id)
}

/// 保護注文の片方が約定したら、もう片方を取り消す
async fn oco_loop(session: Arc<FixSession>, protective: Arc<Mutex<HashMap<String, Vec<ProtectiveOrder>>>>) {
    let mut rx = session.subscribe();
    loop {
        let msg = match rx.recv().await {
            Ok(m) => m,
            Err(broadcast::error::RecvError::Lagged(n)) => {
                warn!("OCO monitor missed {n} FIX messages");
                continue;
            }
            Err(broadcast::error::RecvError::Closed) => break,
        };
        if msg.msg_type() != msg_type::EXECUTION_REPORT {
            continue;
        }
        let Some(cl_ord_id) = msg.get(tag::CL_ORD_ID) else { continue };
        let exec = msg.get(tag::EXEC_TYPE).unwrap_or_default();
        let filled = exec == exec_type::TRADE && msg.get(tag::ORD_STATUS) == Some(ord_status::FILLED);
        let gone = matches!(exec, exec_type::CANCELED | exec_type::EXPIRED | exec_type::REJECTED);
        if !filled && !gone {
            continue;
        }

        // 約定・消滅した注文が属するポジションを特定する
        let mut guard = protective.lock().await;
        let Some((position_id, siblings)) = guard
            .iter()
            .find(|(_, orders)| orders.iter().any(|o| o.cl_ord_id == cl_ord_id))
            .map(|(k, v)| (k.clone(), v.clone()))
        else {
            continue;
        };
        guard.remove(&position_id);
        drop(guard);

        if filled {
            info!(position_id, %cl_ord_id, "protective order filled; cancelling the other side");
        }
        for o in siblings.iter().filter(|o| o.cl_ord_id != cl_ord_id) {
            let mut m = FixMessage::new(msg_type::ORDER_CANCEL_REQUEST);
            m.set(tag::ORIG_CL_ORD_ID, o.cl_ord_id.clone())
                .set(tag::CL_ORD_ID, new_cl_ord_id("cxl"));
            if let Some(id) = &o.order_id {
                m.set(tag::ORDER_ID, id.clone());
            }
            if let Err(e) = session.send(&mut m).await {
                warn!("Failed to cancel sibling protective order {}: {e:#}", o.cl_ord_id);
            }
        }
    }
}

/// OrderQty(38) は最大精度 0.01
fn format_qty(units: f64) -> String {
    let rounded = (units * 100.0).round() / 100.0;
    if (rounded - rounded.trunc()).abs() < f64::EPSILON {
        format!("{}", rounded.trunc() as i64)
    } else {
        format!("{rounded}")
    }
}

/// 価格は余分な桁を出さない（cTrader は最大5桁）
fn format_price(price: f64) -> String {
    format!("{:.5}", price).trim_end_matches('0').trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_lots_to_units() {
        // FIX の OrderQty は units。0.01 lot = 1,000 通貨
        assert_eq!(lots_to_units(0.01), 1_000.0);
        assert_eq!(lots_to_units(1.0), 100_000.0);
        assert_eq!(lots_to_units(0.05), 5_000.0);
    }

    #[test]
    fn formats_quantity_without_trailing_decimals() {
        assert_eq!(format_qty(1_000.0), "1000");
        assert_eq!(format_qty(1_000.5), "1000.5");
    }

    #[test]
    fn formats_price_without_trailing_zeros() {
        assert_eq!(format_price(157.04), "157.04");
        assert_eq!(format_price(1.08395), "1.08395");
        assert_eq!(format_price(157.0), "157");
    }
}
