use anyhow::Result;
use ntrade::ctrader::{BarPeriod, CTraderConfig, CTraderService, TradingPermission};
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

    // 2.5 口座状態と取引権限の確認（建玉は作らない）
    match service.get_account_info().await {
        Ok(info) => println!(
            "  口座: #{} | 残高: {:.2} | レバレッジ: {} | 取引権限: {}",
            info.trader_login.unwrap_or(info.account_id),
            info.balance,
            info.leverage.map(|l| format!("1:{l:.0}")).unwrap_or_else(|| "不明".into()),
            info.access_rights.as_deref().unwrap_or("不明")
        ),
        Err(e) => println!("  口座情報の取得に失敗: {e:#}"),
    }
    println!();

    println!("[2.5] Access Token の取引権限を確認中（発注はしません）...");
    match service.probe_trading_permission().await {
        Ok(TradingPermission::Granted { error_code }) => {
            println!("  ・TRADING_DISABLED は返りませんでした（打診への応答: {error_code}）");
            println!("    ※ 存在しない建玉への打診のため、取引可能であることの確証ではありません");
        }
        Ok(TradingPermission::Denied { error_code, description }) => {
            println!("  ✗ 取引権限がありません: {error_code} ({description})");
            println!("    → 設定画面から再認証し、cTrader の認可画面で scope に『Trading』を選択してください。");
            println!("    → 認可時は対象口座（CTRADER_ACCOUNT_ID）にも許可を与える必要があります。");
        }
        Ok(TradingPermission::Unknown(e)) => println!("  ? 判定できませんでした: {e}"),
        Err(e) => println!("  ? 判定に失敗しました: {e:#}"),
    }
    println!();

    // 3. 主要通貨ペアのシンボル解決
    println!("[3/4] 主要シンボル (USDJPY, EURUSD) の確認:");
    let pairs = ["USDJPY", "EURUSD"];
    for pair in &pairs {
        match service.get_symbol_id(pair).await {
            Ok(id) => {
                let mode = match service.get_symbol_trading_mode(pair).await {
                    Ok(m) => m,
                    Err(e) => format!("取得失敗: {e}"),
                };
                println!("  ✓ {} -> Symbol ID: {} | 取引モード: {}", pair, id, mode);
            }
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

    // 5. 発注経路の検証（--test-order 指定時のみ。実際に建玉が立つ）
    if std::env::args().any(|a| a == "--test-order") {
        println!("[5/5] テスト発注（USDJPY 0.01 lot BUY / SL・TP なし）:");
        let volume = ntrade::ctrader::lots_to_volume(0.01);
        match service.place_market_order_with_sltp("USDJPY", true, volume, None, None).await {
            Ok(ev) => {
                let position_id = ev
                    .position
                    .as_ref()
                    .map(|p| p.position_id)
                    .or_else(|| ev.deal.as_ref().map(|d| d.position_id));
                println!("  ✓ 発注成功: position_id={position_id:?}");
                // 検証用の建玉なので即座に決済する
                if let Some(id) = position_id {
                    println!("  決済中...");
                    match service.close_position_by_id(id, volume).await {
                        Ok(_) => println!("  ✓ 決済完了 (position_id={id})"),
                        Err(e) => println!("  ✗ 決済に失敗しました。cTrader アプリで手動決済してください: {e:#}"),
                    }
                }
            }
            Err(e) => println!("  ✗ 発注失敗: {e:#}"),
        }
        println!();
    }

    println!("============================================================");
    println!("  cTrader Open API 接続・データ取得検証 完了！");
    println!("============================================================");

    Ok(())
}
