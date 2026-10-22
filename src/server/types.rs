use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BotState {
    Running,
    Paused,
    CircuitBreaker,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub id: String,
    pub symbol: String,
    pub side: String, // "BUY" | "SELL"
    pub volume_lots: f64,
    pub entry_price: f64,
    pub current_price: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub pnl_pips: f64,
    pub pnl_amount: f64,
    pub open_time: String,
    /// 建てた理由など（戦略のシグナル理由・ブローカーから取り込んだ旨）
    #[serde(alias = "invalidationReason")]
    pub note: String,
    /// この建玉を建てたシグナル
    #[serde(default, alias = "cotLogId", skip_serializing_if = "Option::is_none")]
    pub signal_id: Option<String>,
    /// この建玉を管理する戦略（デプロイ ID）。None なら戦略の管理外（手動・取り込み）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy: Option<String>,
}

/// ブローカーに置いてある、まだ約定していない予約注文（ntrade の記録）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkingOrder {
    /// ブローカーの注文 ID
    pub id: String,
    pub symbol: String,
    pub side: String, // "BUY" | "SELL"
    pub kind: crate::ctrader::PendingKind,
    pub volume_lots: f64,
    pub entry_price: f64,
    pub stop_loss: f64,
    /// 0.0 なら TP なし
    pub take_profit: f64,
    /// "YYYY-MM-DD HH:MM:SS"（UTC）
    pub created_at: String,
    pub expires_at: String,
    #[serde(alias = "invalidationReason")]
    pub note: String,
    /// この注文を置いたシグナル。約定した建玉に引き継ぐ
    #[serde(default, alias = "cotLogId", skip_serializing_if = "Option::is_none")]
    pub signal_id: Option<String>,
    /// この注文を置いた戦略（デプロイ ID）。約定した建玉に引き継ぐ
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CloseReason {
    TakeProfit,
    StopLoss,
    /// 戦略の決済シグナルで ntrade が成行決済した
    Strategy,
    Manual,
    CircuitBreaker,
    /// 旧: LLM 時代の終値判定の損切り（保存済みの決済履歴を読むためだけに残す）
    Invalidated,
    /// 旧: LLM が判断サイクルで決済を選んだ（同上）
    Discretionary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradeHistory {
    pub id: String,
    pub symbol: String,
    pub side: String, // "BUY" | "SELL"
    pub volume_lots: f64,
    pub entry_price: f64,
    pub close_price: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub pnl_pips: f64,
    pub pnl_amount: f64,
    pub close_reason: CloseReason,
    pub open_time: String,
    pub close_time: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal_id: Option<String>,
    /// 建てた戦略（デプロイ ID）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionStatus {
    pub ctrader: String, // "connected" | "connecting" | "disconnected"
    pub ping_ms: i64,
    pub environment: String, // "DEMO" | "LIVE"
    pub account_number: String,
}

/// 口座残高の推移の1点（決済・入出金の直後の残高）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BalancePoint {
    pub time: String,
    pub balance: f64,
}

/// GET /api/balance-history のクエリ。`days` を省略すると全期間
#[derive(Debug, Clone, Default, Deserialize)]
pub struct BalanceHistoryQuery {
    pub days: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountMetrics {
    /// ボット稼働状態（`/api/status` 応答時に最新値を反映）
    pub bot_state: BotState,
    pub balance: f64,
    pub equity: f64,
    pub margin: f64,
    pub free_margin: f64,
    pub daily_pnl: f64,
    pub daily_pnl_percent: f64,
    pub unrealized_pnl: f64,
    pub win_rate_today: f64,
    pub total_trades_today: u32,
    pub winning_trades_today: u32,
    /// ペアごとの直近スプレッド（pips）。気配値の購読から
    pub spreads: std::collections::BTreeMap<String, f64>,
    pub circuit_breaker_threshold_percent: f64,
    /// "paper" | "live"。live のとき balance はブローカー残高を反映する
    pub order_mode: String,
    /// cTrader から取得したブローカー口座残高（未取得なら None）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub broker_balance: Option<f64>,
    pub connection_status: ConnectionStatus,
}

/// エンジンの稼働設定（環境変数と設定ファイル由来。読み取り専用）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInfo {
    pub order_mode: String,
    pub scheduler_enabled: bool,
    pub pairs: Vec<String>,
    pub bar_delay_secs: i64,
    pub paper_balance: f64,
    pub guard_config_path: String,
    pub db_path: String,
    pub env_file_present: bool,
    /// cTrader の認証情報（トークン）が設定されているか
    pub ctrader_token_present: bool,
    /// Refresh Token があり自動更新できるか
    pub ctrader_refresh_token_present: bool,
    /// Access Token の失効時刻（UTC）。不明なら None
    pub ctrader_token_expires_at: Option<String>,
}

// リクエスト & レスポンス用構造体

pub const DEFAULT_PAGE_LIMIT: usize = 20;
pub const MAX_PAGE_LIMIT: usize = 200;

/// シグナルの検索条件（`?limit=20&offset=0&symbol=&deployment=&status=&q=`）
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub symbol: Option<String>,
    pub deployment: Option<String>,
    /// EXECUTED / REJECTED / FAILED / DRY_RUN
    pub status: Option<String>,
    /// 本文のフリーワード検索
    pub q: Option<String>,
}

