use anyhow::Result;
use ntrade::llm::{LlmClient, LlmClientConfig};
use ntrade::strategy::types::{
    CurrentBarFeatures, FiveMinutePriceAction, MarketStructure, PriceActionInput,
};
use ntrade::strategy::PromptBuilder;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ntrade=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("=== Step 1: LLM CLI Inference PoC ===");

    // 1. サンプルのプライスアクション入力データ（USD/JPY ピンバー反発セットアップ）
    let sample_input = PriceActionInput {
        pair: "USDJPY".to_string(),
        timestamp: "2026-09-14 07:35:00".to_string(),
        spread_pips: 0.2,
        current_price: 154.205,
        market_structure: MarketStructure {
            trend_4h: "BULLISH (Higher Highs / Higher Lows)".to_string(),
            nearest_h4_resistance: 155.00,
            nearest_h4_support: 153.80,
            trend_1h: "BULLISH_PULLBACK_TO_EMA20".to_string(),
        },
        five_minute_pa: FiveMinutePriceAction {
            current_bar: CurrentBarFeatures {
                bar_type: "PINBAR".to_string(),
                direction: "BULLISH".to_string(),
                total_range_pips: 8.2,
                lower_wick_ratio: 0.68,
                upper_wick_ratio: 0.10,
                body_ratio: 0.22,
                rejection_level: Some(154.12),
            },
            pattern_detected: "SUPPORT_REJECTION_PINBAR".to_string(),
            at_key_level: "TESTING_15M_PREVIOUS_RESISTANCE_TURNED_SUPPORT".to_string(),
        },
    };

    // 2. 教訓リスト（過去の反省ルール）
    let lessons = vec![
        "4H上位足が上昇トレンドの局面では、5M逆張りショートは厳禁。".to_string(),
        "欧州ロンドンオープン直後の初動ブレイクはダマシが多いため、プルバックを待つこと。".to_string(),
    ];

    // 3. プロンプト生成
    let prompt_builder = PromptBuilder::new().with_lessons(lessons);
    let prompt = prompt_builder.build_inference_prompt(&sample_input);

    info!("Generated prompt length: {} chars", prompt.len());

    // 4. LlmClient を初期化して claude -p で推論実行
    let client = LlmClient::new(LlmClientConfig {
        cli_binary: "claude".to_string(),
        timeout_secs: 60,
    });

    info!("Calling LLM via `claude -p` pipe...");
    let start_time = std::time::Instant::now();
    let decision = client.infer(&prompt).await?;
    let elapsed = start_time.elapsed();

    info!("Inference finished in {:.2?}", elapsed);
    println!("\n==========================================");
    println!("=== TradeDecision Received from LLM ===");
    println!("==========================================");
    println!("Action:            {:?}", decision.action);
    println!("Confidence:        {:.2}", decision.confidence);
    if let Some(entry) = decision.entry_price {
        println!("Entry Price:       {:.3}", entry);
    }
    if let Some(sl) = decision.stop_loss {
        println!("Stop Loss (SL):    {:.3}", sl);
    }
    if let Some(tp) = decision.take_profit {
        println!("Take Profit (TP):  {:.3}", tp);
    }
    if let Some(rr) = decision.risk_reward_ratio {
        println!("Risk:Reward Ratio: 1 : {:.2}", rr);
    }
    println!("\n--- Price Action Analysis ---");
    println!("Macro Bias:        {}", decision.price_action_analysis.macro_bias);
    println!("Trigger Pattern:   {}", decision.price_action_analysis.trigger_pattern);
    println!("Invalidation:      {}", decision.price_action_analysis.invalidation_point);
    println!("\n--- Reasoning ---");
    println!("{}", decision.reasoning);
    println!("==========================================\n");

    assert!(
        decision.confidence >= 0.0 && decision.confidence <= 1.0,
        "Confidence should be between 0.0 and 1.0"
    );

    info!("PoC completed successfully!");
    Ok(())
}
