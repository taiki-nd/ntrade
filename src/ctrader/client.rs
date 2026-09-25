use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use ctrader_rs::proto::common::{
    ProtoMessage, ProtoOaAccessRights, ProtoOaClosePositionReq, ProtoOaErrorRes, ProtoOaExecutionEvent, ProtoOaExecutionType,
    ProtoOaGetTrendbarsReq, ProtoOaGetTrendbarsRes, ProtoOaOrderErrorEvent, ProtoOaPayloadType, ProtoOaSymbolByIdReq,
    ProtoOaSymbolByIdRes, ProtoOaTradeSide, ProtoOaTradingMode,
};
use ctrader_rs::{Client, Config};
use prost::Message as _;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, RwLock};
use tokio::time::Instant;
use tracing::{debug, info, warn};

use super::config::CTraderConfig;
use super::types::{volume_to_lots, AccountSummary, BarPeriod, BrokerPosition, CandleBar, ClosedDeal, SymbolInfo};

/// 注文に対するサーバーからの非同期な応答
///
/// cTrader は成行注文の結果を clientMsgId 無しの非請求メッセージとして返すため、
/// 受信ハンドラでデコードしてここに流し、発注側が対応するものを拾う。
#[derive(Debug, Clone)]
pub enum OrderOutcome {
    /// 約定・受理・拒否などの執行イベント
    Execution(Box<ProtoOaExecutionEvent>),
    /// 注文が検証段階で弾かれた
    OrderError {
        error_code: String,
        description: Option<String>,
    },
}

/// Access Token の取引権限の判定結果
#[derive(Debug, Clone, PartialEq)]
pub enum TradingPermission {
    /// TRADING_DISABLED は返らなかった（打診に対して返った errorCode を保持）。
    /// 取引可能であることの確証ではない
    Granted { error_code: String },
    /// TRADING_DISABLED。scope が accounts か、口座が取引不可
    Denied { error_code: String, description: String },
    /// 判定できなかった（通信エラーなど）
    Unknown(String),
}

/// 注文応答を待つ上限。ctrader-rs 側の deadline(5s) を含む余裕を見た値。
const ORDER_OUTCOME_TIMEOUT: Duration = Duration::from_secs(12);

