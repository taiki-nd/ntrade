use anyhow::Result;
use chrono::{Duration, Utc};
use std::path::PathBuf;
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use ntrade::chart::ChartPlotterConfig;
use ntrade::ctrader::{BarPeriod, CandleBar, CTraderConfig, CTraderService};
use ntrade::llm::{LlmClient, LlmClientConfig};
use ntrade::strategy::{PriceActionPipeline, PromptBuilder};

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
    println!("  ntrade Step 4: PA特徴量抽出 & plotters 4分割チャート描画検証");
    println!("============================================================");

    let pair = "USDJPY";
    let spread_pips = 0.2;
    let output_dir = PathBuf::from("charts");

    // 1. 相場データの取得（cTrader または 高精度シミュレーション）
    let (bars_4h, bars_1h, bars_15m, bars_5m) = fetch_or_simulate_bars(pair).await?;

    println!("\n[1/4] バーデータ準備完了:");
    println!("  - 4H  足: {} 本 (大局環境認識)", bars_4h.len());
    println!("  - 1H  足: {} 本 (スイング構造 & EMA)", bars_1h.len());
    println!("  - 15M 足: {} 本 (セットアップ & プルバック)", bars_15m.len());
    println!("  - 5M  足: {} 本 (直近エントリートリガー)", bars_5m.len());

    // 2. パイプライン実行（PA特徴量抽出 + 4分割チャートPNG生成）
    println!("\n[2/4] プライスアクション特徴量抽出 & 4分割チャート生成中...");
    let pipeline = PriceActionPipeline::with_plotter_config(
        &output_dir,
        ChartPlotterConfig {
            width: 1600,
            height: 1200,
            bars_to_display: 45,
            dark_mode: true,
        },
    );

    let bundle = pipeline.process(pair, &bars_4h, &bars_1h, &bars_15m, &bars_5m, spread_pips)?;

    println!("  ✓ 特徴量抽出 完了！");
    println!("  ✓ 4分割チャートPNG生成 完了: {:?}", bundle.chart_path);
    if bundle.chart_path.exists() {
        let meta = std::fs::metadata(&bundle.chart_path)?;
        println!("    (ファイルサイズ: {} KB)", meta.len() / 1024);
    }

    // 3. 抽出されたプライスアクション特徴量の表示
    println!("\n[3/4] 抽出されたプライスアクション特徴量 (JSON):");
    println!("------------------------------------------------------------");
    let input_json = serde_json::to_string_pretty(&bundle.input)?;
    println!("{}", input_json);
    println!("------------------------------------------------------------");

    println!("\n【5M足の幾何学的特徴量サマリー】");
    let cur = &bundle.input.five_minute_pa.current_bar;
    println!("  ・ローソク足タイプ : {}", cur.bar_type);
    println!("  ・方向 (Direction) : {}", cur.direction);
    println!("  ・全レンジ (Pips)  : {:.1} pips", cur.total_range_pips);
    println!("  ・実体比率 (Body)  : {:.1}%", cur.body_ratio * 100.0);
    println!("  ・下ヒゲ比率 (Lower): {:.1}%", cur.lower_wick_ratio * 100.0);
    println!("  ・上ヒゲ比率 (Upper): {:.1}%", cur.upper_wick_ratio * 100.0);
    if let Some(rej) = cur.rejection_level {
        println!("  ・拒絶価格 (Rejection): {:.3}", rej);
    }
    println!("  ・検出パターン     : {}", bundle.input.five_minute_pa.pattern_detected);
    println!("  ・キーレベル状態   : {}", bundle.input.five_minute_pa.at_key_level);

    // 4. プロンプトの生成
    println!("\n[4/4] LLMプロンプトの構築 & (オプション)推論テスト:");
    let lessons = vec![
        "4H上位足が上昇トレンドの局面では、5M逆張りショートは厳禁。".to_string(),
        "欧州ロンドンオープン直後の初動ブレイクはダマシが多いため、プルバックを待つこと。".to_string(),
    ];
    let prompt_builder = PromptBuilder::new().with_lessons(lessons);
    let prompt = prompt_builder.build_inference_prompt(&bundle.input);
    println!("  ✓ プロンプト生成成功 (文字数: {} chars)", prompt.len());

    // CLI推論の疎通確認（claude または agy が存在すれば実行、なければスキップ）
    let cli_tool = if std::process::Command::new("claude").arg("--version").output().is_ok() {
        Some("claude")
    } else if std::process::Command::new("agy").arg("--version").output().is_ok() {
        Some("agy")
    } else {
        None
    };

    if let Some(tool) = cli_tool {
        println!("\n  [+] ローカルCLI '{}' が検出されました。LLM推論をテストします...", tool);
        let client = LlmClient::new(LlmClientConfig {
            cli_binary: tool.to_string(),
            timeout_secs: 45,
        });

        match client.infer(&prompt).await {
            Ok(decision) => {
                println!("\n  === LLM 意思決定結果 ===");
                println!("  Action:            {:?}", decision.action);
                println!("  Confidence:        {:.2}", decision.confidence);
                if let Some(entry) = decision.entry_price {
                    println!("  Entry Price:       {:.3}", entry);
                }
                if let Some(sl) = decision.stop_loss {
                    println!("  Stop Loss (SL):    {:.3}", sl);
                }
                if let Some(tp) = decision.take_profit {
                    println!("  Take Profit (TP):  {:.3}", tp);
                }
                if let Some(rr) = decision.risk_reward_ratio {
                    println!("  Risk:Reward Ratio: 1 : {:.2}", rr);
                }
                println!("  Reasoning:         {}", decision.reasoning);
            }
            Err(e) => {
                println!("  [!] LLM推論呼び出しスキップまたは失敗 (PoC単体検証は成功): {}", e);
            }
        }
    } else {
        println!("  (※ claude / agy CLIが見つからないため、LLMパイプ呼び出しはスキップします)");
    }

    println!("\n============================================================");
    println!("  Step 4: プライスアクション特徴量 & plotters 描画 検証完了！");
    println!("  生成画像: {:?}", bundle.chart_path);
    println!("============================================================");

    Ok(())
}

