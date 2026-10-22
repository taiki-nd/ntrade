use serde::{Deserialize, Serialize};

/// 取引アクション（新規の予約注文を置くか）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Action {
    Buy,
    Sell,
    Hold,
}

/// 注文種別。新規は予約注文（LIMIT / STOP）だけを出す。
/// LLM の判断には数十秒かかるので、成行では判断した価格から離れたところで約定してしまう。
/// 価格を指定してブローカーに置いておけば、判断の遅れは「注文を置くまでの時間」になるだけで済む
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum EntryType {
    /// 指値: 今より有利な価格まで戻ったら約定（押し目買い・戻り売り）
    Limit,
    /// 逆指値: その価格を抜けたら約定（ブレイクに乗る）
    Stop,
    /// 旧: 成行。保存済みのリプレイ結果を読むためだけに残す
    Market,
    /// 旧: 条件付きプラン。同上
    Conditional,
}

/// LLM の相場解釈（流れ → 価格の位置 → 無効化 → 反対材料）
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Analysis {
    /// 上位足（1H/15M）の流れと、直近（5M）の値動きの構造
    #[serde(default)]
    pub market_structure: String,
    /// エントリー・利確の価格の根拠（どの水準を使ったか）
    #[serde(default)]
    pub price_levels: String,
    /// どこを抜けたらこの読みは間違いになるか（SLの根拠）
    #[serde(default)]
    pub invalidation: String,
    /// シナリオに反する材料（必ず1つ以上）
    #[serde(default)]
    pub conflicts: String,
    /// 旧: ファンダメンタルズの読み。保存済みの判断を読むためだけに残す
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub fundamental_view: String,
}

/// 既存の予約注文・建玉に対する操作の対象
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ManageTarget {
    Order,
    Position,
}

/// 既存の予約注文・建玉に対する操作
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ManageOp {
    /// 予約注文の取消
    Cancel,
    /// 建玉の成行決済
    Close,
    /// 価格・SL/TP・期限の変更（None の項目は変えない）
    Modify,
}

/// 既存の予約注文・建玉をどうするか。ここに挙げなかったものはそのまま残す
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManageAction {
    pub target: ManageTarget,
    /// `account_state` に出ている注文 ID / 建玉 ID
    pub id: String,
    pub op: ManageOp,
    /// 予約注文の新しい価格（MODIFY のときだけ）
    #[serde(default)]
    pub entry_price: Option<f64>,
    #[serde(default)]
    pub stop_loss: Option<f64>,
    #[serde(default)]
    pub take_profit: Option<f64>,
    /// 予約注文の新しい期限（今から何分後か。MODIFY のときだけ）
    #[serde(default)]
    pub expiry_minutes: Option<u32>,
    #[serde(default)]
    pub reason: String,
}

/// LLM が返す意思決定
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradeDecision {
    /// 新しい予約注文を置くなら BUY / SELL、置かないなら HOLD
    pub action: Action,
    pub confidence: f64,
    #[serde(default)]
    pub entry_type: Option<EntryType>,
    /// 予約注文の価格
    #[serde(default)]
    pub entry_price: Option<f64>,
    #[serde(default)]
    pub stop_loss: Option<f64>,
    #[serde(default)]
    pub take_profit: Option<f64>,
    /// 予約注文の有効期限（今から何分後か）。期限までに約定しなければブローカーが取り消す
    #[serde(default)]
    pub expiry_minutes: Option<u32>,
    pub analysis: Analysis,
    /// 既存の予約注文・建玉への操作
    #[serde(default)]
    pub manage: Vec<ManageAction>,
    pub reasoning: String,
}

impl TradeDecision {
    /// 確信度などを正規化する（0.1刻みに丸め、0.0〜1.0の範囲に収める）
    pub fn normalize(&mut self) {
        self.confidence = ((self.confidence * 10.0).round() / 10.0).clamp(0.0, 1.0);
    }

    /// 安全側のフォールバック（HOLD・既存の注文と建玉はそのまま）
    pub fn fallback_hold(reason: impl Into<String>) -> Self {
        let reason = reason.into();
        Self {
            action: Action::Hold,
            confidence: 0.0,
            entry_type: None,
            entry_price: None,
            stop_loss: None,
            take_profit: None,
            expiry_minutes: None,
            analysis: Analysis {
                market_structure: "N/A (system fallback)".into(),
                price_levels: "N/A (system fallback)".into(),
                invalidation: "N/A".into(),
                conflicts: "N/A".into(),
                fundamental_view: String::new(),
            },
            manage: Vec::new(),
            reasoning: format!("FALLBACK_HOLD: {reason}"),
        }
    }

    /// `claude -p --json-schema` に渡す JSON Schema
    pub fn json_schema() -> serde_json::Value {
        let price = serde_json::json!({ "type": ["number", "null"] });
        serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["action", "confidence", "analysis", "manage", "reasoning"],
            "properties": {
                "action": { "type": "string", "enum": ["BUY", "SELL", "HOLD"] },
                "confidence": {
                    "type": "number",
                    "minimum": 0,
                    "maximum": 1,
                    "description": "新しい予約注文のシナリオに対する確信度（0.0〜1.0、0.1刻み）"
                },
                "entry_type": { "type": ["string", "null"], "enum": ["LIMIT", "STOP", null] },
                "entry_price": price,
                "stop_loss": price,
                "take_profit": price,
                "expiry_minutes": { "type": ["integer", "null"], "minimum": 1 },
                "analysis": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["market_structure", "price_levels", "invalidation", "conflicts"],
                    "properties": {
                        "market_structure": { "type": "string" },
                        "price_levels": { "type": "string" },
                        "invalidation": { "type": "string" },
                        "conflicts": { "type": "string" }
                    }
                },
                "manage": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "required": ["target", "id", "op", "reason"],
                        "properties": {
                            "target": { "type": "string", "enum": ["ORDER", "POSITION"] },
                            "id": { "type": "string" },
                            "op": { "type": "string", "enum": ["CANCEL", "CLOSE", "MODIFY"] },
                            "entry_price": price,
                            "stop_loss": price,
                            "take_profit": price,
                            "expiry_minutes": { "type": ["integer", "null"], "minimum": 1 },
                            "reason": { "type": "string" }
                        }
                    }
                },
                "reasoning": { "type": "string" }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_saved_decisions_still_deserialize() {
        // 旧スキーマ（ファンダメンタルズ・条件付きプラン・観測整合）で保存したリプレイ結果
        let old = r#"{"action":"HOLD","confidence":0.4,"entry_type":"CONDITIONAL",
            "analysis":{"fundamental_view":"f","price_levels":"p","invalidation":"i","conflicts":"c"},
            "conditional_plan":{"wait_for":"w"},"observed":{},"reasoning":"r"}"#;
        let d: TradeDecision = serde_json::from_str(old).unwrap();
        assert_eq!(d.entry_type, Some(EntryType::Conditional));
        assert_eq!(d.analysis.fundamental_view, "f");
        assert!(d.manage.is_empty());
    }

    #[test]
    fn schema_only_offers_pending_entries() {
        let s = TradeDecision::json_schema();
        let kinds = s["properties"]["entry_type"]["enum"].as_array().unwrap();
        assert!(!kinds.iter().any(|k| k == "MARKET" || k == "CONDITIONAL"));
        assert_eq!(s["properties"]["manage"]["items"]["properties"]["op"]["enum"].as_array().unwrap().len(), 3);
    }
}
