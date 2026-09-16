use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::ctrader::{CTraderConfig, CTraderService};
use crate::executor::{CTraderOrderSink, OrderSink, PaperOrderSink, PlanBook};
use crate::guard::{GuardConfig, DEFAULT_GUARD_CONFIG_PATH};
use crate::llm::{LlmClient, LlmClientConfig};
use crate::snapshot::SnapshotBundle;
use super::types::{
    AccountInfo, AccountMetrics, BotState, ConnectionStatus, CoTLog, LessonLearned,
    Position, TradeHistory,
};

#[derive(Clone)]
pub struct AppState {
    pub bot_state: Arc<RwLock<BotState>>,
    pub metrics: Arc<RwLock<AccountMetrics>>,
    pub positions: Arc<RwLock<Vec<Position>>>,
    pub trades: Arc<RwLock<Vec<TradeHistory>>>,
    pub cot_logs: Arc<RwLock<Vec<CoTLog>>>,
    pub lessons: Arc<RwLock<Vec<LessonLearned>>>,
    pub ctrader_service: Arc<RwLock<Option<Arc<CTraderService>>>>,
    pub ctrader_config: Arc<RwLock<Option<CTraderConfig>>>,
    pub available_accounts: Arc<RwLock<Vec<AccountInfo>>>,
    /// 直近に生成した Snapshot（客観的事実 + 画像4枚）
    pub latest_snapshot: Arc<RwLock<Option<SnapshotBundle>>>,
    /// SQLite（ヒストリカルバー・リプレイ結果）
    pub db_path: PathBuf,
    /// 事後ガード設定（config/guard.toml）
    pub guard_config: Arc<RwLock<GuardConfig>>,
    /// 保持中の条件付きプラン
    pub plan_book: Arc<RwLock<PlanBook>>,
    /// LLM 推論クライアント
    pub llm: Arc<LlmClient>,
    /// 発注先。既定はペーパー。`NTRADE_LIVE_ORDERS=1` かつ cTrader 接続時に実発注へ切り替わる
    pub order_sink: Arc<RwLock<Arc<dyn OrderSink>>>,
    /// 判断サイクルの直列化
    pub decide_lock: Arc<tokio::sync::Mutex<()>>,
    /// ブローカー残高の最終同期時刻
    pub last_broker_sync: Arc<RwLock<Option<std::time::Instant>>>,
}