/// cTrader からバーデータを取得、接続できない場合はリアルなシミュレーションデータを生成
async fn fetch_or_simulate_bars(
    pair: &str,
) -> Result<(Vec<CandleBar>, Vec<CandleBar>, Vec<CandleBar>, Vec<CandleBar>)> {
    if let Ok(config) = CTraderConfig::from_env() {
        info!("Found cTrader credentials in .env, attempting live connection...");
        match CTraderService::connect(config).await {
            Ok(service) => {
                info!("cTrader connected! Fetching MTF trendbars...");
                let b4h = service.get_trendbars(pair, BarPeriod::H4, 40).await?;
                let b1h = service.get_trendbars(pair, BarPeriod::H1, 40).await?;
                let b15m = service.get_trendbars(pair, BarPeriod::M15, 40).await?;
                let b5m = service.get_trendbars(pair, BarPeriod::M5, 40).await?;
                return Ok((b4h, b1h, b15m, b5m));
            }
            Err(e) => {
                warn!("cTrader connection failed ({:?}), falling back to realistic simulation.", e);
            }
        }
    }

    info!("Generating realistic multi-timeframe price action simulation for USD/JPY...");
    let now = Utc::now();

    // 4H足: 上昇トレンド (Higher Highs / Higher Lows: 152.50 -> 154.50)
    let mut bars_4h = Vec::new();
    let mut p = 152.50;
    for i in 0..35 {
        let open = p;
        let close = open + 0.08 + ((i as f64 * 0.3).sin() * 0.15);
        let high = open.max(close) + 0.20;
        let low = open.min(close) - 0.12;
        p = close;
        bars_4h.push(CandleBar {
            timestamp: now - Duration::hours((35 - i) * 4),
            open,
            high,
            low,
            close,
            volume: 5000,
        });
    }

    // 1H足: 4H上昇の中での押し目形成 (154.80 -> 154.10 EMA20付近へのプルバック)
    let mut bars_1h = Vec::new();
    let mut p = 153.80;
    for i in 0..40 {
        let open = p;
        let diff = if i > 30 { -0.05 } else { 0.04 };
        let close = open + diff + ((i as f64 * 0.2).cos() * 0.08);
        let high = open.max(close) + 0.10;
        let low = open.min(close) - 0.08;
        p = close;
        bars_1h.push(CandleBar {
            timestamp: now - Duration::hours(40 - i),
            open,
            high,
            low,
            close,
            volume: 1500,
        });
    }

    // 15M足: 154.10付近で揉み合い・下げ止まり
    let mut bars_15m = Vec::new();
    let mut p = 154.30;
    for i in 0..45 {
        let open = p;
        let diff = if i > 35 { 0.02 } else { -0.02 };
        let close = open + diff + ((i as f64 * 0.15).sin() * 0.05);
        let high = open.max(close) + 0.06;
        let low = open.min(close) - 0.06;
        p = close;
        bars_15m.push(CandleBar {
            timestamp: now - Duration::minutes((45 - i) * 15),
            open,
            high,
            low,
            close,
            volume: 400,
        });
    }

    // 5M足: 154.12のサポートで下ヒゲが長い強気レジェクション・ピンバーが出現！
    let mut bars_5m = Vec::new();
    let mut p: f64 = 154.25;
    for i in 0..44 {
        let open: f64 = p;
        let diff: f64 = if i > 38 { -0.02 } else { 0.01 };
        let close: f64 = open + diff;
        let high: f64 = open.max(close) + 0.03;
        let low: f64 = open.min(close) - 0.03;
        p = close;
        bars_5m.push(CandleBar {
            timestamp: now - Duration::minutes((45 - i) * 5),
            open,
            high,
            low,
            close,
            volume: 120,
        });
    }

    // 45本目（最新足）: 154.12のサポートを試して下ヒゲ70%のピンバー確定！
    // open: 154.20, low: 154.08, high: 154.24, close: 154.22
    // range: 0.16, lower_wick: 154.20 - 154.08 = 0.12 (75%下ヒゲ!)
    bars_5m.push(CandleBar {
        timestamp: now,
        open: 154.200,
        high: 154.240,
        low: 154.080,
        close: 154.220,
        volume: 380,
    });

    Ok((bars_4h, bars_1h, bars_15m, bars_5m))
}
