use anyhow::{Context, Result};
use std::env;

/// cTrader Open API 接続設定
#[derive(Debug, Clone)]
pub struct CTraderConfig {
    pub client_id: String,
    pub client_secret: String,
    pub account_id: i64,
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// Access Token の失効時刻（unix 秒）。.env 由来で不明なら None
    pub token_expires_at: Option<i64>,
    pub is_live: bool,
}

impl CTraderConfig {
    /// 環境変数（または .env ファイル）から設定を読み込む
    pub fn from_env() -> Result<Self> {
        let _ = dotenvy::dotenv();

        let client_id = env::var("CTRADER_CLIENT_ID")
            .context("Environment variable CTRADER_CLIENT_ID must be set")?;
        let client_secret = env::var("CTRADER_CLIENT_SECRET")
            .context("Environment variable CTRADER_CLIENT_SECRET must be set")?;
        let account_id_str = env::var("CTRADER_ACCOUNT_ID")
            .context("Environment variable CTRADER_ACCOUNT_ID must be set")?;
        let account_id = account_id_str
            .parse::<i64>()
            .with_context(|| format!("Failed to parse CTRADER_ACCOUNT_ID '{}' as i64", account_id_str))?;
        let access_token = env::var("CTRADER_ACCESS_TOKEN")
            .context("Environment variable CTRADER_ACCESS_TOKEN must be set")?;
        let refresh_token = env::var("CTRADER_REFRESH_TOKEN").ok().filter(|t| !t.trim().is_empty());

        let env_mode = env::var("CTRADER_ENV").unwrap_or_else(|_| "DEMO".to_string());
        let is_live = env_mode.to_uppercase() == "LIVE";

        Ok(Self {
            client_id,
            client_secret,
            account_id,
            access_token,
            refresh_token,
            token_expires_at: None,
            is_live,
        })
    }

    /// 保存済みトークンで上書きする
    pub fn with_tokens(mut self, tokens: &super::TokenSet) -> Self {
        self.access_token = tokens.access_token.clone();
        if tokens.refresh_token.is_some() {
            self.refresh_token = tokens.refresh_token.clone();
        }
        self.token_expires_at = tokens.expires_at;
        self
    }

    pub fn tokens(&self) -> super::TokenSet {
        super::TokenSet {
            access_token: self.access_token.clone(),
            refresh_token: self.refresh_token.clone(),
            expires_at: self.token_expires_at,
        }
    }
}
