use serde::{Deserialize, Serialize};

/// 取引アクション
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Action {
    Buy,
    Sell,
    Hold,
}

/// 注文種別
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum EntryType {
    Market,
    Limit,
}

/// 上位足の環境認識構造体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketStructure {
    #[serde(rename = "4H_trend")]
    pub trend_4h: String,
    pub nearest_h4_resistance: f64,
    pub nearest_h4_support: f64,
    #[serde(rename = "1H_trend")]
    pub trend_1h: String,
}

/// 5分足の直近ローソク足特徴量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentBarFeatures {
    #[serde(rename = "type")]
    pub bar_type: String, // PINBAR, ENGULFING, STANDARD など
    pub direction: String, // BULLISH, BEARISH
    pub total_range_pips: f64,
    pub lower_wick_ratio: f64,
    pub upper_wick_ratio: f64,
    pub body_ratio: f64,
    pub rejection_level: Option<f64>,
}

/// 5分足プライスアクション分析
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FiveMinutePriceAction {
    pub current_bar: CurrentBarFeatures,
    pub pattern_detected: String,
    pub at_key_level: String,
}

/// LLMに渡す構造化プライスアクション入力（JSON）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceActionInput {
    pub pair: String,
    pub timestamp: String,
    pub spread_pips: f64,
    pub current_price: f64,
    pub market_structure: MarketStructure,
    #[serde(rename = "5M_price_action")]
    pub five_minute_pa: FiveMinutePriceAction,
}

/// プライスアクション分析根拠（LLM出力の内部構造）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceActionAnalysis {
    pub macro_bias: String,
    pub trigger_pattern: String,
    pub invalidation_point: String,
}

/// LLMが返す意思決定JSONスキーマ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeDecision {
    pub action: Action,
    pub confidence: f64,
    pub entry_type: Option<EntryType>,
    pub entry_price: Option<f64>,
    pub stop_loss: Option<f64>,
    pub take_profit: Option<f64>,
    pub risk_reward_ratio: Option<f64>,
    pub price_action_analysis: PriceActionAnalysis,
    pub reasoning: String,
}
