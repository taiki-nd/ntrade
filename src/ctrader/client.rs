use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use ctrader_rs::proto::common::{
    ProtoMessage, ProtoOaExecutionEvent, ProtoOaGetTrendbarsReq, ProtoOaGetTrendbarsRes,
    ProtoOaPayloadType, ProtoOaTradeSide,
};
use ctrader_rs::{Client, Config};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::config::CTraderConfig;
use super::types::{BarPeriod, CandleBar, SymbolInfo};

/// cTrader Open API サービス
pub struct CTraderService {
    config: CTraderConfig,
    client: Arc<Client>,
    symbols: Arc<RwLock<HashMap<String, SymbolInfo>>>,
}

impl CTraderService {
    /// cTraderサーバーに接続・認証を行い、サービスを初期化する
    pub async fn connect(mut config: CTraderConfig) -> Result<Self> {
        info!(
            "Connecting to cTrader Open API (live={}, requested_account_id={})...",
            config.is_live, config.account_id
        );

        let mut client_config = Config::new(&config.client_id, &config.client_secret);
        if config.is_live {
            client_config = client_config.live();
        }

        // 1. アプリケーション認証 & 接続 & Heartbeat開始
        let client = Client::start_with_handler(client_config, Some(Self::handle_unsolicited_event))
            .await
            .context("Failed to connect & authenticate application with cTrader")?;
        let client = Arc::new(client);

        // 2. Access Token に紐づく取引口座一覧を取得し、Account ID を自動解決
        info!("Fetching trader accounts linked to Access Token...");
        let accounts_res = client
            .get_accounts_by_access_token(&config.access_token)
            .await
            .context("Failed to query accounts by access token. (Access Token may be invalid or expired)")?;

        let accounts = accounts_res.ctid_trader_account;
        if accounts.is_empty() {
            return Err(anyhow!(
                "No trader accounts found associated with this Access Token."
            ));
        }

        println!("\n  [Access Token に紐づく取引口座一覧]");
        for acc in &accounts {
            let login = acc.trader_login.unwrap_or(0);
            let broker = acc.broker_title_short.as_deref().unwrap_or("Unknown");
            let is_live_str = if acc.is_live.unwrap_or(false) { "LIVE" } else { "DEMO" };
            println!(
                "    - 口座番号(Login): {} | cTID Account ID: {} | Broker: {} | Type: {}",
                login, acc.ctid_trader_account_id, broker, is_live_str
            );
        }
        println!();

        // ユーザーが指定した account_id が「ログイン口座番号(trader_login)」か「ctidTraderAccountId」かを判定
        let mut resolved_account_id = None;
        for acc in &accounts {
            let ctid_id = acc.ctid_trader_account_id as i64;
            let login_id = acc.trader_login.unwrap_or(0);

            // ctidTraderAccountId と完全一致
            if config.account_id == ctid_id {
                resolved_account_id = Some(ctid_id);
                info!("Matched exact ctidTraderAccountId: {}", ctid_id);
                break;
            }
            // ログイン番号（アプリに表示される番号）と一致
            if config.account_id == login_id {
                resolved_account_id = Some(ctid_id);
                info!(
                    "Resolved login account {} to ctidTraderAccountId {}",
                    login_id, ctid_id
                );
                break;
            }
        }

        // 口座が1つしかなく、指定値と未一致の場合はその1つを自動採用
        let target_account_id = match resolved_account_id {
            Some(id) => id,
            None if accounts.len() == 1 => {
                let auto_id = accounts[0].ctid_trader_account_id as i64;
                warn!(
                    "Configured account_id {} was not matched, but only 1 account exists. Auto-selecting ctidTraderAccountId: {}",
                    config.account_id, auto_id
                );
                auto_id
            }
            None => {
                return Err(anyhow!(
                    "Specified account ID {} was not found in the Access Token accounts list. Please select one of the cTID Account IDs listed above in your .env (CTRADER_ACCOUNT_ID).",
                    config.account_id
                ));
            }
        };

        config.account_id = target_account_id;

        // 3. 取引口座の認証
        info!("Authenticating trader account (ctidTraderAccountId: {})...", config.account_id);
        let auth_res = client
            .account_auth(config.account_id, &config.access_token)
            .await
            .context("Failed to authenticate trader account")?;
        info!(
            "Trader account authenticated successfully (ctidTraderAccountId: {})",
            auth_res.ctid_trader_account_id
        );

        let service = Self {
            config,
            client,
            symbols: Arc::new(RwLock::new(HashMap::new())),
        };

        // 4. シンボルリストを初期取得
        service.refresh_symbols().await?;

        Ok(service)
    }

    /// イベント受信ハンドラ（約定通知やSpot価格更新など）
    fn handle_unsolicited_event(msg: ProtoMessage) {
        debug!("Received unsolicited cTrader message (type={})", msg.payload_type);
    }

