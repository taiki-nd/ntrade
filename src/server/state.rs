use anyhow::{Context, Result};
use chrono::{TimeZone, Utc};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::ctrader::{token, BarPeriod, CTraderConfig, CTraderService, TokenSet};
use crate::executor::{CTraderOrderSink, DisconnectedOrderSink, OrderSink, PlanBook};
use crate::guard::GuardConfig;
use crate::llm::{LlmClient, LlmClientConfig};
use crate::snapshot::measures;
use crate::settings::TradingSettings;
use crate::snapshot::SnapshotBundle;
use crate::storage::Db;
use super::types::{
    AccountInfo, AccountMetrics, BotState, CloseReason, ConnectionStatus, CoTLog, Position, TradeHistory,
};

#[derive(Clone)]
pub struct AppState {
    pub bot_state: Arc<RwLock<BotState>>,
    pub metrics: Arc<RwLock<AccountMetrics>>,
    pub positions: Arc<RwLock<Vec<Position>>>,
    pub ctrader_service: Arc<RwLock<Option<Arc<CTraderService>>>>,
    pub ctrader_config: Arc<RwLock<Option<CTraderConfig>>>,
    pub available_accounts: Arc<RwLock<Vec<AccountInfo>>>,
    /// ペアごとの直近の Snapshot（客観的事実 + 画像4枚）
    pub latest_snapshots: Arc<RwLock<HashMap<String, SnapshotBundle>>>,
    /// SQLite（ヒストリカルバー・リプレイ結果・判断ログ・ポジション・決済履歴・教訓）
    pub db_path: PathBuf,
    /// `positions` の SQLite 書き出しを直列化する（古いスナップショットで上書きしないため）
    positions_persist_lock: Arc<tokio::sync::Mutex<()>>,
    /// 画面から変更できる取引設定（対象ペア・待ち秒数・事後ガード）。SQLite の app_settings が正
    pub settings: Arc<RwLock<TradingSettings>>,
    /// 保持中の条件付きプラン
    pub plan_book: Arc<RwLock<PlanBook>>,
    /// LLM 推論クライアント
    pub llm: Arc<LlmClient>,
    /// 発注先（cTrader 接続時は CTraderOrderSink、未接続時は DisconnectedOrderSink）
    pub order_sink: Arc<RwLock<Arc<dyn OrderSink>>>,
    /// 判断サイクル（読み取り側・ペア間で並行可）と cTrader 接続の差し替え（書き込み側）の排他
    pub cycle_gate: Arc<RwLock<()>>,
    /// 同じペアの判断サイクルを重ねない（スケジューラと手動実行の衝突防止）
    pair_locks: Arc<std::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>,
    /// ガード評価（建玉数・日次損失）から発注・建玉記録までを直列化する。
    /// ペアを並行に回すと、両方が「建玉 0」を見て全体上限を超えて発注しうるため
    pub order_lock: Arc<tokio::sync::Mutex<()>>,
    /// ブローカー残高の最終同期時刻
    pub last_broker_sync: Arc<RwLock<Option<std::time::Instant>>>,
    /// ブローカーとの建玉照合の直列化（常駐ループと判断サイクル後の照合が重ならないように）
    reconcile_lock: Arc<tokio::sync::Mutex<()>>,
}

