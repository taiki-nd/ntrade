use anyhow::Result;
use ntrade::ctrader::{BarPeriod, CTraderConfig, CTraderService};
use tracing::error;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

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
    println!("  ntrade: cTrader Open API 接続 & データ取得 検証ツール (PoC)");
    println!("============================================================");

    // 1. 環境設定の読み込み
    let config = match CTraderConfig::from_env() {
        Ok(cfg) => cfg,
        Err(e) => {
            error!("設定の読み込みに失敗しました: {:?}", e);
            println!("\n[!] .env ファイルが存在しないか、必要な環境変数が不足しています。");
            println!("    .env.example をコピーして .env を作成し、認証情報を設定してください。\n");
            println!("    cp .env.example .env\n");
            return Err(e);
        }
    };

    println!("[1/4] 設定確認:");
    println!("  - Account ID: {}", config.account_id);
    println!("  - Environment: {}", if config.is_live { "LIVE" } else { "DEMO" });
    println!("  - Client ID: {}...", &config.client_id[..config.client_id.len().min(8)]);
    println!();

    // 2. cTrader API への接続と認証
    println!("[2/4] cTrader Open API に接続中...");
    let service = CTraderService::connect(config).await?;
    println!("  ✓ 接続およびアプリケーション/口座認証に成功しました！");
    println!();

    // 3. 主要通貨ペアのシンボル解決
    println!("[3/4] 主要シンボル (USDJPY, EURUSD) の確認:");
    let pairs = ["USDJPY", "EURUSD"];
    for pair in &pairs {
        match service.get_symbol_id(pair).await {
            Ok(id) => println!("  ✓ {} -> Symbol ID: {}", pair, id),
            Err(e) => println!("  ✗ {} -> 取得失敗: {}", pair, e),
        }
    }
    println!();

    // 4. マルチタイムフレームのバーデータ取得
    println!("[4/4] 直近バーデータ（マルチタイムフレーム）の取得テスト:");
    let test_periods = [
        BarPeriod::M5,
        BarPeriod::M15,
        BarPeriod::H1,
        BarPeriod::H4,
    ];

    for pair in &pairs {
        println!("------------------------------------------------------------");
        println!("  通貨ペア: {}", pair);
        println!("------------------------------------------------------------");

        for period in &test_periods {
            match service.get_trendbars(pair, *period, 5).await {
                Ok(bars) => {
                    println!(
                        "  [{}] 取得成功: {} 本",
                        period.as_str(),
                        bars.len()
                    );
                    if let Some(latest) = bars.last() {
                        println!(
                            "      最新足: {} | O: {:.5} | H: {:.5} | L: {:.5} | C: {:.5} | Vol: {}",
                            latest.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
                            latest.open,
                            latest.high,
                            latest.low,
                            latest.close,
                            latest.volume
                        );
                    }
                }
                Err(e) => {
                    println!("  [{}] 取得エラー: {}", period.as_str(), e);
                }
            }
        }
        println!();
    }

    println!("============================================================");
    println!("  cTrader Open API 接続・データ取得検証 完了！");
    println!("============================================================");

    Ok(())
}