    /// 利用可能なシンボル一覧をAPIから取得して内部キャッシュを更新
    pub async fn refresh_symbols(&self) -> Result<()> {
        info!("Fetching symbol list from cTrader...");
        let res = self
            .client
            .symbols_list(self.config.account_id, false)
            .await
            .context("Failed to fetch symbols list")?;

        let mut symbols_map = HashMap::new();
        for sym in res.symbol {
            if let Some(name) = sym.symbol_name {
                let info = SymbolInfo::new_with_defaults(sym.symbol_id, name.clone());
                symbols_map.insert(name.to_uppercase(), info);
            }
        }

        info!("Loaded {} symbols into cTrader cache", symbols_map.len());
        let mut lock = self.symbols.write().await;
        *lock = symbols_map;

        Ok(())
    }

    /// 通貨ペア名（例: "USDJPY", "EURUSD"）から symbol_id を解決
    pub async fn get_symbol_id(&self, symbol_name: &str) -> Result<i64> {
        let norm_name = symbol_name.replace("/", "").to_uppercase();
        let symbols = self.symbols.read().await;

        if let Some(info) = symbols.get(&norm_name) {
            return Ok(info.symbol_id);
        }

        // 見つからない場合は前方一致（ブローカー固有のサフィックス等対応: EURUSD.pro, EURUSDm等）
        for (name, info) in symbols.iter() {
            if name.starts_with(&norm_name) {
                info!("Matched symbol '{}' for query '{}'", name, symbol_name);
                return Ok(info.symbol_id);
            }
        }

        Err(anyhow!(
            "Symbol '{}' not found in cTrader symbols list. (Total available: {})",
            symbol_name,
            symbols.len()
        ))
    }

    /// 過去のローソク足（Trendbars）を取得
    pub async fn get_trendbars(
        &self,
        symbol_name: &str,
        period: BarPeriod,
        count: u32,
    ) -> Result<Vec<CandleBar>> {
        let symbol_id = self.get_symbol_id(symbol_name).await?;
        let to_timestamp = Utc::now().timestamp_millis();

        let req = ProtoOaGetTrendbarsReq {
            payload_type: Some(ProtoOaPayloadType::ProtoOaGetTrendbarsReq as i32),
            ctid_trader_account_id: self.config.account_id,
            symbol_id,
            period: period.to_proto_i32(),
            count: Some(count),
            to_timestamp: Some(to_timestamp),
            from_timestamp: None,
        };

        let res: ProtoOaGetTrendbarsRes = self
            .client
            .command(
                ProtoOaPayloadType::ProtoOaGetTrendbarsReq as u32,
                req,
                ProtoOaPayloadType::ProtoOaGetTrendbarsRes as u32,
            )
            .await
            .context("Failed to get trendbars from cTrader")?;

        let mut bars: Vec<CandleBar> = res
            .trendbar
            .iter()
            .map(CandleBar::from_proto)
            .collect();

        // タイムスタンプ順に昇順ソート（古い足 -> 最新足）
        bars.sort_by_key(|b| b.timestamp);

        info!(
            "Retrieved {} bars for {} (period: {:?})",
            bars.len(),
            symbol_name,
            period.as_str()
        );

        Ok(bars)
    }

    /// サーバーサイドSL/TP付き成行注文の発行（デモ検証・本番兼用）
    /// `volume`: 0.01 lot = 100
    /// `rel_sl_points`: 1 pip = 10 points = 1000 in protocol (5-digit pair)
    pub async fn place_market_order_with_sltp(
        &self,
        symbol_name: &str,
        is_buy: bool,
        volume: i64,
        rel_sl_points: Option<i64>,
        rel_tp_points: Option<i64>,
    ) -> Result<ProtoOaExecutionEvent> {
        let symbol_id = self.get_symbol_id(symbol_name).await?;
        let trade_side = if is_buy {
            ProtoOaTradeSide::Buy
        } else {
            ProtoOaTradeSide::Sell
        };

        info!(
            "Placing market order: {} {:?} volume={}, sl={:?}, tp={:?}",
            symbol_name, trade_side, volume, rel_sl_points, rel_tp_points
        );

        match (rel_sl_points, rel_tp_points) {
            (Some(sl), Some(tp)) => {
                self.client
                    .new_market_order_with_sltp(
                        self.config.account_id,
                        symbol_id,
                        trade_side,
                        volume,
                        sl,
                        tp,
                    )
                    .await
                    .context("Failed to place market order with SL/TP")
            }
            _ => {
                self.client
                    .new_market_order(self.config.account_id, symbol_id, trade_side, volume)
                    .await
                    .context("Failed to place market order")
            }
        }
    }

    /// Refresh Token を用いて Access Token を自動更新
    pub async fn refresh_access_token(&mut self) -> Result<String> {
        let refresh_token = self
            .config
            .refresh_token
            .as_ref()
            .ok_or_else(|| anyhow!("Refresh token is not configured in CTRADER_REFRESH_TOKEN"))?;

        info!("Refreshing cTrader access token...");
        let res = self
            .client
            .refresh_token(refresh_token)
            .await
            .context("Failed to refresh token with cTrader API")?;

        info!("Access token refreshed successfully (expires_in: {:?})", res.expires_in);
        self.config.access_token = res.access_token.clone();
        if !res.refresh_token.is_empty() {
            self.config.refresh_token = Some(res.refresh_token);
        }

        Ok(res.access_token)
    }

    /// 現在の口座設定を取得
    pub fn config(&self) -> &CTraderConfig {
        &self.config
    }
}
