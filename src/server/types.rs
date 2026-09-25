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
    pub invalidation_reason: String,
    /// このポジションを建てた判断（CoT ログ）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cot_log_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CloseReason {
    TakeProfit,
    StopLoss,
    Manual,
    CircuitBreaker,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cot_log_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoTLog {
    pub id: String,
    pub timestamp: String,
    pub symbol: String,
    pub action: String, // "BUY" | "SELL" | "HOLD"
    pub confidence: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_loss: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub take_profit: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_reward_ratio: Option<f64>,
    /// 環境認識（4H/1H）
    pub macro_context: String,
    /// 注文の攻防（15M/5M）
    pub order_flow: String,
    /// 無効化ライン（SLの根拠）
    pub invalidation: String,
    /// 反対材料
    #[serde(default)]
    pub conflicts: String,
    /// ガードで弾かれた場合のガード名（PASS 以外）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guard_result: Option<String>,
    pub reasoning: String,
    pub executed: bool,
    pub spread_pips: f64,
    /// 条件付きプランの成立ログの場合、そのプランを立てた LLM 判断の ID。
    ///
    /// プランは HOLD 判断と一緒に出る（「今は入らないが、条件成立時に売る」）ため、
    /// 「発注した記録」と「根拠になった判断」は別のログになる。取引はこの成立ログに紐づき、
    /// 根拠はここから辿る。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_cot_log_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionStatus {
    pub ctrader: String, // "connected" | "connecting" | "disconnected"
    pub llm: String,     // "ready" | "busy" | "error"
    pub ping_ms: i64,
    pub environment: String, // "DEMO" | "LIVE"
    pub account_number: String,
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
    pub usdjpy_spread: f64,
    pub eurusd_spread: f64,
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
    pub llm_cli: String,
    pub llm_timeout_secs: u64,
    pub env_file_present: bool,
    /// cTrader の認証情報（トークン）が設定されているか
    pub ctrader_token_present: bool,
    /// Refresh Token があり自動更新できるか
    pub ctrader_refresh_token_present: bool,
    /// Access Token の失効時刻（UTC）。不明なら None
    pub ctrader_token_expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LessonLearned {
    pub id: String,
    pub created_at: String,
    pub symbol: String,
    pub rule: String,
    pub context: String,
    pub active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_trade_id: Option<String>,
    pub category: String, // "RISK" | "TIMING" | "PATTERN" | "NEWS"
}

// リクエスト & レスポンス用構造体

pub const DEFAULT_PAGE_LIMIT: usize = 20;
pub const MAX_PAGE_LIMIT: usize = 200;

/// 一覧 API のページング指定（`?limit=20&offset=0`）
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    /// ペアで絞り込む（CoT ログのみ）
    pub symbol: Option<String>,
    /// 判断 ID で絞り込む（決済履歴のみ）
    pub cot_log_id: Option<String>,
    /// 本文のフリーワード検索（CoT ログのみ）
    pub q: Option<String>,
}

impl PageQuery {
    pub fn limit(&self) -> usize {
        self.limit.unwrap_or(DEFAULT_PAGE_LIMIT).clamp(1, MAX_PAGE_LIMIT)
    }

    pub fn offset(&self) -> usize {
        self.offset.unwrap_or(0)
    }
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
    /// 本文（銘柄・売買・決済理由・時刻・判断ID）のフリーワード部分一致
    pub q: Option<String>,
    /// 判断 ID で絞り込む
    pub cot_log_id: Option<String>,
    pub symbol: Option<String>,
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

    /// 判断 ID 指定だけの既定クエリ（CoT 詳細などの内部用）
    pub fn by_cot_log(cot_log_id: Option<&str>, limit: usize, offset: usize) -> Self {
        Self {
            limit: Some(limit),
            offset: Some(offset),
            cot_log_id: cot_log_id.map(str::to_string),
            ..Default::default()
        }
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

/// 1件の判断と、そこから生まれた取引
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoTDetail {
    pub log: CoTLog,
    /// 決済済みの取引
    pub trades: Vec<TradeHistory>,
    /// 保有中のポジション
    pub open_positions: Vec<Position>,
    /// 成立ログの場合、そのプランを立てた元の LLM 判断
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<CoTLog>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateBotStateRequest {
    pub state: BotState,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateLessonRequest {
    pub symbol: String,
    pub rule: String,
    pub context: String,
    pub category: String,
    #[serde(default = "default_active")]
    pub active: bool,
}

fn default_active() -> bool {
    true
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