/// ブローカー側で決済された建玉を決済履歴に変換する。
///
/// 決済価格・決済時刻・実現損益は cTrader の約定（closing deal）から取る。ローカルの足終値から
/// 推定すると、決済から検知までの間に価格が動いた分だけ cTrader の履歴とずれるため。
/// 約定が引けなかった場合のみ、最後に把握していた価格からの概算にフォールバックする。
async fn closed_trade(ctrader: &CTraderService, p: Position) -> TradeHistory {
    let deal = match p.id.parse::<i64>() {
        Ok(position_id) => {
            let from = crate::storage::parse_ts(&p.open_time)
                .ok()
                .and_then(|s| Utc.timestamp_opt(s, 0).single())
                .unwrap_or_else(|| Utc::now() - chrono::Duration::days(30))
                - chrono::Duration::hours(1);
            match ctrader.get_position_close(position_id, from).await {
                Ok(Some(d)) => Some(d),
                Ok(None) => {
                    warn!(position = %p.id, "no closing deal found; falling back to last known price");
                    None
                }
                Err(e) => {
                    warn!(position = %p.id, "failed to fetch closing deal: {e:#}");
                    None
                }
            }
        }
        Err(_) => None,
    };

    let pip = measures::get_pip_size(&p.symbol);
    let sign = if p.side == "BUY" { 1.0 } else { -1.0 };
    let pip_value = match &deal {
        Some(_) => 0.0,
        None => pip_value_or_estimate(ctrader, &p.symbol).await,
    };
    let (entry_price, close_price, close_time, pnl_amount) = match &deal {
        Some(d) => (
            d.entry_price,
            d.close_price,
            d.close_time.format("%Y-%m-%d %H:%M:%S").to_string(),
            d.net_profit,
        ),
        None => {
            let pnl_pips = (p.current_price - p.entry_price) * sign / pip;
            (
                p.entry_price,
                p.current_price,
                Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                (pnl_pips * pip_value * p.volume_lots).round(),
            )
        }
    };
    let pnl_pips = ((close_price - entry_price) * sign / pip * 10.0).round() / 10.0;

    let close_reason = classify_close_reason(&p.side, close_price, p.stop_loss, p.take_profit);

    if let Some(d) = &deal {
        info!(
            position = %p.id, deal = d.deal_id, close_price, gross = d.gross_profit,
            swap = d.swap, commission = d.commission, net = d.net_profit,
            "closing deal resolved from broker"
        );
    }

    TradeHistory {
        id: format!("trd-{}", p.id),
        symbol: p.symbol,
        side: p.side,
        volume_lots: deal.as_ref().map(|d| d.volume_lots).filter(|v| *v > 0.0).unwrap_or(p.volume_lots),
        entry_price,
        close_price,
        stop_loss: p.stop_loss,
        take_profit: p.take_profit,
        pnl_pips,
        pnl_amount,
        close_reason,
        open_time: p.open_time,
        close_time,
        cot_log_id: p.cot_log_id,
    }
}

/// 決済価格を建玉の SL/TP と突き合わせて決済理由を決める。
///
/// SL/TP はブローカー側で執行されるので、ntrade からは「どちらに当たったか」を価格でしか
/// 判定できない。どちらにも当たっていなければ手動決済（＝ダッシュボード操作か cTrader 側の操作）。
/// 前提として建玉の SL/TP がブローカーの持つ値と一致している必要がある（`adopt_broker_protection`）。
fn classify_close_reason(side: &str, close_price: f64, stop_loss: f64, take_profit: f64) -> CloseReason {
    let is_buy = side == "BUY";
    if stop_loss > 0.0 && ((is_buy && close_price <= stop_loss) || (!is_buy && close_price >= stop_loss)) {
        CloseReason::StopLoss
    } else if take_profit > 0.0 && ((is_buy && close_price >= take_profit) || (!is_buy && close_price <= take_profit)) {
        CloseReason::TakeProfit
    } else {
        CloseReason::Manual
    }
}

/// ブローカー側が実際に持っている SL/TP をローカルの建玉に取り込む。
///
/// 決済理由（SL/TP/手動）の判定は建玉に記録された SL/TP との比較で行うため、ここがずれると
/// 「SL で刈られたのに手動決済と表示される」ことになる。ブローカーが SL/TP を持っていない
/// 場合は 0.0 を入れる（＝無防備であることを記録に残す）。
fn adopt_broker_protection(p: &mut Position, b: &crate::ctrader::types::BrokerPosition) {
    let (sl, tp) = (b.stop_loss.unwrap_or(0.0), b.take_profit.unwrap_or(0.0));
    if (p.stop_loss - sl).abs() < 1e-9 && (p.take_profit - tp).abs() < 1e-9 {
        return;
    }
    warn!(
        position = %p.id, local_sl = p.stop_loss, broker_sl = sl, local_tp = p.take_profit, broker_tp = tp,
        "protection levels differ from broker; adopting the broker's"
    );
    p.stop_loss = sl;
    p.take_profit = tp;
}

/// 1 lot・1 pip の価値（口座通貨）。ブローカーの銘柄仕様とレートから求め、取れなければ固定の概算に倒す
async fn pip_value_or_estimate(ctrader: &CTraderService, symbol: &str) -> f64 {
    match ctrader.pip_value_per_lot(symbol).await {
        Ok(v) => v,
        Err(e) => {
            warn!(%symbol, "failed to resolve pip value from broker; using a fixed estimate: {e:#}");
            measures::pip_value_per_lot(symbol)
        }
    }
}

