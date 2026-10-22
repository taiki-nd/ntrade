use chrono::{Duration, TimeZone, Utc};
use ntrade::ctrader::CandleBar;
use ntrade::llm::{LlmClient, LlmClientConfig};
use ntrade::snapshot::{AccountState, SnapshotInput, SnapshotPipeline};
use ntrade::strategy::types::*;
use ntrade::strategy::PromptBuilder;

fn bars(count: usize, step_min: i64, end: chrono::DateTime<Utc>) -> Vec<CandleBar> {
    (0..count)
        .map(|i| {
            let p = 154.0 + ((i as f64) * 0.3).sin() * 0.2;
            CandleBar {
                timestamp: end - Duration::minutes(step_min * (count as i64 - 1 - i as i64)),
                open: p,
                high: p + 0.1,
                low: p - 0.1,
                close: p + 0.03,
                volume: 10,
            }
        })
        .collect()
}

#[test]
fn prompt_carries_price_facts_without_chart_images_or_judgment_labels() {
    let now = Utc.with_ymd_and_hms(2026, 9, 15, 9, 35, 0).unwrap();
    let (b4, b1, b15, b5) = (bars(60, 240, now), bars(60, 60, now), bars(60, 15, now), bars(60, 5, now));
    let root = std::env::temp_dir().join(format!("ntrade_prompt_test_{}", std::process::id()));

    let bundle = SnapshotPipeline::new(&root)
        .build(&SnapshotInput {
            pair: "USDJPY",
            bars_4h: &b4,
            bars_1h: &b1,
            bars_15m: &b15,
            bars_5m: &b5,
            spread_pips: Some(0.2),
            current_price: None,
            now: Some(now),
            account_state: AccountState::default(),
        })
        .unwrap();

    let prompt = PromptBuilder::new()
        .with_lessons(vec!["テスト教訓".to_string()])
        .build(&bundle.snapshot);

    // チャート画像は LLM に渡さない（価格の数値だけを渡す）
    for (_, p) in bundle.charts.ordered() {
        let abs = std::fs::canonicalize(p).unwrap();
        assert!(!prompt.contains(abs.to_str().unwrap()), "prompt must not reference {:?}", abs);
    }
    assert!(prompt.contains("USDJPY"));
    assert!(prompt.contains("テスト教訓"));
    assert!(prompt.contains("\"bars_5m\"") && prompt.contains("\"bars_1h\""));
    for forbidden in ["PINBAR", "pattern_detected", "at_key_level", "必ず「HOLD"] {
        assert!(!prompt.contains(forbidden), "prompt leaks old judgment: {forbidden}");
    }

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn parses_structured_output_envelope() {
    let client = LlmClient::new(LlmClientConfig::default());
    let raw = r#"{"type":"result","subtype":"success","is_error":false,"result":"{...}","structured_output":{
        "action":"SELL","confidence":0.7,"entry_type":"LIMIT","entry_price":154.45,"stop_loss":154.62,"take_profit":154.05,
        "expiry_minutes":90,
        "analysis":{"market_structure":"1H下落中の戻り","price_levels":"前日安値154.45の戻り売り","invalidation":"154.62超え","conflicts":"アジア安値が近い"},
        "manage":[{"target":"ORDER","id":"555","op":"CANCEL","reason":"買いシナリオ崩れ"},
                  {"target":"POSITION","id":"777","op":"MODIFY","stop_loss":154.30,"take_profit":null,"reason":"建値へ"}],
        "reasoning":"戻り売り"}}"#;

    let d = client.parse_decision(raw);
    assert_eq!(d.action, Action::Sell);
    assert_eq!(d.entry_type, Some(EntryType::Limit));
    assert_eq!(d.expiry_minutes, Some(90));
    assert_eq!(d.manage.len(), 2);
    assert_eq!((d.manage[0].target, d.manage[0].op), (ManageTarget::Order, ManageOp::Cancel));
    assert_eq!(d.manage[1].stop_loss, Some(154.30));
    assert_eq!(d.manage[1].take_profit, None);
}

#[test]
fn parses_markdown_block_fallback() {
    let client = LlmClient::new(LlmClientConfig::default());
    let raw = r#"analysis:
```json
{"action":"BUY","confidence":0.8,"entry_type":"STOP","entry_price":154.2,"stop_loss":154.07,"take_profit":154.5,"expiry_minutes":30,
 "analysis":{"market_structure":"a","price_levels":"b","invalidation":"c","conflicts":"d"},
 "manage":[],"reasoning":"r"}
```"#;
    let d = client.parse_decision(raw);
    assert_eq!(d.action, Action::Buy);
    assert_eq!(d.entry_type, Some(EntryType::Stop));
    assert_eq!(d.stop_loss, Some(154.07));
    assert!(d.manage.is_empty());
}

#[test]
fn error_envelope_and_garbage_fall_back_to_hold() {
    let client = LlmClient::new(LlmClientConfig::default());
    let err = client.parse_decision(r#"{"type":"result","is_error":true,"result":"rate limited"}"#);
    assert_eq!(err.action, Action::Hold);
    assert!(err.reasoning.contains("FALLBACK_HOLD"));

    let garbage = client.parse_decision("no json here");
    assert_eq!(garbage.action, Action::Hold);
}

#[test]
fn json_schema_matches_serialized_decision_shape() {
    let schema = TradeDecision::json_schema();
    let props = schema["properties"].as_object().unwrap();
    for key in ["action", "confidence", "analysis", "manage", "expiry_minutes", "reasoning"] {
        assert!(props.contains_key(key), "schema missing {key}");
    }
    let d = TradeDecision::fallback_hold("x");
    let v = serde_json::to_value(&d).unwrap();
    for key in props.keys() {
        assert!(v.get(key).is_some(), "serialized decision missing {key}");
    }
    assert!(!props.contains_key("observed") && !props.contains_key("conditional_plan"));
    let analysis = schema["properties"]["analysis"]["required"].as_array().unwrap();
    assert!(analysis.iter().any(|k| k == "market_structure") && analysis.iter().any(|k| k == "price_levels"));
    assert!(!analysis.iter().any(|k| k == "fundamental_view"));
}
