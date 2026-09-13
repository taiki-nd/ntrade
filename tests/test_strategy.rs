use ntrade::strategy::types::*;
use ntrade::strategy::PromptBuilder;
use ntrade::llm::{LlmClient, LlmClientConfig};

#[test]
fn test_prompt_builder_contains_data_and_lessons() {
    let input = PriceActionInput {
        pair: "USDJPY".to_string(),
        timestamp: "2026-09-14 07:35:00".to_string(),
        spread_pips: 0.2,
        current_price: 154.205,
        market_structure: MarketStructure {
            trend_4h: "BULLISH".to_string(),
            nearest_h4_resistance: 155.0,
            nearest_h4_support: 153.8,
            trend_1h: "BULLISH".to_string(),
        },
        five_minute_pa: FiveMinutePriceAction {
            current_bar: CurrentBarFeatures {
                bar_type: "PINBAR".to_string(),
                direction: "BULLISH".to_string(),
                total_range_pips: 8.0,
                lower_wick_ratio: 0.65,
                upper_wick_ratio: 0.1,
                body_ratio: 0.25,
                rejection_level: Some(154.12),
            },
            pattern_detected: "SUPPORT_REJECTION_PINBAR".to_string(),
            at_key_level: "TESTING_SUPPORT".to_string(),
        },
    };

    let builder = PromptBuilder::new().with_lessons(vec!["テスト教訓ルール".to_string()]);
    let prompt = builder.build_inference_prompt(&input);

    assert!(prompt.contains("USDJPY"));
    assert!(prompt.contains("SUPPORT_REJECTION_PINBAR"));
    assert!(prompt.contains("テスト教訓ルール"));
    assert!(prompt.contains("154.12"));
}

#[test]
fn test_parse_decision_from_markdown_block() {
    let client = LlmClient::new(LlmClientConfig::default());
    let raw = r#"
Here is my analysis:
```json
{
  "action": "BUY",
  "confidence": 0.88,
  "entry_type": "MARKET",
  "entry_price": 154.20,
  "stop_loss": 154.08,
  "take_profit": 154.50,
  "risk_reward_ratio": 2.5,
  "price_action_analysis": {
    "macro_bias": "4Hダウ上昇",
    "trigger_pattern": "5Mピンバー",
    "invalidation_point": "154.08安値割れ"
  },
  "reasoning": "良好なセットアップ"
}
```
Good luck!
"#;

    let decision = client.parse_decision(raw).expect("Parsing should succeed");
    assert_eq!(decision.action, Action::Buy);
    assert_eq!(decision.confidence, 0.88);
    assert_eq!(decision.entry_price, Some(154.20));
    assert_eq!(decision.stop_loss, Some(154.08));
    assert_eq!(decision.take_profit, Some(154.50));
    assert_eq!(decision.risk_reward_ratio, Some(2.5));
    assert_eq!(decision.price_action_analysis.trigger_pattern, "5Mピンバー");
}
