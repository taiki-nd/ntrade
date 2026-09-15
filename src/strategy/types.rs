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
    /// 今は入らず、`conditional_plan` の条件成立を待つ
    Conditional,
}

/// LLM の相場解釈（環境認識 → 攻防 → 無効化 → 反対材料）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Analysis {
    /// 4H/1H: どちらの勢力が優勢か、押し目/戻り形成中か、天井圏/底値圏か
    pub macro_context: String,
    /// 15M/5M: 直近数本で主導権を握り直したのはどちらか、ブレイク試行と拒絶の兆候
    pub order_flow: String,
    /// どこを抜けたらこの相場観は間違いになるか（SLの根拠）
    pub invalidation: String,
    /// シナリオに反する材料（必ず1つ以上）
    pub conflicts: String,
}

/// 今は入らないが条件が揃えば入る、という条件付きプラン
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConditionalPlan {
    /// 成立条件（例: "5Mが154.24を実体で上抜けて確定"）
    pub wait_for: String,
    /// 成立時のアクション
    pub then_action: Action,
    /// プランを破棄する条件
    pub invalidate_if: String,
    /// 期限（"YYYY-MM-DD HH:MM:SS UTC"）
    pub expires_at: String,
    /// 成立時のSL/TP（任意）
    #[serde(default)]
    pub stop_loss: Option<f64>,
    #[serde(default)]
    pub take_profit: Option<f64>,
}

/// LLM が画像で実際に確認した各時間足の最新足時刻（読まずに答えた場合の検出用）
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Observed {
    #[serde(default)]
    pub latest_bar_4h: Option<String>,
    #[serde(default)]
    pub latest_bar_1h: Option<String>,
    #[serde(default)]
    pub latest_bar_15m: Option<String>,
    #[serde(default)]
    pub latest_bar_5m: Option<String>,
}

/// LLM が返す意思決定
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradeDecision {
    pub action: Action,
    pub confidence: f64,
    #[serde(default)]
    pub entry_type: Option<EntryType>,
    #[serde(default)]
    pub entry_price: Option<f64>,
    #[serde(default)]
    pub stop_loss: Option<f64>,
    #[serde(default)]
    pub take_profit: Option<f64>,
    pub analysis: Analysis,
    #[serde(default)]
    pub conditional_plan: Option<ConditionalPlan>,
    #[serde(default)]
    pub observed: Observed,
    pub reasoning: String,
}

impl TradeDecision {
    /// 安全側のフォールバック（HOLD）
    pub fn fallback_hold(reason: impl Into<String>) -> Self {
        let reason = reason.into();
        Self {
            action: Action::Hold,
            confidence: 0.0,
            entry_type: None,
            entry_price: None,
            stop_loss: None,
            take_profit: None,
            analysis: Analysis {
                macro_context: "N/A (system fallback)".into(),
                order_flow: "N/A (system fallback)".into(),
                invalidation: "N/A".into(),
                conflicts: "N/A".into(),
            },
            conditional_plan: None,
            observed: Observed::default(),
            reasoning: format!("FALLBACK_HOLD: {reason}"),
        }
    }

    /// `claude -p --json-schema` に渡す JSON Schema
    pub fn json_schema() -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["action", "confidence", "analysis", "observed", "reasoning"],
            "properties": {
                "action": { "type": "string", "enum": ["BUY", "SELL", "HOLD"] },
                "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
                "entry_type": { "type": ["string", "null"], "enum": ["MARKET", "LIMIT", "CONDITIONAL", null] },
                "entry_price": { "type": ["number", "null"] },
                "stop_loss": { "type": ["number", "null"] },
                "take_profit": { "type": ["number", "null"] },
                "analysis": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["macro_context", "order_flow", "invalidation", "conflicts"],
                    "properties": {
                        "macro_context": { "type": "string" },
                        "order_flow": { "type": "string" },
                        "invalidation": { "type": "string" },
                        "conflicts": { "type": "string" }
                    }
                },
                "conditional_plan": {
                    "type": ["object", "null"],
                    "additionalProperties": false,
                    "required": ["wait_for", "then_action", "invalidate_if", "expires_at"],
                    "properties": {
                        "wait_for": { "type": "string" },
                        "then_action": { "type": "string", "enum": ["BUY", "SELL", "HOLD"] },
                        "invalidate_if": { "type": "string" },
                        "expires_at": { "type": "string" },
                        "stop_loss": { "type": ["number", "null"] },
                        "take_profit": { "type": ["number", "null"] }
                    }
                },
                "observed": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["latest_bar_4h", "latest_bar_1h", "latest_bar_15m", "latest_bar_5m"],
                    "properties": {
                        "latest_bar_4h": { "type": ["string", "null"] },
                        "latest_bar_1h": { "type": ["string", "null"] },
                        "latest_bar_15m": { "type": ["string", "null"] },
                        "latest_bar_5m": { "type": ["string", "null"] }
                    }
                },
                "reasoning": { "type": "string" }
            }
        })
    }
}
