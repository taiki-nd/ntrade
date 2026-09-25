use axum::{extract::State, response::Json};
use serde::Deserialize;
use std::env;
use tracing::{error, info, warn};

use crate::ctrader::{CTraderConfig, CTraderService, TokenSet};
use crate::server::state::AppState;
use crate::server::types::{
    AccountInfo, ApiResponse, OAuthExchangeRequest, OAuthUrlResponse, SelectAccountRequest,
};

const DEFAULT_REDIRECT_URI: &str = "http://localhost:3000/auth/ctrader/callback";
const SPOTWARE_AUTH_URL: &str = "https://id.ctrader.com/my/settings/openapi/grantingaccess/";
use crate::ctrader::token::SPOTWARE_TOKEN_URL;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct SpotwareTokenResponse {
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub token_type: Option<String>,
    pub expires_in: Option<i64>,
    pub error_code: Option<String>,
    #[serde(alias = "errorDescription")]
    pub description: Option<String>,
}

/// GET /api/auth/ctrader/url
/// Spotware OAuth 認可画面へのリダイレクトURLを生成
pub async fn get_oauth_url(State(_state): State<AppState>) -> Json<ApiResponse<OAuthUrlResponse>> {
    let client_id = env::var("CTRADER_CLIENT_ID").unwrap_or_default();
    let redirect_uri = env::var("CTRADER_REDIRECT_URI").unwrap_or_else(|_| DEFAULT_REDIRECT_URI.to_string());

    if client_id.is_empty() {
        return Json(ApiResponse {
            success: false,
            data: None,
            message: Some(".env に CTRADER_CLIENT_ID が設定されていません。".to_string()),
        });
    }

    // scope は "trading"（発注可）か "accounts"（閲覧のみ）のどちらか1つ
    let url = match reqwest::Url::parse_with_params(
        SPOTWARE_AUTH_URL,
        &[("client_id", client_id.as_str()), ("redirect_uri", redirect_uri.as_str()), ("scope", "trading")],
    ) {
        Ok(u) => u.to_string(),
        Err(e) => return Json(ApiResponse::err(format!("認可URLの生成に失敗しました: {e}"))),
    };

    Json(ApiResponse::ok(OAuthUrlResponse {
        url,
        client_id,
        redirect_uri,
    }))
}

