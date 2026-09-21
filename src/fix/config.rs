//! cTrader FIX API の接続設定
//!
//! 認証情報は cTrader プラットフォームの「設定 → FIX API」から取得する。

use anyhow::{Context, Result};
use std::env;

/// FIX セッションの接続設定
#[derive(Debug, Clone, PartialEq)]
pub struct FixConfig {
    /// cServer のホスト名
    pub host: String,
    /// 取引セッションのポート（SSL: 5212 / 平文: 5202 が既定）
    pub port: u16,
    /// TLS を使うか
    pub use_tls: bool,
    /// "<Environment>.<BrokerUID>.<TraderLogin>" 形式（例: demo.axiory.8069118）
    pub sender_comp_id: String,
    /// 数値のトレーダーログイン
    pub username: String,
    pub password: String,
    /// Heartbeat 間隔（秒）。0 なら Heartbeat 不要
    pub heartbeat_secs: u32,
}

/// 取引セッションを表す TargetSubID(57)
pub const TARGET_SUB_TRADE: &str = "TRADE";
/// TargetCompID(56) は常に CSERVER
pub const TARGET_COMP_ID: &str = "CSERVER";

impl FixConfig {
    /// 環境変数から読み込む。FIX_* が未設定なら None（FIX 発注は無効）
    ///
    /// - `CTRADER_FIX_HOST`
    /// - `CTRADER_FIX_PORT`（既定: 5212）
    /// - `CTRADER_FIX_TLS`（既定: true）
    /// - `CTRADER_FIX_SENDER_COMP_ID`
    /// - `CTRADER_FIX_USERNAME`
    /// - `CTRADER_FIX_PASSWORD`
    /// - `CTRADER_FIX_HEARTBEAT`（既定: 30）
    pub fn from_env() -> Result<Option<Self>> {
        let _ = dotenvy::dotenv();
        let Ok(host) = env::var("CTRADER_FIX_HOST") else {
            return Ok(None);
        };
        if host.is_empty() {
            return Ok(None);
        }
        let port = match env::var("CTRADER_FIX_PORT") {
            Ok(p) => p.parse().context("CTRADER_FIX_PORT must be a port number")?,
            Err(_) => 5212,
        };
        let use_tls = env::var("CTRADER_FIX_TLS").map(|v| !matches!(v.to_lowercase().as_str(), "0" | "false" | "no")).unwrap_or(true);
        let heartbeat_secs = match env::var("CTRADER_FIX_HEARTBEAT") {
            Ok(h) => h.parse().context("CTRADER_FIX_HEARTBEAT must be a number of seconds")?,
            Err(_) => 30,
        };
        Ok(Some(Self {
            host,
            port,
            use_tls,
            sender_comp_id: env::var("CTRADER_FIX_SENDER_COMP_ID")
                .context("CTRADER_FIX_SENDER_COMP_ID must be set when CTRADER_FIX_HOST is set")?,
            username: env::var("CTRADER_FIX_USERNAME")
                .context("CTRADER_FIX_USERNAME must be set when CTRADER_FIX_HOST is set")?,
            password: env::var("CTRADER_FIX_PASSWORD")
                .context("CTRADER_FIX_PASSWORD must be set when CTRADER_FIX_HOST is set")?,
            heartbeat_secs,
        }))
    }

    /// SenderCompID の環境部分（demo / live）
    pub fn environment(&self) -> &str {
        self.sender_comp_id.split('.').next().unwrap_or_default()
    }

    pub fn is_live(&self) -> bool {
        self.environment().eq_ignore_ascii_case("live")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(sender: &str) -> FixConfig {
        FixConfig {
            host: "h".into(),
            port: 5212,
            use_tls: true,
            sender_comp_id: sender.into(),
            username: "8069118".into(),
            password: "p".into(),
            heartbeat_secs: 30,
        }
    }

    #[test]
    fn environment_is_derived_from_sender_comp_id() {
        assert_eq!(cfg("demo.axiory.8069118").environment(), "demo");
        assert!(!cfg("demo.axiory.8069118").is_live());
        assert_eq!(cfg("live.theBroker.12345").environment(), "live");
        assert!(cfg("live.theBroker.12345").is_live());
    }
}