/// cTrader Open API サービス
pub struct CTraderService {
    config: CTraderConfig,
    client: Arc<Client>,
    symbols: Arc<RwLock<HashMap<String, SymbolInfo>>>,
    /// 非請求メッセージから復元した注文応答の配信元
    order_events: broadcast::Sender<OrderOutcome>,
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
        let (order_events, _) = broadcast::channel::<OrderOutcome>(64);
        let handler_tx = order_events.clone();
        let client = Client::start_with_handler(
            client_config,
            Some(move |msg: ProtoMessage| handle_unsolicited_event(msg, &handler_tx)),
        )
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
            order_events,
        };

        // 4. シンボルリストを初期取得
        service.refresh_symbols().await?;

        // 5. 取引権限の確認（TRADING_DISABLED の切り分け用）
        match service.get_account_info().await {
            Ok(info) => match info.access_rights.as_deref() {
                Some("FULL_ACCESS") => info!("Account access rights: FULL_ACCESS (trading enabled)"),
                Some(other) => warn!(
                    "Account access rights: {other} — この口座では新規発注ができません（ブローカー側の口座設定を確認してください）"
                ),
                None => warn!("Account access rights unknown (accessRights not returned)"),
            },
            Err(e) => warn!("Failed to read account access rights: {e:#}"),
        }

        Ok(service)
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

    /// ブローカー側の銘柄名から口座固有のサフィックスを取り除く（`USDJPY_z` → `USDJPY`）
    fn strip_symbol_suffix(&self, name: &str) -> String {
        match &self.config.symbol_suffix {
            Some(suffix) => name.strip_suffix(suffix.as_str()).unwrap_or(name).to_string(),
            None => name.to_string(),
        }
    }

    /// 通貨ペア名（例: "USDJPY", "EURUSD"）から symbol_id を解決
    pub async fn get_symbol_id(&self, symbol_name: &str) -> Result<i64> {
        let norm_name = symbol_name.replace("/", "").to_uppercase();
        let symbols = self.symbols.read().await;

        // ゼロ口座のように、取引できるのがサフィックス付き銘柄（例: USDJPY_z）だけの
        // 口座がある。素の銘柄もシンボル一覧に存在し tradingMode は ENABLED に見えるが、
        // 発注すると TRADING_DISABLED で拒否されるため、サフィックス付きを優先して解決する。
        if let Some(suffix) = &self.config.symbol_suffix {
            let suffixed = format!("{}{}", norm_name, suffix.to_uppercase());
            match symbols.get(&suffixed) {
                Some(info) => {
                    debug!("Resolved symbol '{}' -> '{}' (id {})", symbol_name, suffixed, info.symbol_id);
                    return Ok(info.symbol_id);
                }
                None => warn!(
                    "Symbol '{}' not found (CTRADER_SYMBOL_SUFFIX={}); falling back to '{}'",
                    suffixed, suffix, norm_name
                ),
            }
        }

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

    /// 期間指定でヒストリカルバーを取得する（リプレイ用）。
    /// cTrader は1リクエストあたりの期間に上限があるため、時間足に応じてチャンク分割して繰り返し取得する。
    pub async fn get_trendbars_range(
        &self,
        symbol_name: &str,
        period: BarPeriod,
        from: chrono::DateTime<Utc>,
        to: chrono::DateTime<Utc>,
    ) -> Result<Vec<CandleBar>> {
        let symbol_id = self.get_symbol_id(symbol_name).await?;
        let chunk = match period {
            BarPeriod::M1 => chrono::Duration::days(2),
            BarPeriod::M5 | BarPeriod::M15 | BarPeriod::M30 => chrono::Duration::days(7),
            BarPeriod::H1 | BarPeriod::H4 => chrono::Duration::days(30),
            BarPeriod::D1 => chrono::Duration::days(365),
        };

        let mut all: Vec<CandleBar> = Vec::new();
        let mut cursor = from;
        while cursor < to {
            let chunk_end = (cursor + chunk).min(to);
            let req = ProtoOaGetTrendbarsReq {
                payload_type: Some(ProtoOaPayloadType::ProtoOaGetTrendbarsReq as i32),
                ctid_trader_account_id: self.config.account_id,
                symbol_id,
                period: period.to_proto_i32(),
                count: None,
                from_timestamp: Some(cursor.timestamp_millis()),
                to_timestamp: Some(chunk_end.timestamp_millis()),
            };
            let res: ProtoOaGetTrendbarsRes = self
                .client
                .command(
                    ProtoOaPayloadType::ProtoOaGetTrendbarsReq as u32,
                    req,
                    ProtoOaPayloadType::ProtoOaGetTrendbarsRes as u32,
                )
                .await
                .with_context(|| format!("Failed to get trendbars {}..{}", cursor, chunk_end))?;
            let n = res.trendbar.len();
            all.extend(res.trendbar.iter().map(CandleBar::from_proto));
            debug!("Fetched {} {:?} bars for {} ({} .. {})", n, period, symbol_name, cursor, chunk_end);
            cursor = chunk_end;
            // レート制限への配慮
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        }

        all.sort_by_key(|b| b.timestamp);
        all.dedup_by_key(|b| b.timestamp);
        info!(
            "Retrieved {} historical bars for {} (period: {}, {} .. {})",
            all.len(),
            symbol_name,
            period.as_str(),
            from,
            to
        );
        Ok(all)
    }

    /// 口座情報（残高など）。cTrader の金額は整数で、`money_digits` 桁分を割って実値にする。
    pub async fn get_account_info(&self) -> Result<AccountSummary> {
        let res = self
            .client
            .get_trader(self.config.account_id)
            .await
            .context("Failed to get trader info from cTrader")?;
        let t = res.trader;
        let digits = t.money_digits.unwrap_or(2);
        let scale = 10f64.powi(digits as i32);
        let access_rights = t
            .access_rights
            .and_then(|r| ProtoOaAccessRights::try_from(r).ok())
            .map(|r| r.as_str_name().to_string());
        Ok(AccountSummary {
            account_id: t.ctid_trader_account_id,
            trader_login: t.trader_login,
            balance: t.balance as f64 / scale,
            leverage: t.leverage_in_cents.map(|l| l as f64 / 100.0),
            is_live: self.config.is_live,
            access_rights,
        })
    }

    /// ブローカー側の保有ポジション一覧（reconcile）
    pub async fn get_open_positions(&self) -> Result<Vec<BrokerPosition>> {
        let res = self
            .client
            .reconcile(self.config.account_id, false)
            .await
            .context("Failed to reconcile positions from cTrader")?;
        let symbols = self.symbols.read().await;
        let by_id: HashMap<i64, String> = symbols.values().map(|s| (s.symbol_id, s.symbol_name.clone())).collect();
        Ok(res
            .position
            .iter()
            .map(|p| BrokerPosition {
                position_id: p.position_id,
                symbol_id: p.trade_data.symbol_id,
                // アプリ内では素の銘柄名（USDJPY）で扱うため、口座固有のサフィックスは剥がす
                symbol_name: by_id.get(&p.trade_data.symbol_id).map(|n| self.strip_symbol_suffix(n)),
                is_buy: p.trade_data.trade_side == ProtoOaTradeSide::Buy as i32,
                volume_lots: volume_to_lots(p.trade_data.volume),
                entry_price: p.price,
                stop_loss: p.stop_loss,
                take_profit: p.take_profit,
                open_time: p
                    .trade_data
                    .open_timestamp
                    .and_then(|ms| chrono::TimeZone::timestamp_millis_opt(&Utc, ms).single()),
            })
            .collect())
    }

    /// 指定ポジションの決済（closing deal）をブローカーの約定履歴から取得する。
    ///
    /// SL/TP はブローカー側で執行されるため、ntrade から見ると「ポジションが消えた」ことしか分からない。
    /// 決済価格・決済時刻・実現損益をローカルの足から推定すると cTrader の履歴とずれるので、
    /// 約定そのものから確定させる。分割決済された場合は最後の決済約定を採用する。
    pub async fn get_position_close(
        &self,
        position_id: i64,
        from: chrono::DateTime<Utc>,
    ) -> Result<Option<ClosedDeal>> {
        let res = self
            .client
            .deal_list_by_position(
                self.config.account_id,
                position_id,
                Some(from.timestamp_millis()),
                Some(Utc::now().timestamp_millis()),
            )
            .await
            .with_context(|| format!("Failed to fetch deals for position {position_id}"))?;

        let deal = res
            .deal
            .iter()
            .filter(|d| d.close_position_detail.is_some())
            .max_by_key(|d| d.execution_timestamp);
        let Some(d) = deal else { return Ok(None) };
        let detail = d.close_position_detail.as_ref().expect("filtered above");

        // 金額は 10^money_digits 倍の整数で返る
        let scale = 10f64.powi(detail.money_digits.unwrap_or(2) as i32);
        let gross_profit = detail.gross_profit as f64 / scale;
        let swap = detail.swap as f64 / scale;
        let commission = detail.commission as f64 / scale;
        let conversion_fee = detail.pnl_conversion_fee.unwrap_or(0) as f64 / scale;
        let close_time = chrono::TimeZone::timestamp_millis_opt(&Utc, d.execution_timestamp)
            .single()
            .unwrap_or_else(Utc::now);

        Ok(Some(ClosedDeal {
            position_id,
            deal_id: d.deal_id,
            close_price: d.execution_price.unwrap_or(detail.entry_price),
            close_time,
            entry_price: detail.entry_price,
            net_profit: gross_profit + swap + commission + conversion_fee,
            gross_profit,
            swap,
            commission,
            volume_lots: volume_to_lots(detail.closed_volume.unwrap_or(d.filled_volume)),
        }))
    }

    /// 保有ポジションごとの含み損益（口座通貨）。ブローカー側の確定値なので手数料込みで一致する。
    pub async fn get_unrealized_pnl(&self) -> Result<HashMap<i64, f64>> {
        let res = self
            .client
            .position_unrealized_pnl(self.config.account_id)
            .await
            .context("Failed to fetch unrealized PnL from cTrader")?;
        let scale = 10f64.powi(res.money_digits as i32);
        Ok(res
            .position_unrealized_pn_l
            .iter()
            .map(|p| (p.position_id, p.net_unrealized_pn_l as f64 / scale))
            .collect())
    }

    /// シンボル単位の取引可否（ENABLED / CLOSE_ONLY_MODE / DISABLED_*）を取得する。
    ///
    /// ctrader_rs::symbol_by_id は戻り値の型が Req になっており正しくデコードできないため、
    /// command を直接呼ぶ。
    pub async fn get_symbol_trading_mode(&self, symbol_name: &str) -> Result<String> {
        let symbol_id = self.get_symbol_id(symbol_name).await?;
        let req = ProtoOaSymbolByIdReq {
            payload_type: Some(ProtoOaPayloadType::ProtoOaSymbolByIdReq as i32),
            ctid_trader_account_id: self.config.account_id,
            symbol_id: vec![symbol_id],
        };
        let res: ProtoOaSymbolByIdRes = self
            .client
            .command(
                ProtoOaPayloadType::ProtoOaSymbolByIdReq as u32,
                req,
                ProtoOaPayloadType::ProtoOaSymbolByIdRes as u32,
            )
            .await
            .map_err(|e| anyhow!(e).context(format!("Failed to fetch symbol details for {symbol_name}")))?;
        let sym = res
            .symbol
            .first()
            .ok_or_else(|| anyhow!("cTrader returned no symbol details for {symbol_name}"))?;
        Ok(sym
            .trading_mode
            .and_then(|m| ProtoOaTradingMode::try_from(m).ok())
            .map(|m| m.as_str_name().to_string())
            .unwrap_or_else(|| "UNKNOWN".to_string()))
    }

    /// 建玉を作らずに取引権限を打診する。
    ///
    /// 存在しないポジションの決済を要求し、返ってきた errorCode を見る。
    /// 誤って実在の建玉を決済しないよう、既存のどの position_id とも一致しない ID と
    /// volume=0 を使う。
    ///
    /// 注意: `Granted`（TRADING_DISABLED が返らなかった）は取引可能であることの確証ではない。
    /// 対象ポジションが存在しないため、権限チェックより先に POSITION_NOT_FOUND で
    /// 弾かれている可能性がある。`Denied` の場合のみ断定できる。
    pub async fn probe_trading_permission(&self) -> Result<TradingPermission> {
        let open = self.get_open_positions().await?;
        let probe_id = open.iter().map(|p| p.position_id).max().unwrap_or(0).saturating_add(1_000_000_000);
        // volume=0 なら万一 ID が実在しても決済数量はゼロ
        let req = ProtoOaClosePositionReq {
            payload_type: Some(ProtoOaPayloadType::ProtoOaClosePositionReq as i32),
            ctid_trader_account_id: self.config.account_id,
            position_id: probe_id,
            volume: 0,
        };
        // 応答は ProtoOAOrderErrorEvent で返る。ctrader_rs::close_position は
        // ExecutionEvent しか想定しておらず errorCode を捨ててしまうため、直接デコードする。
        let res: Result<ProtoOaOrderErrorEvent, _> = self
            .client
            .command(
                ProtoOaPayloadType::ProtoOaClosePositionReq as u32,
                req,
                ProtoOaPayloadType::ProtoOaOrderErrorEvent as u32,
            )
            .await;
        match res {
            Ok(ev) if ev.error_code == "TRADING_DISABLED" => Ok(TradingPermission::Denied {
                error_code: ev.error_code,
                description: ev.description.unwrap_or_default(),
            }),
            Ok(ev) => Ok(TradingPermission::Granted { error_code: ev.error_code }),
            Err(ctrader_rs::Error::Api { error_code, description }) if error_code == "TRADING_DISABLED" => {
                Ok(TradingPermission::Denied { error_code, description })
            }
            Err(ctrader_rs::Error::Api { error_code, .. }) => Ok(TradingPermission::Granted { error_code }),
            Err(e) => Ok(TradingPermission::Unknown(e.to_string())),
        }
    }

    /// ポジション ID 指定の決済
    ///
    /// 発注と同様、ctrader-rs は決済結果（非請求の執行イベント）を拾えず Timeout を返すため、
    /// 執行イベント側で結果を確定させる。
    pub async fn close_position_by_id(&self, position_id: i64, volume: i64) -> Result<ProtoOaExecutionEvent> {
        let mut rx = self.order_events.subscribe();
        let deadline = Instant::now() + ORDER_OUTCOME_TIMEOUT;
        match self.client.close_position(self.config.account_id, position_id, volume).await {
            Ok(ev) => return Ok(ev),
            Err(ctrader_rs::Error::Timeout) => {
                debug!("Close response not delivered by ctrader-rs; falling back to execution events");
            }
            Err(e) => return Err(anyhow!(e).context(format!("Failed to close position {position_id}"))),
        }
        self.await_order_outcome(&mut rx, deadline, |ev| {
            ev.position.as_ref().map(|p| p.position_id) == Some(position_id)
                || ev.deal.as_ref().map(|d| d.position_id) == Some(position_id)
        })
        .await
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

        // 注文フレーム送信より先に購読する（応答を取りこぼさないため）
        let mut rx = self.order_events.subscribe();
        let deadline = Instant::now() + ORDER_OUTCOME_TIMEOUT;

        // ctrader-rs 0.1.2 の new_market_order* は応答待ちの実装が欠けており、
        // 注文フレームを送った後に必ず Error::Timeout を返す。送信自体は成功しているので、
        // Timeout は無視して非請求イベント側で結果を確定させる。
        let sent = match (rel_sl_points, rel_tp_points) {
            (Some(sl), Some(tp)) => {
                self.client
                    .new_market_order_with_sltp(self.config.account_id, symbol_id, trade_side, volume, sl, tp)
                    .await
            }
            _ => {
                self.client
                    .new_market_order(self.config.account_id, symbol_id, trade_side, volume)
                    .await
            }
        };
        match sent {
            // 将来ライブラリ側が修正された場合はそのまま結果を使う
            Ok(ev) => return Ok(ev),
            Err(ctrader_rs::Error::Timeout) => {
                debug!("Order response not delivered by ctrader-rs; falling back to execution events");
            }
            Err(e) => return Err(anyhow!(e).context("Failed to send market order to cTrader")),
        }

        self.await_order_outcome(&mut rx, deadline, |ev| {
            // 他シンボルの建玉更新などが混ざるのでシンボルで絞る
            let ev_symbol = ev
                .deal
                .as_ref()
                .map(|d| d.symbol_id)
                .or_else(|| ev.order.as_ref().map(|o| o.trade_data.symbol_id))
                .or_else(|| ev.position.as_ref().map(|p| p.trade_data.symbol_id));
            ev_symbol == Some(symbol_id)
        })
        .await
    }

    /// 発注・決済後、対象の執行イベント（または注文エラー）が届くまで待つ
    async fn await_order_outcome(
        &self,
        rx: &mut broadcast::Receiver<OrderOutcome>,
        deadline: Instant,
        matches: impl Fn(&ProtoOaExecutionEvent) -> bool,
    ) -> Result<ProtoOaExecutionEvent> {
        loop {
            let outcome = match tokio::time::timeout_at(deadline, rx.recv()).await {
                Ok(Ok(o)) => o,
                Ok(Err(broadcast::error::RecvError::Lagged(n))) => {
                    warn!("Missed {n} cTrader events while waiting for order result");
                    continue;
                }
                Ok(Err(broadcast::error::RecvError::Closed)) => {
                    return Err(anyhow!("cTrader event stream closed while waiting for order result"))
                }
                Err(_) => {
                    return Err(anyhow!(
                        "No execution event received within {}s; order state at broker is unknown",
                        ORDER_OUTCOME_TIMEOUT.as_secs()
                    ))
                }
            };

            match outcome {
                OrderOutcome::OrderError { error_code, description } => {
                    return Err(anyhow!(
                        "cTrader rejected order: {error_code}{}{}",
                        description.map(|d| format!(" ({d})")).unwrap_or_default(),
                        order_error_hint(&error_code)
                    ));
                }
                OrderOutcome::Execution(ev) => {
                    if !matches(&ev) {
                        continue;
                    }
                    let exec_type = ProtoOaExecutionType::try_from(ev.execution_type).ok();
                    match exec_type {
                        // 受理のみの通知。約定イベントが続くので待ち続ける
                        Some(ProtoOaExecutionType::OrderAccepted) => continue,
                        Some(ProtoOaExecutionType::OrderRejected) | Some(ProtoOaExecutionType::OrderCancelled) => {
                            let code = ev.error_code.as_deref().unwrap_or("no error code");
                            return Err(anyhow!(
                                "cTrader {}: {code}{}",
                                exec_type.map(|t| t.as_str_name()).unwrap_or("UNKNOWN"),
                                order_error_hint(code)
                            ));
                        }
                        _ => return Ok(*ev),
                    }
                }
            }
        }
    }

    /// 現在の口座設定を取得
    pub fn config(&self) -> &CTraderConfig {
        &self.config
    }
}