/// `NTRADE_LIVE_ORDERS=1` なら実発注
pub fn live_orders_enabled() -> bool {
    std::env::var("NTRADE_LIVE_ORDERS").map(|v| v.trim_matches('"') == "1").unwrap_or(false)
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        let (metrics, positions, trades, cot_logs, lessons) = Self::initial_data();

        let initial_config = CTraderConfig::from_env().ok();

        Self {
            bot_state: Arc::new(RwLock::new(BotState::Running)),
            metrics: Arc::new(RwLock::new(metrics)),
            positions: Arc::new(RwLock::new(positions)),
            trades: Arc::new(RwLock::new(trades)),
            cot_logs: Arc::new(RwLock::new(cot_logs)),
            lessons: Arc::new(RwLock::new(lessons)),
            ctrader_service: Arc::new(RwLock::new(None)),
            ctrader_config: Arc::new(RwLock::new(initial_config)),
            available_accounts: Arc::new(RwLock::new(Vec::new())),
            latest_snapshot: Arc::new(RwLock::new(None)),
            db_path: PathBuf::from(crate::storage::DEFAULT_DB_PATH),
            guard_config: Arc::new(RwLock::new(GuardConfig::load_or_default(DEFAULT_GUARD_CONFIG_PATH))),
            plan_book: Arc::new(RwLock::new(PlanBook::default())),
            llm: Arc::new(LlmClient::new(LlmClientConfig::default())),
            order_sink: Arc::new(RwLock::new(Arc::new(PaperOrderSink))),
            decide_lock: Arc::new(tokio::sync::Mutex::new(())),
            last_broker_sync: Arc::new(RwLock::new(None)),
        }
    }

    /// バックグラウンドでcTraderへの初期接続を試みる
    pub async fn init_ctrader_connection(&self) {
        let cfg_opt = {
            let lock = self.ctrader_config.read().await;
            lock.clone()
        };

        if let Some(cfg) = cfg_opt {
            info!("Attempting background connection to cTrader Open API...");
            match CTraderService::connect(cfg.clone()).await {
                Ok(service) => {
                    info!("Successfully connected and authenticated with cTrader!");
                    let service_arc = Arc::new(service);
                    {
                        let mut svc_lock = self.ctrader_service.write().await;
                        *svc_lock = Some(service_arc.clone());
                    }
                    if live_orders_enabled() {
                        warn!("NTRADE_LIVE_ORDERS=1: orders will be sent to cTrader ({})", if cfg.is_live { "LIVE" } else { "DEMO" });
                        *self.order_sink.write().await = Arc::new(CTraderOrderSink::new(service_arc));
                    } else {
                        info!("Paper order mode (set NTRADE_LIVE_ORDERS=1 to send real orders)");
                    }
                    self.sync_broker_account().await;
                    {
                        let mut m_lock = self.metrics.write().await;
                        m_lock.connection_status.ctrader = "connected".to_string();
                        m_lock.connection_status.environment =
                            if cfg.is_live { "LIVE".to_string() } else { "DEMO".to_string() };
                        m_lock.connection_status.account_number =
                            format!("cTrader #{} ({})", cfg.account_id, if cfg.is_live { "Live" } else { "Demo" });
                    }
                }
                Err(e) => {
                    warn!("Initial cTrader connection failed: {:?}. System is in offline/simulation mode.", e);
                    let mut m_lock = self.metrics.write().await;
                    m_lock.connection_status.ctrader = "disconnected".to_string();
                }
            }
        } else {
            info!("No cTrader credentials configured in .env. Waiting for in-app OAuth linking.");
            let mut m_lock = self.metrics.write().await;
            m_lock.connection_status.ctrader = "disconnected".to_string();
        }
    }

    /// cTrader から口座残高を取得して metrics に反映する。
    /// ペーパーモードでは `broker_balance` に参考表示するだけで、`balance`（ペーパー残高）は変えない。
    /// 実発注モードでは `balance` もブローカー残高に揃える。
    pub async fn sync_broker_account(&self) {
        let Some(ctrader) = self.ctrader_service.read().await.clone() else { return };
        match ctrader.get_account_info().await {
            Ok(acc) => {
                let mut m = self.metrics.write().await;
                m.broker_balance = Some(acc.balance);
                if live_orders_enabled() {
                    m.balance = acc.balance;
                    m.equity = m.balance + m.unrealized_pnl;
                    m.free_margin = m.equity - m.margin;
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

    /// 起動時の初期状態。ダミーデータは持たず、ペーパー口座の初期残高のみ設定する
    /// （`NTRADE_PAPER_BALANCE`、既定 1,000,000）。
    fn initial_data() -> (AccountMetrics, Vec<Position>, Vec<TradeHistory>, Vec<CoTLog>, Vec<LessonLearned>) {
        let balance = std::env::var("NTRADE_PAPER_BALANCE")
            .ok()
            .and_then(|v| v.trim_matches('"').parse::<f64>().ok())
            .unwrap_or(1_000_000.0);

        let metrics = AccountMetrics {
            bot_state: BotState::Running,
            balance,
            equity: balance,
            margin: 0.0,
            free_margin: balance,
            daily_pnl: 0.0,
            daily_pnl_percent: 0.0,
            unrealized_pnl: 0.0,
            win_rate_today: 0.0,
            total_trades_today: 0,
            winning_trades_today: 0,
            usdjpy_spread: 0.0,
            eurusd_spread: 0.0,
            circuit_breaker_threshold_percent: -3.0,
            order_mode: if live_orders_enabled() { "live".to_string() } else { "paper".to_string() },
            broker_balance: None,
            connection_status: ConnectionStatus {
                ctrader: "connecting".to_string(),
                llm: "ready".to_string(),
                ping_ms: 0,
                environment: "DEMO".to_string(),
                account_number: "未連携".to_string(),
            },
        };

        (metrics, Vec::new(), Vec::new(), Vec::new(), Vec::new())
    }
}
