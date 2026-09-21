//! cTrader FIX API の接続・発注検証ツール
//!
//! 認証情報は cTrader の「設定 → FIX API」から取得し、`.env` の `CTRADER_FIX_*` に設定する。
//!
//! ```text
//! cargo run --bin poc_fix                  # 接続と Logon のみ
//! cargo run --bin poc_fix -- --test-order  # 0.01 lot の成行 + SL/TP を張って即決済
//! ```

use anyhow::{Context, Result};
use ntrade::ctrader::{CTraderConfig, CTraderService};
use ntrade::fix::{lots_to_units, FixConfig, FixTradingClient};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "ntrade=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    println!("============================================================");
    println!("  ntrade: cTrader FIX API 接続 & 発注 検証ツール");
    println!("============================================================");

    let fix_config = FixConfig::from_env()?.context(
        "CTRADER_FIX_HOST が設定されていません。cTrader の「設定 → FIX API」から認証情報を取得し、\n\
         .env.example の CTRADER_FIX_* を参考に .env へ設定してください。",
    )?;

    println!("[1/4] FIX 設定:");
    println!("  - Host: {}:{} (TLS={})", fix_config.host, fix_config.port, fix_config.use_tls);
    println!("  - SenderCompID: {}", fix_config.sender_comp_id);
    println!("  - Environment: {}", if fix_config.is_live() { "LIVE" } else { "DEMO" });
    println!();

    println!("[2/4] FIX セッションに接続中...");
    let fix = FixTradingClient::connect(fix_config).await?;
    println!("  ✓ Logon 成功");
    println!();

    // シンボル ID の解決は Open API 側のキャッシュを使う
    println!("[3/4] シンボル ID を Open API から解決中...");
    let ctrader = CTraderService::connect(CTraderConfig::from_env()?).await?;
    let symbol_id = ctrader.get_symbol_id("USDJPY").await?;
    println!("  ✓ USDJPY -> Symbol ID: {symbol_id}");
    println!();

    if std::env::args().any(|a| a == "--test-order") {
        println!("[4/4] テスト発注（USDJPY 0.01 lot BUY + SL 10 pips / TP 20 pips）:");
        let units = lots_to_units(0.01);
        let fill = fix.place_market_order(symbol_id, true, units).await?;
        println!(
            "  ✓ 約定: position_id={} price={:?} units={}",
            fill.position_id, fill.avg_px, fill.filled_units
        );

        if let Some(entry) = fill.avg_px {
            let (sl, tp) = (entry - 0.10, entry + 0.20);
            println!("  保護注文を発注中 (SL={sl:.3} / TP={tp:.3})...");
            match fix
                .attach_protective_orders(&fill.position_id, symbol_id, true, fill.filled_units, Some(sl), Some(tp))
                .await
            {
                Ok(()) => println!("  ✓ SL/TP を設定しました"),
                Err(e) => println!("  ✗ 保護注文の発注に失敗: {e:#}"),
            }
        }

        println!("  決済中（保護注文の取り消しを含む）...");
        match fix.close_position(&fill.position_id, symbol_id, true, fill.filled_units).await {
            Ok(close) => println!("  ✓ 決済完了 (position_id={})", close.position_id),
            Err(e) => println!("  ✗ 決済に失敗しました。cTrader アプリで手動決済してください: {e:#}"),
        }
        println!();
    } else {
        println!("[4/4] テスト発注はスキップ（--test-order で実行）");
        println!();
    }

    fix.logout().await;
    println!("============================================================");
    println!("  cTrader FIX API 検証 完了！");
    println!("============================================================");
    Ok(())
}