/// 注文拒否コードに対する、設定側で取りうる対処のヒント
///
/// cTrader の errorCode だけでは原因が口座側かトークン側か分からないため、
/// 切り分けに必要な情報をエラーメッセージ（＝ダッシュボードの未発注理由）に載せる。
fn order_error_hint(error_code: &str) -> &'static str {
    match error_code {
        "TRADING_DISABLED" => {
            " / Access Token に取引権限が無い可能性があります。設定画面から再認証し、cTrader の認可画面で scope に『Trading』を選択してください（『View only』では発注できません）"
        }
        "NOT_ENOUGH_MONEY" => " / 証拠金が不足しています。ロット数またはレバレッジを確認してください",
        "MARKET_CLOSED" => " / 市場が閉じています",
        "POSITION_LOCKED" | "POSITION_NOT_FOUND" => " / 対象ポジションの状態を reconcile で確認してください",
        _ => "",
    }
}

/// 非請求メッセージ（約定通知・注文エラー・Spot 価格更新など）の受信ハンドラ
///
/// 成行注文の結果は clientMsgId を持たないためライブラリの request/response 機構では
/// 受け取れない。ここでデコードしてログに残し、発注側へ配信する。
fn handle_unsolicited_event(msg: ProtoMessage, tx: &broadcast::Sender<OrderOutcome>) {
    let payload = msg.payload.as_deref().unwrap_or_default();
    match msg.payload_type {
        t if t == ProtoOaPayloadType::ProtoOaExecutionEvent as u32 => {
            match ProtoOaExecutionEvent::decode(payload) {
                Ok(ev) => {
                    let exec_type = ProtoOaExecutionType::try_from(ev.execution_type)
                        .map(|t| t.as_str_name().to_string())
                        .unwrap_or_else(|_| format!("UNKNOWN({})", ev.execution_type));
                    info!(
                        exec_type = %exec_type,
                        error_code = ?ev.error_code,
                        position_id = ?ev.position.as_ref().map(|p| p.position_id),
                        order_id = ?ev.order.as_ref().map(|o| o.order_id),
                        symbol_id = ?ev.order.as_ref().map(|o| o.trade_data.symbol_id),
                        "cTrader execution event"
                    );
                    let _ = tx.send(OrderOutcome::Execution(Box::new(ev)));
                }
                Err(e) => warn!("Failed to decode ProtoOAExecutionEvent: {e}"),
            }
        }
        t if t == ProtoOaPayloadType::ProtoOaOrderErrorEvent as u32 => {
            match ProtoOaOrderErrorEvent::decode(payload) {
                Ok(ev) => {
                    warn!(
                        error_code = %ev.error_code,
                        description = ?ev.description,
                        order_id = ?ev.order_id,
                        "cTrader rejected an order"
                    );
                    let _ = tx.send(OrderOutcome::OrderError {
                        error_code: ev.error_code,
                        description: ev.description,
                    });
                }
                Err(e) => warn!("Failed to decode ProtoOAOrderErrorEvent: {e}"),
            }
        }
        t if t == ProtoOaPayloadType::ProtoOaErrorRes as u32 => match ProtoOaErrorRes::decode(payload) {
            Ok(res) => {
                warn!(
                    error_code = %res.error_code,
                    description = ?res.description,
                    "cTrader error response (unsolicited)"
                );
                let _ = tx.send(OrderOutcome::OrderError {
                    error_code: res.error_code,
                    description: res.description,
                });
            }
            Err(e) => warn!("Failed to decode ProtoOAErrorRes: {e}"),
        },
        t => debug!("Received unsolicited cTrader message (type={t})"),
    }
}