impl SignalQuery {
    pub fn limit(&self) -> usize {
        self.limit.unwrap_or(DEFAULT_PAGE_LIMIT).clamp(1, MAX_PAGE_LIMIT)
    }

    pub fn offset(&self) -> usize {
        self.offset.unwrap_or(0)
    }
}

/// シグナル（戦略が出した操作）の処理結果
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SignalStatus {
    /// ブローカーで執行した
    Executed,
    /// リスクチェックで弾いた
    Rejected,
    /// 発注・操作がブローカーで失敗した
    Failed,
    /// ドライラン（発注せず記録だけ）
    DryRun,
}

impl SignalStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Executed => "EXECUTED",
            Self::Rejected => "REJECTED",
            Self::Failed => "FAILED",
            Self::DryRun => "DRY_RUN",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "EXECUTED" => Self::Executed,
            "REJECTED" => Self::Rejected,
            "DRY_RUN" => Self::DryRun,
            _ => Self::Failed,
        }
    }
}

/// 戦略が出した操作 1 件の記録
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalLog {
    pub id: String,
    pub timestamp: String,
    /// デプロイ ID
    pub deployment: String,
    /// 戦略と時間足・パラメータ（`StrategySpec::label`）
    pub strategy: String,
    pub symbol: String,
    /// OPEN / CLOSE / CANCEL / MODIFY
    pub action: String,
    pub side: Option<String>,
    pub reason: String,
    /// 操作の中身（注文の意図・発注量・判断時の相場など）
    pub detail: serde_json::Value,
    /// リスクチェックの結果（OPEN のみ。PASS か失敗したチェック名）
    pub guard_result: Option<String>,
    pub status: SignalStatus,
    pub message: Option<String>,
    /// 置いた注文 ID / 建てた建玉 ID / 操作した対象
    pub order_id: Option<String>,
    pub dry_run: bool,
}

/// 決済履歴の並び順（列名は固定値としてSQLへ埋め込む）
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TradeSort {
    #[default]
    CloseTime,
    OpenTime,
    PnlPips,
    PnlAmount,
    VolumeLots,
    Symbol,
}

impl TradeSort {
    pub fn column(self) -> &'static str {
        match self {
            Self::CloseTime => "close_time",
            Self::OpenTime => "open_time",
            Self::PnlPips => "pnl_pips",
            Self::PnlAmount => "pnl_amount",
            Self::VolumeLots => "volume_lots",
            Self::Symbol => "symbol",
        }
    }
}

/// 決済履歴の検索条件（`None` の項目は絞り込まない）
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradeQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    /// 本文（銘柄・売買・決済理由・時刻・シグナルID・戦略）のフリーワード部分一致
    pub q: Option<String>,
    /// シグナル ID で絞り込む
    pub signal_id: Option<String>,
    pub symbol: Option<String>,
    /// 戦略（デプロイ ID）で絞り込む
    pub strategy: Option<String>,
    /// "BUY" | "SELL"
    pub side: Option<String>,
    pub close_reason: Option<CloseReason>,
    /// "win" = 損益 >= 0 のみ / "loss" = 損益 < 0 のみ
    pub result: Option<String>,
    /// 決済時刻の下限（"YYYY-MM-DD" または "YYYY-MM-DD HH:MM:SS"）
    pub from: Option<String>,
    /// 決済時刻の上限（日付のみの場合はその日の終わりまで）
    pub to: Option<String>,
    pub min_pnl_pips: Option<f64>,
    pub max_pnl_pips: Option<f64>,
    #[serde(default)]
    pub sort: TradeSort,
    /// true で昇順（既定は降順）
    #[serde(default)]
    pub asc: bool,
}

impl TradeQuery {
    pub fn limit(&self) -> usize {
        self.limit.unwrap_or(DEFAULT_PAGE_LIMIT).clamp(1, MAX_PAGE_LIMIT)
    }

    pub fn offset(&self) -> usize {
        self.offset.unwrap_or(0)
    }

    /// Some(true) = 勝ちのみ / Some(false) = 負けのみ
    pub fn win_only(&self) -> Option<bool> {
        match self.result.as_deref().map(str::trim) {
            Some("win") => Some(true),
            Some("loss") => Some(false),
            _ => None,
        }
    }
}

/// 決済履歴の一括削除リクエスト
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteTradesRequest {
    pub ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateBotStateRequest {
    pub state: BotState,
}

#[derive(Debug, Serialize)]
pub struct OAuthUrlResponse {
    pub url: String,
    pub client_id: String,
    pub redirect_uri: String,
}

#[derive(Debug, Deserialize)]
pub struct OAuthExchangeRequest {
    pub code: String,
    pub redirect_uri: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    pub trader_login: i64,
    pub ctid_trader_account_id: i64,
    pub broker_title: String,
    pub is_live: bool,
}

#[derive(Debug, Deserialize)]
pub struct SelectAccountRequest {
    pub account_id: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: None,
        }
    }

    pub fn ok_msg(data: T, msg: impl Into<String>) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: Some(msg.into()),
        }
    }

    pub fn err(msg: impl Into<String>) -> ApiResponse<T> {
        ApiResponse {
            success: false,
            data: None,
            message: Some(msg.into()),
        }
    }
}
