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
    pub connection_status: ConnectionStatus,
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
