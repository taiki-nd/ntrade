//! 戦略の入出力。バックテストと本番で同じ型を使う。
//!
//! 戦略は確定足を受け取り、注文の意図（`Action`）を返すだけ。発注量の計算・リスクチェック・執行は
//! 呼び出し側（`backtest` / `live`）が行う。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::ctrader::{BarPeriod, CandleBar, PendingKind};

/// 売買の向き
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Side {
    Buy,
    Sell,
}

impl Side {
    /// BUY = +1 / SELL = -1。価格差に掛けると有利な方向が正になる
    pub fn sign(self) -> f64 {
        match self {
            Side::Buy => 1.0,
            Side::Sell => -1.0,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Side::Buy => "BUY",
            Side::Sell => "SELL",
        }
    }

    pub fn opposite(self) -> Self {
        match self {
            Side::Buy => Side::Sell,
            Side::Sell => Side::Buy,
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_uppercase().as_str() {
            "BUY" => Some(Side::Buy),
            "SELL" => Some(Side::Sell),
            _ => None,
        }
    }
}

/// 新規注文の入り方
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "UPPERCASE")]
pub enum Entry {
    /// 成行。次の足の始値付近で約定する
    Market,
    /// 指値（今より有利な価格まで戻ったら約定）
    Limit { price: f64 },
    /// 逆指値（その価格を抜けたら約定）
    Stop { price: f64 },
}

impl Entry {
    pub fn pending(&self) -> Option<(PendingKind, f64)> {
        match *self {
            Entry::Market => None,
            Entry::Limit { price } => Some((PendingKind::Limit, price)),
            Entry::Stop { price } => Some((PendingKind::Stop, price)),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Entry::Market => "MARKET",
            Entry::Limit { .. } => "LIMIT",
            Entry::Stop { .. } => "STOP",
        }
    }
}

/// 新規注文の意図（発注量はリスク設定から呼び出し側が決める）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderIntent {
    pub side: Side,
    pub entry: Entry,
    /// 損切り（絶対価格）。必須
    pub stop_loss: f64,
    /// 利確（絶対価格）。None なら置かない（戦略の決済シグナルで手仕舞う）
    pub take_profit: Option<f64>,
    /// 予約注文の有効期限（分）。None なら上限（`GuardConfig::order_max_hours`）まで
    pub expiry_minutes: Option<u32>,
    /// 記録用の理由
    pub reason: String,
}

/// 戦略が出す操作
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Action {
    Open(OrderIntent),
    /// 建玉を成行で決済する
    ClosePosition { id: String, reason: String },
    /// 予約注文を取り消す
    CancelOrder { id: String, reason: String },
    /// 建玉の SL / TP を付け替える（None の項目は変えない）
    ModifyPosition {
        id: String,
        stop_loss: Option<f64>,
        take_profit: Option<f64>,
        reason: String,
    },
}

impl Action {
    pub fn kind(&self) -> &'static str {
        match self {
            Action::Open(_) => "OPEN",
            Action::ClosePosition { .. } => "CLOSE",
            Action::CancelOrder { .. } => "CANCEL",
            Action::ModifyPosition { .. } => "MODIFY",
        }
    }

    pub fn reason(&self) -> &str {
        match self {
            Action::Open(i) => &i.reason,
            Action::ClosePosition { reason, .. } | Action::CancelOrder { reason, .. } | Action::ModifyPosition { reason, .. } => reason,
        }
    }
}

/// 戦略から見た保有中の建玉（その戦略が建てたものだけ）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PositionView {
    pub id: String,
    pub side: Side,
    pub volume_lots: f64,
    pub entry_price: f64,
    pub stop_loss: Option<f64>,
    pub take_profit: Option<f64>,
    pub open_time: DateTime<Utc>,
}

/// 戦略から見た約定待ちの予約注文（その戦略が置いたものだけ）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderView {
    pub id: String,
    pub side: Side,
    pub kind: PendingKind,
    pub price: f64,
    pub stop_loss: Option<f64>,
    pub take_profit: Option<f64>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// 戦略に渡す1足分の状況
#[derive(Debug, Clone)]
pub struct StrategyContext<'a> {
    pub pair: &'a str,
    /// 判断時刻（直近の足が確定した時刻）
    pub now: DateTime<Utc>,
    /// 戦略の時間足の確定足（古い順。末尾が今確定した足）。本数は `Strategy::lookback` 以下
    pub bars: &'a [CandleBar],
    pub pip_size: f64,
    pub positions: &'a [PositionView],
    pub orders: &'a [OrderView],
}

impl StrategyContext<'_> {
    pub fn last(&self) -> Option<&CandleBar> {
        self.bars.last()
    }

    pub fn position(&self, side: Side) -> Option<&PositionView> {
        self.positions.iter().find(|p| p.side == side)
    }
}

/// ロジック戦略。
///
/// `on_bar` は戦略の時間足が確定するたびに呼ばれる。本番では再起動のたびに作り直すので、
/// 判断は渡された足（`lookback` 本）だけから決めること（内部状態に頼ると本番とバックテストで結果がずれる）。
pub trait Strategy: Send {
    /// 判断に使う足の本数（指標の助走を含む）
    fn lookback(&self) -> usize;
    fn on_bar(&mut self, ctx: &StrategyContext) -> Vec<Action>;
}

/// 戦略パラメータ（名前 → 値）。整数のパラメータも f64 で持ち、戦略側で丸める
pub type Params = BTreeMap<String, f64>;

/// どの戦略を・どの時間足で・どのパラメータで動かすか
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategySpec {
    pub name: String,
    pub timeframe: BarPeriod,
    #[serde(default)]
    pub params: Params,
}

impl StrategySpec {
    /// 表示用: `ema_cross M5 fast=20 slow=50`
    pub fn label(&self) -> String {
        let params: Vec<String> = self.params.iter().map(|(k, v)| format!("{k}={v}")).collect();
        format!("{} {} {}", self.name, self.timeframe.as_str(), params.join(" ")).trim().to_string()
    }
}

/// パラメータの定義（既定値と、画面・最適化で使う範囲）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParamSpec {
    pub name: &'static str,
    pub default: f64,
    pub min: f64,
    pub max: f64,
    /// 整数で使うパラメータか（期間など）
    pub integer: bool,
    pub description: &'static str,
}

/// 戦略の一覧に出す情報
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyInfo {
    pub name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub params: Vec<ParamSpec>,
}
