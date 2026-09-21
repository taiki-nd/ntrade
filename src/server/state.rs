use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::ctrader::{token, CTraderConfig, CTraderService, TokenSet};
use crate::executor::{CTraderOrderSink, DisconnectedOrderSink, FixOrderSink, OrderSink, PlanBook};
use crate::guard::{GuardConfig, DEFAULT_GUARD_CONFIG_PATH};
use crate::llm::{LlmClient, LlmClientConfig};
use crate::snapshot::SnapshotBundle;
use crate::storage::Db;
use super::types::{
    AccountInfo, AccountMetrics, BotState, ConnectionStatus, CoTLog, Position, TradeHistory,
};

#[derive(Clone)]
pub struct AppState {
    pub bot_state: Arc<RwLock<BotState>>,
    pub metrics: Arc<RwLock<AccountMetrics>>,
    pub positions: Arc<RwLock<Vec<Position>>>,
    pub ctrader_service: Arc<RwLock<Option<Arc<CTraderService>>>>,
    pub ctrader_config: Arc<RwLock<Option<CTraderConfig>>>,
    pub available_accounts: Arc<RwLock<Vec<AccountInfo>>>,
    /// 直近に生成した Snapshot（客観的事実 + 画像4枚）
    pub latest_snapshot: Arc<RwLock<Option<SnapshotBundle>>>,
    /// SQLite（ヒストリカルバー・リプレイ結果・判断ログ・ポジション・決済履歴・教訓）
    pub db_path: PathBuf,
    /// `positions` の SQLite 書き出しを直列化する（古いスナップショットで上書きしないため）
    positions_persist_lock: Arc<tokio::sync::Mutex<()>>,
    /// 事後ガード設定（config/guard.toml）
    pub guard_config: Arc<RwLock<GuardConfig>>,
    /// 保持中の条件付きプラン
    pub plan_book: Arc<RwLock<PlanBook>>,
    /// LLM 推論クライアント
    pub llm: Arc<LlmClient>,
    /// 発注先（FIX 設定時は FixOrderSink、cTrader 接続時は CTraderOrderSink、未接続時は DisconnectedOrderSink）
    pub order_sink: Arc<RwLock<Arc<dyn OrderSink>>>,
    /// FIX 取引セッション（発注を FIX に委ねている場合のみ。決済時の保護注文取り消しに使う）
    pub fix_client: Arc<RwLock<Option<Arc<crate::fix::FixTradingClient>>>>,
    /// 判断サイクルの直列化
    pub decide_lock: Arc<tokio::sync::Mutex<()>>,
    /// ブローカー残高の最終同期時刻
    pub last_broker_sync: Arc<RwLock<Option<std::time::Instant>>>,
}

/// 前回終了時の保有ポジション（ペーパー決済の継続と判断ログとの対応付けのため）
fn load_saved_positions(db_path: &std::path::Path) -> Vec<Position> {
    match Db::open(db_path).and_then(|db| db.positions()) {
        Ok(positions) => {
            if !positions.is_empty() {
                info!(count = positions.len(), "Restored open positions from SQLite");
            }
            positions
        }
        Err(e) => {
            warn!("Failed to load positions from SQLite: {e:#}");
            Vec::new()
        }
    }
}

fn load_saved_tokens(db_path: &std::path::Path) -> Option<TokenSet> {
    match Db::open(db_path).and_then(|db| db.load_ctrader_tokens()) {
        Ok(tokens) => tokens,
        Err(e) => {
            warn!("Failed to load cTrader tokens from SQLite: {e:#}");
            None
        }
    }
}

