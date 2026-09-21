//! Step 6 PoC: 判定なし Snapshot（画像4枚 + 生OHLC + 客観数値）の生成と、
//! `claude -p` への画像パス渡し推論の疎通確認。
//!
//! 実行: `cargo run --bin poc_price_action` （`make step4`）
//! - .env に cTrader 資格情報があれば実データ、無ければ擬似データ
//! - `claude` CLI があれば推論まで実行し、observed の整合をチェックする
//! - `NTRADE_SKIP_LLM=1` で Snapshot 生成のみ（画像の確認用）

use anyhow::Result;
use chrono::Utc;
use std::path::PathBuf;
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use ntrade::ctrader::{BarPeriod, CandleBar, CTraderConfig, CTraderService};
use ntrade::llm::{LlmClient, LlmClientConfig};
use ntrade::snapshot::{mock, AccountState, SnapshotInput, SnapshotPipeline};
use ntrade::strategy::PromptBuilder;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ntrade=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    println!("============================================================");
    println!("  ntrade Step 6: 判定なし Snapshot 生成 & 画像パス渡し推論 PoC");
    println!("============================================================");

    let pair = "USDJPY";
    let spread_pips = 0.2;

    let (bars_4h, bars_1h, bars_15m, bars_5m, source) = fetch_or_simulate_bars(pair).await?;
    println!("\n[1/4] バーデータ ({source}):");
    println!("  4H {} 本 / 1H {} 本 / 15M {} 本 / 5M {} 本", bars_4h.len(), bars_1h.len(), bars_15m.len(), bars_5m.len());

    println!("\n[2/4] Snapshot 生成（画像4枚 + 客観的事実）...");
    let pipeline = SnapshotPipeline::new(PathBuf::from("charts"));
    let bundle = pipeline.build(&SnapshotInput {
        pair,
        bars_4h: &bars_4h,
        bars_1h: &bars_1h,
        bars_15m: &bars_15m,
        bars_5m: &bars_5m,
        spread_pips,
        current_price: None,
        now: None,
        account_state: AccountState::default(),
    })?;
    for (tf, p) in bundle.charts.ordered() {
        let size = std::fs::metadata(p).map(|m| m.len() / 1024).unwrap_or(0);
        println!("  ✓ {:<3} {:?} ({} KB)", tf, p, size);
    }

    println!("\n[3/4] 客観的事実 (JSON):");
    println!("------------------------------------------------------------");
    println!("{}", bundle.snapshot.to_json_pretty());
    println!("------------------------------------------------------------");

    println!("\n[4/4] プロンプト構築 & (オプション) LLM推論:");
    let prompt = PromptBuilder::new()
        .with_lessons(vec![
            "ロンドン開場直後の初動ブレイクは反転しやすい。追認の足を待って読む。".to_string(),
        ])
        .build(&bundle.snapshot, &bundle.charts);
    println!("  ✓ プロンプト {} 文字", prompt.chars().count());

    if std::env::var("NTRADE_SKIP_LLM").is_ok() {
        println!("  (NTRADE_SKIP_LLM が設定されているため推論はスキップ)");
        return Ok(());
    }
    let has_claude = std::process::Command::new("claude").arg("--version").output().is_ok();
    if !has_claude {
        println!("  (claude CLI が見つからないため推論はスキップ)");
        return Ok(());
    }

    println!("  claude -p で推論中（画像4枚を Read で読ませる）...");
    let client = LlmClient::new(LlmClientConfig::default());
    let started = std::time::Instant::now();
    let decision = client.infer(&prompt).await?;
    println!("  所要時間: {:.1?}", started.elapsed());

    println!("\n  === TradeDecision ===");
    println!("  Action:      {:?}", decision.action);
    println!("  Confidence:  {:.2}", decision.confidence);
    println!("  Entry type:  {:?}", decision.entry_type);
    println!("  Entry/SL/TP: {:?} / {:?} / {:?}", decision.entry_price, decision.stop_loss, decision.take_profit);
    println!("  Macro:       {}", decision.analysis.macro_context);
    println!("  Order flow:  {}", decision.analysis.order_flow);
    println!("  Invalidation:{}", decision.analysis.invalidation);
    println!("  Conflicts:   {}", decision.analysis.conflicts);
    if let Some(plan) = &decision.conditional_plan {
        println!("  Plan:        wait_for={} | then={:?} | invalidate_if={} | expires={}",
            plan.wait_for, plan.then_action, plan.invalidate_if, plan.expires_at);
    }
    println!("  Reasoning:   {}", decision.reasoning);

    // 観測整合: LLM が報告した最新足時刻と Snapshot の最新足時刻を突き合わせる
    let lb = &bundle.snapshot.latest_bars;
    let ob = &decision.observed;
    let checks = [
        ("4H", lb.h4.as_deref(), ob.latest_bar_4h.as_deref()),
        ("1H", lb.h1.as_deref(), ob.latest_bar_1h.as_deref()),
        ("15M", lb.m15.as_deref(), ob.latest_bar_15m.as_deref()),
        ("5M", lb.m5.as_deref(), ob.latest_bar_5m.as_deref()),
    ];
    println!("\n  === 観測整合チェック (observed vs latest_bars) ===");
    let mut all_ok = true;
    for (tf, expected, got) in checks {
        let ok = expected.is_some() && expected == got;
        all_ok &= ok;
        println!("  {:<3} expected={:?} observed={:?} {}", tf, expected, got, if ok { "OK" } else { "MISMATCH" });
    }
    println!("  => {}", if all_ok { "LLM は4枚とも読んで回答した" } else { "不一致あり: ガードなら HOLD に倒す対象" });

    Ok(())
}

async fn fetch_or_simulate_bars(
    pair: &str,
) -> Result<(Vec<CandleBar>, Vec<CandleBar>, Vec<CandleBar>, Vec<CandleBar>, &'static str)> {
    if let Ok(config) = CTraderConfig::from_env() {
        info!("Found cTrader credentials, attempting live connection...");
        match CTraderService::connect(config).await {
            Ok(service) => {
                let b4h = service.get_trendbars(pair, BarPeriod::H4, 200).await?;
                let b1h = service.get_trendbars(pair, BarPeriod::H1, 200).await?;
                let b15m = service.get_trendbars(pair, BarPeriod::M15, 200).await?;
                let b5m = service.get_trendbars(pair, BarPeriod::M5, 200).await?;
                return Ok((b4h, b1h, b15m, b5m, "cTrader 実データ"));
            }
            Err(e) => warn!("cTrader connection failed ({:?}), falling back to simulation.", e),
        }
    }
    let (a, b, c, d) = mock::simulate_multi_timeframe(Utc::now(), 154.0);
    Ok((a, b, c, d, "擬似データ"))
}