/// POST /api/auth/ctrader/exchange
/// 認可コード（code）から Access Token / Refresh Token を取得して自動保存 & 即時接続
pub async fn exchange_oauth_code(
    State(state): State<AppState>,
    Json(payload): Json<OAuthExchangeRequest>,
) -> Json<ApiResponse<Vec<AccountInfo>>> {
    info!("Exchanging cTrader OAuth authorization code for tokens...");

    let client_id = match env::var("CTRADER_CLIENT_ID") {
        Ok(id) if !id.is_empty() => id,
        _ => {
            return Json(ApiResponse {
                success: false,
                data: None,
                message: Some("CTRADER_CLIENT_ID が設定されていません。".to_string()),
            });
        }
    };

    let client_secret = match env::var("CTRADER_CLIENT_SECRET") {
        Ok(sec) if !sec.is_empty() => sec,
        _ => {
            return Json(ApiResponse {
                success: false,
                data: None,
                message: Some("CTRADER_CLIENT_SECRET が設定されていません。".to_string()),
            });
        }
    };

    let redirect_uri = payload
        .redirect_uri
        .unwrap_or_else(|| env::var("CTRADER_REDIRECT_URI").unwrap_or_else(|_| DEFAULT_REDIRECT_URI.to_string()));

    // Spotware Token API へ POST リクエスト
    let http_client = reqwest::Client::new();
    let token_req = [
        ("grant_type", "authorization_code"),
        ("code", &payload.code),
        ("redirect_uri", &redirect_uri),
        ("client_id", &client_id),
        ("client_secret", &client_secret),
    ];

    info!("Sending token request to Spotware: {}", SPOTWARE_TOKEN_URL);
    let resp = match http_client.get(SPOTWARE_TOKEN_URL).query(&token_req).send().await {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to request token from Spotware: {:?}", e);
            return Json(ApiResponse {
                success: false,
                data: None,
                message: Some(format!("Spotware認可サーバーへの接続に失敗しました: {}", e)),
            });
        }
    };

    let token_data: SpotwareTokenResponse = match resp.json().await {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to parse Spotware token response: {:?}", e);
            return Json(ApiResponse {
                success: false,
                data: None,
                message: Some(format!("トークンレスポンスの解析に失敗しました: {}", e)),
            });
        }
    };

    if let Some(err_desc) = token_data.description.or(token_data.error_code) {
        warn!("Spotware returned error: {:?}", err_desc);
        return Json(ApiResponse {
            success: false,
            data: None,
            message: Some(format!("Spotware認証エラー: {}", err_desc)),
        });
    }

    let access_token = match token_data.access_token {
        Some(token) if !token.is_empty() => token,
        _ => {
            return Json(ApiResponse {
                success: false,
                data: None,
                message: Some("認可サーバーからアクセストークンが返却されませんでした。".to_string()),
            });
        }
    };

    let tokens = TokenSet {
        access_token,
        refresh_token: token_data.refresh_token.filter(|t| !t.is_empty()),
        expires_at: token_data.expires_in.map(|s| chrono::Utc::now().timestamp() + s),
    };
    if tokens.refresh_token.is_none() {
        warn!("Spotware did not return a refresh token; automatic token refresh will be unavailable");
    }

    info!("Access token received successfully! Saving tokens to SQLite...");
    // 1. トークンを SQLite に保存（以後の自動更新もここを更新する）
    state.save_ctrader_tokens(&tokens).await;

    // 2. 取引口座一覧を取得し、cTrader サービスに即座に接続
    let env_mode = env::var("CTRADER_ENV").unwrap_or_else(|_| "DEMO".to_string());
    let is_live = env_mode.to_uppercase() == "LIVE";
    let account_id: i64 = env::var("CTRADER_ACCOUNT_ID")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let config = CTraderConfig {
        client_id: client_id.clone(),
        client_secret: client_secret.clone(),
        account_id,
        access_token: String::new(),
        refresh_token: None,
        token_expires_at: None,
        is_live,
        symbol_suffix: crate::ctrader::symbol_suffix_from_env(),
    }
    .with_tokens(&tokens);

    info!("Connecting to cTrader Open API with newly obtained token...");
    match CTraderService::connect(config).await {
        Ok(service) => {
            let actual_account_id = service.config().account_id;
            info!("Successfully connected to cTrader! Target account: {}", actual_account_id);

            // .env の口座番号も同期
            let _ = AppState::update_env_file("CTRADER_ACCOUNT_ID", &actual_account_id.to_string());

            state.install_ctrader(service, true).await;

            let accounts = vec![AccountInfo {
                trader_login: actual_account_id,
                ctid_trader_account_id: actual_account_id,
                broker_title: "Spotware cTrader".to_string(),
                is_live,
            }];

            {
                let mut acc_lock = state.available_accounts.write().await;
                *acc_lock = accounts.clone();
            }

            Json(ApiResponse::ok_msg(
                accounts,
                format!("cTraderアカウント (#{}) との連携が完了しました！", actual_account_id),
            ))
        }
        Err(e) => {
            error!("Connected service init failed: {:?}", e);
            Json(ApiResponse {
                success: false,
                data: None,
                message: Some(format!("cTraderソケット接続エラー: {}", e)),
            })
        }
    }
}

/// GET /api/auth/ctrader/accounts
/// 連携済み取引口座一覧を返却
pub async fn get_accounts(State(state): State<AppState>) -> Json<ApiResponse<Vec<AccountInfo>>> {
    let lock = state.available_accounts.read().await;
    Json(ApiResponse::ok(lock.clone()))
}

/// POST /api/auth/ctrader/select-account
/// 使用する取引口座を切り替え
pub async fn select_account(
    State(state): State<AppState>,
    Json(payload): Json<SelectAccountRequest>,
) -> Json<ApiResponse<String>> {
    info!("Selecting cTrader active account: {}", payload.account_id);

    let _ = AppState::update_env_file("CTRADER_ACCOUNT_ID", &payload.account_id.to_string());

    // 現在の設定（SQLite 保存済みトークン込み）で口座だけ差し替えて再接続
    let current = state.ctrader_config.read().await.clone();
    let Some(mut config) = current.or_else(|| CTraderConfig::from_env().ok()) else {
        return Json(ApiResponse::err("cTrader の認証情報がありません"));
    };
    config.account_id = payload.account_id;
    if let Err(e) = state.connect_ctrader(config, true).await {
        warn!("Account switch failed: {e:#}");
    } else {
        return Json(ApiResponse::ok_msg(
            format!("口座 #{} に切り替えました", payload.account_id),
            "口座を切り替えました",
        ));
    }

    Json(ApiResponse::err("口座の切り替えに失敗しました"))
}