/// 実発注のみサポート
pub fn live_orders_enabled() -> bool {
    true
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        let metrics = Self::initial_metrics();

        let db_path = PathBuf::from(crate::storage::DEFAULT_DB_PATH);
        let positions = load_saved_positions(&db_path);
        // SQLite に保存済みのトークンがあれば .env の値より優先する（自動更新で .env は古くなるため）
        let initial_config = CTraderConfig::from_env().ok().map(|cfg| match load_saved_tokens(&db_path) {
            Some(tokens) => {
                info!("Using cTrader tokens saved in SQLite");
                cfg.with_tokens(&tokens)
            }
            None => cfg,
        });

        Self {
            bot_state: Arc::new(RwLock::new(BotState::Running)),
            metrics: Arc::new(RwLock::new(metrics)),
            positions: Arc::new(RwLock::new(positions)),
            ctrader_service: Arc::new(RwLock::new(None)),
            ctrader_config: Arc::new(RwLock::new(initial_config)),
            available_accounts: Arc::new(RwLock::new(Vec::new())),
            latest_snapshot: Arc::new(RwLock::new(None)),
            db_path,
            positions_persist_lock: Arc::new(tokio::sync::Mutex::new(())),
            guard_config: Arc::new(RwLock::new(GuardConfig::load_or_default(DEFAULT_GUARD_CONFIG_PATH))),
            plan_book: Arc::new(RwLock::new(PlanBook::default())),
            llm: Arc::new(LlmClient::new(LlmClientConfig::default())),
            order_sink: Arc::new(RwLock::new(Arc::new(DisconnectedOrderSink))),
            fix_client: Arc::new(RwLock::new(None)),
            decide_lock: Arc::new(tokio::sync::Mutex::new(())),
            last_broker_sync: Arc::new(RwLock::new(None)),
        }
    }

    /// バックグラウンドでcTraderへの初期接続を試みる
    pub async fn init_ctrader_connection(&self) {
        let cfg_opt = self.ctrader_config.read().await.clone();

        if let Some(cfg) = cfg_opt {
            info!("Attempting background connection to cTrader Open API...");
            if let Err(e) = self.connect_ctrader(cfg, true).await {
                warn!("Initial cTrader connection failed: {:?}. System is in offline/simulation mode.", e);
            }
        } else {
            info!("No cTrader credentials configured in .env. Waiting for in-app OAuth linking.");
            let mut m_lock = self.metrics.write().await;
            m_lock.connection_status.ctrader = "disconnected".to_string();
        }
    }

    /// 接続して state に組み込む。トークン失効などで失敗した場合は Refresh Token で更新して1回だけ再試行する。
    /// `account_changed` なら口座切り替えとみなしてペーパー残高を初期化し直す。
    pub async fn connect_ctrader(&self, cfg: CTraderConfig, account_changed: bool) -> Result<()> {
        let result = match CTraderService::connect(cfg.clone()).await {
            Ok(service) => Ok(service),
            Err(e) if cfg.refresh_token.is_some() => {
                warn!("cTrader connection failed ({e:#}); refreshing access token and retrying");
                match self.refresh_ctrader_tokens(cfg).await {
                    Ok(cfg) => CTraderService::connect(cfg).await,
                    Err(e) => Err(e),
                }
            }
            Err(e) => Err(e),
        };
        match result {
            Ok(service) => {
                self.install_ctrader(service, account_changed).await;
                Ok(())
            }
            Err(e) => {
                self.metrics.write().await.connection_status.ctrader = "disconnected".to_string();
                Err(e)
            }
        }
    }

    /// 接続済みサービスを state に反映する（発注先・接続表示・残高同期）
    pub async fn install_ctrader(&self, service: CTraderService, account_changed: bool) {
        info!("Successfully connected and authenticated with cTrader!");
        let cfg = service.config().clone();
        let service_arc = Arc::new(service);
        *self.ctrader_service.write().await = Some(service_arc.clone());
        *self.ctrader_config.write().await = Some(cfg.clone());
        info!("cTrader connected: live order sink installed ({})", if cfg.is_live { "LIVE" } else { "DEMO" });
        *self.order_sink.write().await = self.build_order_sink(service_arc).await;
        {
            let mut m_lock = self.metrics.write().await;
            m_lock.connection_status.ctrader = "connected".to_string();
            m_lock.connection_status.environment = if cfg.is_live { "LIVE".to_string() } else { "DEMO".to_string() };
            m_lock.connection_status.account_number =
                format!("cTrader #{} ({})", cfg.account_id, if cfg.is_live { "Live" } else { "Demo" });
        }
        if account_changed {
            self.on_account_connected().await;
        } else {
            self.sync_broker_account().await;
        }
    }

    /// ブローカー側の建玉を成行で決済する。
    ///
    /// FIX 発注を使っている場合は FIX 経由で決済し、併せて保護注文（SL/TP）を取り消す。
    /// ネッティング口座では、建玉が無くなった後に保護注文が残っていると発動時に
    /// 逆建玉を作ってしまうため、この取り消しは省略できない。
    pub async fn close_broker_position(&self, position: &Position) -> Result<()> {
        let entry_is_buy = position.side == "BUY";
        let fix = self.fix_client.read().await.clone();
        let service = self.ctrader_service.read().await.clone();
        let Some(service) = service else {
            anyhow::bail!("cTrader is not connected; cannot close position {}", position.id);
        };

        if let Some(fix) = fix {
            let symbol_id = service.get_symbol_id(&position.symbol).await?;
            let units = crate::fix::lots_to_units(position.volume_lots);
            fix.close_position(&position.id, symbol_id, entry_is_buy, units)
                .await
                .with_context(|| format!("Failed to close position {} over FIX", position.id))?;
            return Ok(());
        }

        let volume = crate::ctrader::lots_to_volume(position.volume_lots);
        service
            .place_market_order_with_sltp(&position.symbol, !entry_is_buy, volume, None, None)
            .await
            .with_context(|| format!("Failed to close position {} over Open API", position.id))?;
        Ok(())
    }

    /// 発注先を決める。
    ///
    /// FIX の接続情報（`CTRADER_FIX_*`）が設定されていればそちらを使う。ブローカーが
    /// Open API の取引を無効化している（新規注文に TRADING_DISABLED が返る）場合の
    /// 発注経路で、データ取得・口座情報・ポジション照合は Open API のまま。
    /// 接続に失敗したときは Open API 発注にフォールバックする。
    async fn build_order_sink(&self, service: Arc<CTraderService>) -> Arc<dyn OrderSink> {
        match crate::fix::FixConfig::from_env() {
            Ok(Some(fix_cfg)) => {
                info!("FIX credentials found; connecting FIX trading session for order execution");
                match crate::fix::FixTradingClient::connect(fix_cfg).await {
                    Ok(fix) => {
                        info!("FIX session established: orders will be placed over FIX");
                        *self.fix_client.write().await = Some(Arc::new(fix));
                        let fix = self.fix_client.read().await.clone().expect("just installed");
                        return Arc::new(FixOrderSink::new(fix, service));
                    }
                    Err(e) => warn!("FIX connection failed ({e:#}); falling back to Open API orders"),
                }
            }
            Ok(None) => debug!("No FIX credentials configured; using Open API for orders"),
            Err(e) => warn!("Invalid FIX configuration ({e:#}); using Open API for orders"),
        }
        Arc::new(CTraderOrderSink::new(service))
    }

    /// Refresh Token で Access Token を更新し、SQLite に保存して設定に反映する
    async fn refresh_ctrader_tokens(&self, cfg: CTraderConfig) -> Result<CTraderConfig> {
        let refresh_token = cfg.refresh_token.clone().context("cTrader refresh token is not available")?;
        info!("Refreshing cTrader access token...");
        let tokens = token::refresh_tokens(&cfg.client_id, &cfg.client_secret, &refresh_token).await?;
        info!(expires_at = ?tokens.expires_at.map(crate::storage::fmt_ts), "cTrader access token refreshed");
        self.save_ctrader_tokens(&tokens).await;
        let cfg = cfg.with_tokens(&tokens);
        *self.ctrader_config.write().await = Some(cfg.clone());
        Ok(cfg)
    }

    /// トークンを SQLite に保存する。更新後は古いトークンが無効になるため、失敗時は .env に退避する
    pub async fn save_ctrader_tokens(&self, tokens: &TokenSet) {
        let path = self.db_path.clone();
        let t = tokens.clone();
        let res = tokio::task::spawn_blocking(move || Db::open(path)?.save_ctrader_tokens(&t)).await;
        match res {
            Ok(Ok(())) => info!("cTrader tokens saved to SQLite"),
            Ok(Err(e)) => self.save_tokens_to_env_fallback(tokens, &format!("{e:#}")),
            Err(e) => self.save_tokens_to_env_fallback(tokens, &e.to_string()),
        }
    }

    fn save_tokens_to_env_fallback(&self, tokens: &TokenSet, reason: &str) {
        error!("Failed to save cTrader tokens to SQLite ({reason}); writing them to .env instead");
        let _ = Self::update_env_file("CTRADER_ACCESS_TOKEN", &tokens.access_token);
        if let Some(r) = &tokens.refresh_token {
            let _ = Self::update_env_file("CTRADER_REFRESH_TOKEN", r);
        }
    }

    /// 期限が近ければトークンを更新して再接続する。未接続なら再接続を試みる（常駐ループから呼ぶ）
    pub async fn refresh_ctrader_tokens_if_due(&self) {
        let Some(cfg) = self.ctrader_config.read().await.clone() else { return };
        let connected = self.ctrader_service.read().await.is_some();
        if !connected {
            info!("cTrader is not connected; retrying connection");
            if let Err(e) = self.connect_ctrader(cfg, true).await {
                warn!("cTrader reconnection failed: {e:#}");
            }
            return;
        }
        if cfg.refresh_token.is_none() {
            if cfg.token_expires_at.is_none() {
                debug!("No cTrader refresh token; automatic token refresh is disabled (re-link via OAuth)");
            }
            return;
        }
        if !cfg.tokens().needs_refresh(chrono::Utc::now().timestamp()) {
            return;
        }
        // 判断サイクルの途中で接続を差し替えないよう直列化する
        let _serial = self.decide_lock.lock().await;
        let result = async {
            let cfg = self.refresh_ctrader_tokens(cfg).await?;
            let service = CTraderService::connect(cfg).await?;
            self.install_ctrader(service, false).await;
            anyhow::Ok(())
        }
        .await;
        if let Err(e) = result {
            warn!("Scheduled cTrader token refresh failed: {e:#}");
        }
    }

    /// トークン更新の常駐ループ（30分ごとに期限を確認）
    pub async fn run_token_refresh_loop(self) {
        const INTERVAL: std::time::Duration = std::time::Duration::from_secs(30 * 60);
        // 起動直後の初期接続と競合しないよう少し待つ
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        loop {
            self.refresh_ctrader_tokens_if_due().await;
            tokio::time::sleep(INTERVAL).await;
        }
    }

    /// 口座の接続・切り替え直後に呼ぶ。ブローカー残高を同期する。
    pub async fn on_account_connected(&self) {
        self.sync_broker_account().await;
    }

    /// cTrader から口座残高を取得して metrics に反映する。
    pub async fn sync_broker_account(&self) {
        let Some(ctrader) = self.ctrader_service.read().await.clone() else { return };
        match ctrader.get_account_info().await {
            Ok(acc) => {
                let mut m = self.metrics.write().await;
                m.broker_balance = Some(acc.balance);
                m.balance = acc.balance;
                m.equity = m.balance + m.unrealized_pnl;
                m.free_margin = m.equity - m.margin;
                if let Some(login) = acc.trader_login {
                    m.connection_status.account_number =
                        format!("cTrader #{} ({})", login, if acc.is_live { "Live" } else { "Demo" });
                }
                *self.last_broker_sync.write().await = Some(std::time::Instant::now());
                info!(balance = acc.balance, leverage = ?acc.leverage, "broker account synced");
            }
            Err(e) => warn!("failed to sync broker account: {e:#}"),
        }
    }

    /// 直近の同期から `max_age` 以上経っていれば同期する（ポーリング用）
    pub async fn sync_broker_account_if_stale(&self, max_age: std::time::Duration) {
        let stale = self
            .last_broker_sync
            .read()
            .await
            .map(|t| t.elapsed() >= max_age)
            .unwrap_or(true);
        if stale {
            self.sync_broker_account().await;
        }
    }

    /// .env ファイル内の指定されたキーの値を更新・保存する
    pub fn update_env_file(key: &str, val: &str) -> Result<()> {
        let env_path = PathBuf::from(".env");
        let content = if env_path.exists() {
            fs::read_to_string(&env_path).context("Failed to read .env file")?
        } else {
            String::new()
        };

        let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        let target_prefix = format!("{}=", key);
        let mut found = false;

        for line in lines.iter_mut() {
            let trimmed = line.trim();
            if trimmed.starts_with(&target_prefix) || trimmed.starts_with(&format!("# {}", target_prefix)) {
                *line = format!("{}=\"{}\"", key, val);
                found = true;
                break;
            }
        }

        if !found {
            lines.push(format!("{}=\"{}\"", key, val));
        }

        let new_content = lines.join("\n") + "\n";
        fs::write(&env_path, new_content).context("Failed to write to .env file")?;
        info!("Updated {} in .env successfully.", key);
        Ok(())
    }

    /// SQLite を開いて `f` を専用スレッドで実行する
    pub async fn with_db<T, F>(&self, f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Db) -> Result<T> + Send + 'static,
    {
        let path = self.db_path.clone();
        tokio::task::spawn_blocking(move || f(&mut Db::open(path)?)).await?
    }

    /// 現在の保有ポジションを SQLite に書き出す。`positions` を変更したら呼ぶ
    pub async fn persist_positions(&self) {
        let _serial = self.positions_persist_lock.lock().await;
        let snapshot = self.positions.read().await.clone();
        if let Err(e) = self.with_db(move |db| db.replace_positions(&snapshot)).await {
            error!("Failed to persist positions to SQLite: {e:#}");
        }
    }

    /// 決済履歴を SQLite に記録する
    pub async fn record_trades(&self, trades: Vec<TradeHistory>) {
        if trades.is_empty() {
            return;
        }
        let res = self
            .with_db(move |db| trades.iter().try_for_each(|t| db.insert_trade(t)))
            .await;
        if let Err(e) = res {
            error!("Failed to record trades to SQLite: {e:#}");
        }
    }

    /// 判断ログを SQLite に記録する
    pub async fn record_cot_log(&self, log: CoTLog) {
        if let Err(e) = self.with_db(move |db| db.insert_cot_log(&log)).await {
            error!("Failed to record CoT log to SQLite: {e:#}");
        }
    }

    /// 起動時の口座メトリクス。cTrader 接続後にブローカー残高で上書きされる（`sync_broker_account`）
    fn initial_metrics() -> AccountMetrics {
        AccountMetrics {
            bot_state: BotState::Running,
            balance: 0.0,
            equity: 0.0,
            margin: 0.0,
            free_margin: 0.0,
            daily_pnl: 0.0,
            daily_pnl_percent: 0.0,
            unrealized_pnl: 0.0,
            win_rate_today: 0.0,
            total_trades_today: 0,
            winning_trades_today: 0,
            usdjpy_spread: 0.0,
            eurusd_spread: 0.0,
            circuit_breaker_threshold_percent: -3.0,
            order_mode: "live".to_string(),
            broker_balance: None,
            connection_status: ConnectionStatus {
                ctrader: "connecting".to_string(),
                llm: "ready".to_string(),
                ping_ms: 0,
                environment: "DEMO".to_string(),
                account_number: "未連携".to_string(),
            },
        }
    }
}
