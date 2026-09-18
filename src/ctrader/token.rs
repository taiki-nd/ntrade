//! cTrader OAuth トークン（Access Token / Refresh Token）の取得・更新。

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use serde::Deserialize;

pub const SPOTWARE_TOKEN_URL: &str = "https://openapi.ctrader.com/apps/token";

/// 期限までこの秒数を切ったら更新する（3日）
pub const REFRESH_MARGIN_SECS: i64 = 3 * 24 * 3600;

/// 保存・受け渡し用のトークン一式
#[derive(Debug, Clone, PartialEq)]
pub struct TokenSet {
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// Access Token の失効時刻（unix 秒）。不明なら None
    pub expires_at: Option<i64>,
}

impl TokenSet {
    /// 期限が近い（または不明な）ので更新すべきか
    pub fn needs_refresh(&self, now: i64) -> bool {
        match self.expires_at {
            Some(exp) => exp - now <= REFRESH_MARGIN_SECS,
            None => true,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
    error_code: Option<String>,
    #[serde(alias = "errorDescription")]
    description: Option<String>,
}

/// Refresh Token で Access Token を更新する（Spotware Token API、POST + クエリパラメータ）。
/// 更新すると古い Access Token / Refresh Token は無効になるので、戻り値は必ず保存すること。
pub async fn refresh_tokens(client_id: &str, client_secret: &str, refresh_token: &str) -> Result<TokenSet> {
    let resp = reqwest::Client::new()
        .post(SPOTWARE_TOKEN_URL)
        .query(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", client_id),
            ("client_secret", client_secret),
        ])
        .send()
        .await
        .context("Failed to reach Spotware token endpoint")?;
    let status = resp.status();
    let body: TokenResponse = resp
        .json()
        .await
        .with_context(|| format!("Failed to parse Spotware token response (HTTP {status})"))?;
    token_set_from_response(body, Some(refresh_token))
}

fn token_set_from_response(body: TokenResponse, previous_refresh: Option<&str>) -> Result<TokenSet> {
    if body.error_code.is_some() || body.description.is_some() {
        return Err(anyhow!(
            "Spotware token error: {} {}",
            body.error_code.unwrap_or_default(),
            body.description.unwrap_or_default()
        ));
    }
    let access_token = body
        .access_token
        .filter(|t| !t.is_empty())
        .ok_or_else(|| anyhow!("Spotware token response has no accessToken"))?;
    Ok(TokenSet {
        access_token,
        refresh_token: body
            .refresh_token
            .filter(|t| !t.is_empty())
            .or_else(|| previous_refresh.map(str::to_string)),
        expires_at: body.expires_in.map(|s| Utc::now().timestamp() + s),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_refresh_by_margin() {
        let now = 1_000_000;
        let t = |exp| TokenSet { access_token: "a".into(), refresh_token: None, expires_at: exp };
        assert!(t(None).needs_refresh(now));
        assert!(t(Some(now + REFRESH_MARGIN_SECS - 1)).needs_refresh(now));
        assert!(!t(Some(now + REFRESH_MARGIN_SECS + 60)).needs_refresh(now));
    }

    #[test]
    fn parses_success_and_error() {
        let ok: TokenResponse = serde_json::from_str(
            r#"{"accessToken":"new","tokenType":"bearer","expiresIn":2628000,"refreshToken":"r2","errorCode":null,"description":null}"#,
        )
        .unwrap();
        let set = token_set_from_response(ok, Some("r1")).unwrap();
        assert_eq!(set.access_token, "new");
        assert_eq!(set.refresh_token.as_deref(), Some("r2"));
        assert!(set.expires_at.is_some());

        let err: TokenResponse =
            serde_json::from_str(r#"{"errorCode":"ACCESS_DENIED","description":"bad refresh token"}"#).unwrap();
        assert!(token_set_from_response(err, Some("r1")).is_err());
    }
}