/// 保有中の建玉の現在値と含み損益を更新する。含み損益はブローカーの確定値（手数料込み）を使い、
/// 取れなかった場合のみ pip 換算にフォールバックする。
async fn refresh_unrealized(ctrader: &CTraderService, positions: &mut [Position]) {
    if positions.is_empty() {
        return;
    }
    let broker_pnl: HashMap<i64, f64> = match ctrader.get_unrealized_pnl().await {
        Ok(m) => m,
        Err(e) => {
            warn!("failed to fetch unrealized PnL; estimating from price: {e:#}");
            HashMap::new()
        }
    };

    let mut prices: HashMap<String, f64> = HashMap::new();
    let symbols: Vec<String> = {
        let mut s: Vec<String> = positions.iter().map(|p| p.symbol.clone()).collect();
        s.sort();
        s.dedup();
        s
    };
    for symbol in symbols {
        match ctrader.get_trendbars(&symbol, BarPeriod::M1, 1).await {
            Ok(bars) => {
                if let Some(bar) = bars.last() {
                    prices.insert(symbol, bar.close);
                }
            }
            Err(e) => warn!(%symbol, "failed to fetch latest price for open position: {e:#}"),
        }
    }

    let mut pip_values: HashMap<String, f64> = HashMap::new();
    for p in positions.iter() {
        let missing = p.id.parse::<i64>().ok().and_then(|id| broker_pnl.get(&id)).is_none();
        if missing && !pip_values.contains_key(&p.symbol) {
            pip_values.insert(p.symbol.clone(), pip_value_or_estimate(ctrader, &p.symbol).await);
        }
    }

    for p in positions.iter_mut() {
        if let Some(price) = prices.get(&p.symbol) {
            p.current_price = *price;
        }
        let sign = if p.side == "BUY" { 1.0 } else { -1.0 };
        let pip = measures::get_pip_size(&p.symbol);
        p.pnl_pips = ((p.current_price - p.entry_price) * sign / pip * 10.0).round() / 10.0;
        p.pnl_amount = match p.id.parse::<i64>().ok().and_then(|id| broker_pnl.get(&id)) {
            Some(v) => *v,
            None => (p.pnl_pips * pip_values.get(&p.symbol).copied().unwrap_or_default() * p.volume_lots).round(),
        };
    }
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
        let settings = TradingSettings::load_or_seed(&db_path);
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
            latest_snapshots: Arc::new(RwLock::new(HashMap::new())),
            db_path,
            positions_persist_lock: Arc::new(tokio::sync::Mutex::new(())),
            settings: Arc::new(RwLock::new(settings)),
            plan_book: Arc::new(RwLock::new(PlanBook::default())),
            llm: Arc::new(LlmClient::new(LlmClientConfig::default())),
            order_sink: Arc::new(RwLock::new(Arc::new(DisconnectedOrderSink))),
            cycle_gate: Arc::new(RwLock::new(())),
            pair_locks: Arc::new(std::sync::Mutex::new(HashMap::new())),
            order_lock: Arc::new(tokio::sync::Mutex::new(())),
            last_broker_sync: Arc::new(RwLock::new(None)),
            reconcile_lock: Arc::new(tokio::sync::Mutex::new(())),
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
    /// 反対売買の新規成行ではなく position_id 指定の決済を使う。反対売買では
    /// ヘッジ口座で建玉が相殺されず、SL/TP を抱えた元の建玉が残ってしまうため。
    pub async fn close_broker_position(&self, position: &Position) -> Result<()> {
        let service = self.ctrader_service.read().await.clone();
        let Some(service) = service else {
            anyhow::bail!("cTrader is not connected; cannot close position {}", position.id);
        };
        let position_id: i64 = position
            .id
            .parse()
            .with_context(|| format!("Position id {} is not a cTrader position id", position.id))?;

        let volume = crate::ctrader::lots_to_volume(position.volume_lots);
        service
            .close_position_by_id(position_id, volume)
            .await
            .with_context(|| format!("Failed to close position {} over Open API", position.id))?;
        Ok(())
    }

    /// 発注先を決める。
    async fn build_order_sink(&self, service: Arc<CTraderService>) -> Arc<dyn OrderSink> {
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
        // 判断サイクルの途中で接続を差し替えないよう、実行中のサイクルが終わるのを待つ
        let _gate = self.cycle_gate.write().await;
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

    // ------------------------------------------------------------- 建玉の照合

    /// ブローカーとの建玉照合。判断サイクルとは独立に呼べるようにしてある。
    ///
    /// SL/TP はブローカー側で執行されるため、決済は「ブローカーから建玉が消えたこと」でしか
    /// 検知できない。判断サイクルの後にしか照合しないと、停止中・サイクル間・再起動直後の決済を
    /// 取りこぼし、決済済みの建玉が残って純資産がずれる。
    ///
    /// 戻り値は今回はじめて確定した決済（自己反省の対象）。
    pub async fn reconcile_positions(&self) -> Result<Vec<TradeHistory>> {
        let _serial = self.reconcile_lock.lock().await;
        let Some(ctrader) = self.ctrader_service.read().await.clone() else {
            return Ok(Vec::new());
        };

        self.sync_broker_account().await;
        let broker = ctrader.get_open_positions().await.context("Failed to reconcile positions")?;

        // 1. ローカルにあってブローカーに無い建玉 = 決済済み
        let local = self.positions.read().await.clone();
        let mut remaining: Vec<Position> = Vec::new();
        let mut closed: Vec<TradeHistory> = Vec::new();
        for mut p in local {
            match broker.iter().find(|b| b.position_id.to_string() == p.id) {
                Some(b) => {
                    adopt_broker_protection(&mut p, b);
                    // 以前は銘柄名から口座のサフィックスを外して記録していた（USDJPY_z → USDJPY）。
                    // 設定のペアとの照合（ペアごとの建玉数など）がずれないよう、ブローカーの表記にそろえる
                    if let Some(name) = b.symbol_name.as_ref().filter(|n| **n != p.symbol) {
                        info!(position = %p.id, from = %p.symbol, to = %name, "position symbol aligned with broker");
                        p.symbol = name.clone();
                    }
                    remaining.push(p);
                }
                None => closed.push(closed_trade(&ctrader, p).await),
            }
        }

        // 2. ブローカーにあってローカルに無い建玉（発注は通ったが記録に失敗した等）を取り込む
        let orphans: Vec<_> = broker
            .iter()
            .filter(|b| !remaining.iter().any(|p| p.id == b.position_id.to_string()))
            .cloned()
            .collect();
        for b in orphans {
            let symbol = b.symbol_name.clone().unwrap_or_else(|| format!("symbol-{}", b.symbol_id));
            warn!(position_id = b.position_id, symbol = %symbol, "orphan broker position adopted (not tracked locally)");
            let entry = b.entry_price.unwrap_or_default();
            remaining.push(Position {
                id: b.position_id.to_string(),
                symbol,
                side: if b.is_buy { "BUY".into() } else { "SELL".into() },
                volume_lots: b.volume_lots,
                entry_price: entry,
                current_price: entry,
                stop_loss: b.stop_loss.unwrap_or_default(),
                take_profit: b.take_profit.unwrap_or_default(),
                pnl_pips: 0.0,
                pnl_amount: 0.0,
                open_time: b.open_time.unwrap_or_else(Utc::now).format("%Y-%m-%d %H:%M:%S").to_string(),
                invalidation_reason: "ブローカー側から取り込んだ建玉（ntrade の記録に無し）".into(),
                cot_log_id: None,
                close_stop: None,
            });
        }

        // 3. 残った建玉の含み損益をブローカーの値で更新する
        refresh_unrealized(&ctrader, &mut remaining).await;

        *self.positions.write().await = remaining;
        self.persist_positions().await;

        Ok(self.record_closed_trades(closed).await)
    }

    /// 単一建玉の手動成行決済。ブローカーの約定を待ってから決済履歴を確定させるので、
    /// 決済に失敗した場合は建玉をローカルから消さない。
    pub async fn close_position_now(&self, id: &str) -> Result<TradeHistory> {
        self.close_position_as(id, None).await
    }

    /// 単一建玉の成行決済。`reason` を渡すと、決済価格からの推定ではなくその理由で記録する。
    pub async fn close_position_as(&self, id: &str, reason: Option<CloseReason>) -> Result<TradeHistory> {
        // 照合ループが先に「ブローカーから消えた建玉」として別の理由で取り込まないよう、照合と直列にする
        let _serial = self.reconcile_lock.lock().await;
        let target = self
            .positions
            .read()
            .await
            .iter()
            .find(|p| p.id == id)
            .cloned()
            .with_context(|| format!("Position {id} not found"))?;

        self.close_broker_position(&target).await?;

        let ctrader = self
            .ctrader_service
            .read()
            .await
            .clone()
            .context("cTrader is not connected")?;
        let mut trade = closed_trade(&ctrader, target).await;
        if let Some(r) = reason {
            trade.close_reason = r;
        }

        self.positions.write().await.retain(|p| p.id != id);
        self.persist_positions().await;
        Ok(self.record_closed_trades(vec![trade.clone()]).await.into_iter().next().unwrap_or(trade))
    }

    /// 決済履歴を SQLite に記録し、まだ記録されていなかったものだけを返す。
    /// 同じ決済を複数回照合しても口座メトリクスを二重計上しないため。
    pub async fn record_closed_trades(&self, trades: Vec<TradeHistory>) -> Vec<TradeHistory> {
        if trades.is_empty() {
            return Vec::new();
        }
        let fresh = self
            .with_db(move |db| {
                let mut fresh = Vec::new();
                for t in trades {
                    let already_known = db.trade(&t.id)?.is_some();
                    db.insert_trade(&t)?;
                    if !already_known {
                        fresh.push(t);
                    }
                }
                Ok(fresh)
            })
            .await;
        let fresh = match fresh {
            Ok(fresh) => fresh,
            Err(e) => {
                error!("Failed to record trades to SQLite: {e:#}");
                return Vec::new();
            }
        };

        let mut metrics = self.metrics.write().await;
        for t in &fresh {
            metrics.daily_pnl += t.pnl_amount;
            metrics.daily_pnl_percent =
                if metrics.balance > 0.0 { metrics.daily_pnl / metrics.balance * 100.0 } else { 0.0 };
            metrics.total_trades_today += 1;
            if t.pnl_amount > 0.0 {
                metrics.winning_trades_today += 1;
            }
            metrics.win_rate_today = if metrics.total_trades_today > 0 {
                metrics.winning_trades_today as f64 / metrics.total_trades_today as f64 * 100.0
            } else {
                0.0
            };
            info!(id = %t.id, ?t.close_reason, pnl_pips = t.pnl_pips, pnl = t.pnl_amount, "position closed on broker");
        }
        drop(metrics);
        fresh
    }

    /// 建玉照合の常駐ループ。判断サイクルや bot の稼働状態に依存せず決済を取り込む。
    pub async fn run_reconcile_loop(self) {
        const INTERVAL: std::time::Duration = std::time::Duration::from_secs(60);
        loop {
            tokio::time::sleep(INTERVAL).await;
            match self.reconcile_positions().await {
                Ok(closed) => crate::scheduler::reflect_on_closed(&self, closed).await,
                Err(e) => warn!("reconcile failed: {e:#}"),
            }
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

    // ------------------------------------------------------------- 取引設定

    pub async fn guard_config(&self) -> GuardConfig {
        self.settings.read().await.guard.clone()
    }

    /// 判断サイクルの対象ペア
    pub async fn pairs(&self) -> Vec<String> {
        self.settings.read().await.pairs.clone()
    }

    /// API のペア指定を解決する。設定にあるペアなら設定の表記にそろえ、省略時は設定の先頭ペア
    pub async fn resolve_pair(&self, pair: Option<&str>) -> String {
        let settings = self.settings.read().await;
        match pair.map(str::trim).filter(|p| !p.is_empty()) {
            Some(p) => settings.pairs.iter().find(|s| s.eq_ignore_ascii_case(p)).cloned().unwrap_or_else(|| p.to_string()),
            None => settings.default_pair(),
        }
    }

    /// 設定を検証して SQLite に保存し、次の判断サイクルから反映する
    pub async fn save_settings(&self, mut next: TradingSettings) -> Result<TradingSettings> {
        next.normalize();
        next.validate()?;
        // 接続中なら、ブローカーに存在しない銘柄を保存前に弾き、表記をブローカーの一覧どおりにそろえる
        // （usdjpy_z → USDJPY_z）。保存後にサイクルが失敗し続けるのを防ぐため
        if let Some(ctrader) = self.ctrader_service.read().await.clone() {
            let mut unknown = Vec::new();
            for p in next.pairs.iter_mut() {
                match ctrader.canonical_symbol(p).await {
                    Ok(name) => *p = name,
                    Err(e) => unknown.push(format!("{e:#}")),
                }
            }
            if !unknown.is_empty() {
                anyhow::bail!("cTrader に存在しない銘柄です: {}", unknown.join(" / "));
            }
        }
        let saved = next.clone();
        self.with_db(move |db| db.save_setting(crate::settings::SETTINGS_KEY, &saved)).await?;
        let prev_pairs = std::mem::replace(&mut *self.settings.write().await, next.clone()).pairs;
        info!(pairs = ?next.pairs, ?prev_pairs, "trading settings updated");
        Ok(next)
    }

    /// ペア単位のロック（同じペアの判断サイクルを重ねないため）
    pub fn pair_lock(&self, pair: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self.pair_locks.lock().unwrap_or_else(|e| e.into_inner());
        locks.entry(pair.to_uppercase()).or_default().clone()
    }

    /// 1 lot・1 pip の価値（口座通貨）をブローカーのレートから求める。未接続・取得失敗なら None
    pub async fn live_pip_value(&self, pair: &str) -> Option<f64> {
        let ctrader = self.ctrader_service.read().await.clone()?;
        match ctrader.pip_value_per_lot(pair).await {
            Ok(v) => Some(v),
            Err(e) => {
                warn!(pair, "failed to resolve pip value from broker: {e:#}");
                None
            }
        }
    }

    /// 発注ロット。リスク % 指定のときだけブローカーから pip 価値を引く
    pub async fn order_volume(&self, cfg: &GuardConfig, pair: &str, sl_pips: f64) -> f64 {
        let balance = self.metrics.read().await.balance;
        let live = if cfg.risk_pct.is_some() && cfg.pip_value_per_lot.is_none() {
            self.live_pip_value(pair).await
        } else {
            None
        };
        cfg.volume_lots(balance, sl_pips, live)
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
            spreads: Default::default(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctrader::types::BrokerPosition;

    fn position(sl: f64, tp: f64) -> Position {
        Position {
            id: "10871878".into(),
            symbol: "USDJPY".into(),
            side: "SELL".into(),
            volume_lots: 0.01,
            entry_price: 157.229,
            current_price: 157.229,
            stop_loss: sl,
            take_profit: tp,
            pnl_pips: 0.0,
            pnl_amount: 0.0,
            open_time: "2026-09-25 16:51:30".into(),
            invalidation_reason: String::new(),
            cot_log_id: None,
            close_stop: None,
        }
    }

    fn broker(sl: Option<f64>, tp: Option<f64>) -> BrokerPosition {
        BrokerPosition {
            position_id: 10871878,
            symbol_id: 1,
            symbol_name: Some("USDJPY".into()),
            is_buy: false,
            volume_lots: 0.01,
            entry_price: Some(157.229),
            stop_loss: sl,
            take_profit: tp,
            open_time: None,
        }
    }

    #[test]
    fn close_reason_follows_the_levels_actually_held_by_the_broker() {
        // 実例（trd-10871878）: 計画上の SL は 157.600 だが、成行約定のズレで
        // ブローカーには 157.379 が置かれていた。計画値のままだと手動決済に見えてしまう。
        assert!(matches!(classify_close_reason("SELL", 157.379, 157.600, 156.950), CloseReason::Manual));
        assert!(matches!(classify_close_reason("SELL", 157.379, 157.379, 156.729), CloseReason::StopLoss));

        assert!(matches!(classify_close_reason("SELL", 156.900, 157.600, 156.950), CloseReason::TakeProfit));
        assert!(matches!(classify_close_reason("BUY", 157.700, 157.000, 157.600), CloseReason::TakeProfit));
        assert!(matches!(classify_close_reason("BUY", 156.900, 157.000, 157.600), CloseReason::StopLoss));
        // SL/TP 未設定（0.0）の建玉は価格で判定できない
        assert!(matches!(classify_close_reason("SELL", 157.379, 0.0, 0.0), CloseReason::Manual));
    }

    #[test]
    fn broker_protection_overrides_the_planned_levels() {
        let mut p = position(157.600, 156.950);
        adopt_broker_protection(&mut p, &broker(Some(157.379), Some(156.729)));
        assert_eq!(p.stop_loss, 157.379);
        assert_eq!(p.take_profit, 156.729);

        // ブローカーが SL/TP を持っていないなら、無防備であることをそのまま記録する
        let mut p = position(157.600, 156.950);
        adopt_broker_protection(&mut p, &broker(None, None));
        assert_eq!(p.stop_loss, 0.0);
        assert_eq!(p.take_profit, 0.0);
    }
}
