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
fn prompt_references_all_four_images_and_contains_no_judgment_labels() {
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
            spread_pips: 0.2,
            current_price: None,
            now: Some(now),
            account_state: AccountState::default(),
        })
        .unwrap();

    let prompt = PromptBuilder::new()
        .with_lessons(vec!["テスト教訓".to_string()])
        .build(&bundle.snapshot, &bundle.charts);

    for (_, p) in bundle.charts.ordered() {
        let abs = std::fs::canonicalize(p).unwrap();
        assert!(prompt.contains(abs.to_str().unwrap()), "prompt must reference {:?}", abs);
    }
    assert!(prompt.contains("USDJPY"));
    assert!(prompt.contains("テスト教訓"));
    assert!(prompt.contains("\"bars_5m\""));
    for forbidden in ["PINBAR", "pattern_detected", "at_key_level", "必ず「HOLD"] {
        assert!(!prompt.contains(forbidden), "prompt leaks old judgment: {forbidden}");
    }

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn parses_structured_output_envelope() {
    let client = LlmClient::new(LlmClientConfig::default());
    let raw = r#"{"type":"result","subtype":"success","is_error":false,"result":"{...}","structured_output":{
        "action":"HOLD","confidence":0.4,"entry_type":"CONDITIONAL","entry_price":null,"stop_loss":null,"take_profit":null,
        "analysis":{"macro_context":"4H上昇の押し目","order_flow":"5Mで下げ止まり","invalidation":"154.07割れ","conflicts":"低ボラ"},
        "conditional_plan":{"wait_for":"154.24実体抜け","then_action":"BUY","invalidate_if":"154.07割れ","expires_at":"2026-09-15 10:30:00 UTC","stop_loss":154.07,"take_profit":154.5},
        "observed":{"latest_bar_4h":"2026-09-15 08:00","latest_bar_1h":"2026-09-15 09:00","latest_bar_15m":"2026-09-15 09:30","latest_bar_5m":"2026-09-15 09:35"},
        "reasoning":"待ち"}}"#;

    let d = client.parse_decision(raw);
    assert_eq!(d.action, Action::Hold);
    assert_eq!(d.entry_type, Some(EntryType::Conditional));
    let plan = d.conditional_plan.expect("plan");
    assert_eq!(plan.then_action, Action::Buy);
    assert_eq!(plan.stop_loss, Some(154.07));
    assert_eq!(d.observed.latest_bar_5m.as_deref(), Some("2026-09-15 09:35"));
}

#[test]
fn parses_markdown_block_fallback() {
    let client = LlmClient::new(LlmClientConfig::default());
    let raw = r#"analysis:
```json
{"action":"BUY","confidence":0.8,"entry_price":154.2,"stop_loss":154.07,"take_profit":154.5,
 "analysis":{"macro_context":"a","order_flow":"b","invalidation":"c","conflicts":"d"},
 "observed":{"latest_bar_5m":"2026-09-15 09:35"},"reasoning":"r"}
```"#;
    let d = client.parse_decision(raw);
    assert_eq!(d.action, Action::Buy);
    assert_eq!(d.stop_loss, Some(154.07));
    assert!(d.observed.latest_bar_4h.is_none());
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
    for key in ["action", "confidence", "analysis", "conditional_plan", "observed", "reasoning"] {
        assert!(props.contains_key(key), "schema missing {key}");
    }
    let d = TradeDecision::fallback_hold("x");
    let v = serde_json::to_value(&d).unwrap();
    for key in props.keys() {
        assert!(v.get(key).is_some(), "serialized decision missing {key}");
    }
}
