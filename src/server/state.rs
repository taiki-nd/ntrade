use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::ctrader::{CTraderConfig, CTraderService};
use super::types::{
    AccountInfo, AccountMetrics, BotState, CloseReason, ConnectionStatus, CoTLog, LessonLearned,
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
    pub chart_image_path: Arc<RwLock<PathBuf>>,
}

impl AppState {
    pub fn new() -> Self {
        let (metrics, positions, trades, cot_logs, lessons) = Self::initial_data();

        let initial_config = CTraderConfig::from_env().ok();
        let chart_path = PathBuf::from("charts/mtf_analysis_latest.png");

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
            chart_image_path: Arc::new(RwLock::new(chart_path)),
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
                        *svc_lock = Some(service_arc);
                    }
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

    /// 初期シードデータ
    fn initial_data() -> (AccountMetrics, Vec<Position>, Vec<TradeHistory>, Vec<CoTLog>, Vec<LessonLearned>) {
        let metrics = AccountMetrics {
            balance: 1000000.0,
            equity: 1018500.0,
            margin: 45000.0,
            free_margin: 973500.0,
            daily_pnl: 18500.0,
            daily_pnl_percent: 1.85,
            unrealized_pnl: 6400.0,
            win_rate_today: 75.0,
            total_trades_today: 4,
            winning_trades_today: 3,
            usdjpy_spread: 0.2,
            eurusd_spread: 0.3,
            circuit_breaker_threshold_percent: -3.0,
            connection_status: ConnectionStatus {
                ctrader: "connecting".to_string(),
                llm: "ready".to_string(),
                ping_ms: 18,
                environment: "DEMO".to_string(),
                account_number: "cTrader #8921045 (Axiory Demo)".to_string(),
            },
        };

        let positions = vec![Position {
            id: "pos-101".to_string(),
            symbol: "USDJPY".to_string(),
            side: "BUY".to_string(),
            volume_lots: 0.35,
            entry_price: 154.205,
            current_price: 154.388,
            stop_loss: 154.08,
            take_profit: 154.55,
            pnl_pips: 18.3,
            pnl_amount: 6405.0,
            open_time: "2026-09-14 07:35:12".to_string(),
            invalidation_reason: "5M支持帯(154.12)での下ヒゲ68%ピンバー安値(154.08)割れで無効化".to_string(),
        }];

        let trades = vec![
            TradeHistory {
                id: "trd-304".to_string(),
                symbol: "USDJPY".to_string(),
                side: "BUY".to_string(),
                volume_lots: 0.3,
                entry_price: 153.85,
                close_price: 154.15,
                stop_loss: 153.72,
                take_profit: 154.15,
                pnl_pips: 30.0,
                pnl_amount: 9000.0,
                close_reason: CloseReason::TakeProfit,
                open_time: "2026-09-14 05:10:00".to_string(),
                close_time: "2026-09-14 06:45:20".to_string(),
                cot_log_id: Some("cot-201".to_string()),
            },
            TradeHistory {
                id: "trd-303".to_string(),
                symbol: "EURUSD".to_string(),
                side: "SELL".to_string(),
                volume_lots: 0.25,
                entry_price: 1.0845,
                close_price: 1.0825,
                stop_loss: 1.0860,
                take_profit: 1.0825,
                pnl_pips: 20.0,
                pnl_amount: 7500.0,
                close_reason: CloseReason::TakeProfit,
                open_time: "2026-09-14 03:20:00".to_string(),
                close_time: "2026-09-14 04:55:10".to_string(),
                cot_log_id: Some("cot-200".to_string()),
            },
            TradeHistory {
                id: "trd-302".to_string(),
                symbol: "USDJPY".to_string(),
                side: "SELL".to_string(),
                volume_lots: 0.2,
                entry_price: 154.05,
                close_price: 154.20,
                stop_loss: 154.20,
                take_profit: 153.70,
                pnl_pips: -15.0,
                pnl_amount: -3000.0,
                close_reason: CloseReason::StopLoss,
                open_time: "2026-09-14 01:05:00".to_string(),
                close_time: "2026-09-14 02:15:30".to_string(),
                cot_log_id: None,
            },
        ];

        let cot_logs = vec![
            CoTLog {
                id: "cot-202".to_string(),
                timestamp: "2026-09-14 07:35:05".to_string(),
                symbol: "USDJPY".to_string(),
                action: "BUY".to_string(),
                confidence: 0.88,
                entry_type: Some("MARKET".to_string()),
                entry_price: Some(154.205),
                stop_loss: Some(154.080),
                take_profit: Some(154.550),
                risk_reward_ratio: Some(2.76),
                macro_bias: "4H EMA20上方推移の強気パーフェクトオーダー。押し目買い優勢。".to_string(),
                trigger_pattern: "154.12支持帯における下ヒゲ68%強気レジェクション・ピンバー(M5)".to_string(),
                invalidation_point: "154.080(ピンバー安値下抜けかつ直近スイング安値割れ)".to_string(),
                reasoning: "4H/1Hが共に明確な上昇トレンド。15Mにて154.10ラインへのプルバックが完了し、5M足で反発を示す教科書的な強気ピンバーが確定した。スプレッド0.2pipsと狭小でRR比2.76を確保できるためロングエントリーを実行。".to_string(),
                executed: true,
                spread_pips: 0.2,
            },
            CoTLog {
                id: "cot-201".to_string(),
                timestamp: "2026-09-14 05:09:55".to_string(),
                symbol: "USDJPY".to_string(),
                action: "BUY".to_string(),
                confidence: 0.82,
                entry_type: Some("MARKET".to_string()),
                entry_price: Some(153.850),
                stop_loss: Some(153.720),
                take_profit: Some(154.150),
                risk_reward_ratio: Some(2.31),
                macro_bias: "1Hチャネル下限サポートからのリバウンド局面。".to_string(),
                trigger_pattern: "5M強気包み足（ブル・エンガルフィング）".to_string(),
                invalidation_point: "153.720(直近安値割れ)".to_string(),
                reasoning: "直近の急落後に153.80ラインで下げ止まりを確認。大陰線を丸ごと包む強気包み足が確定。リスクリワード2.31を確保できるため買い判断。".to_string(),
                executed: true,
                spread_pips: 0.2,
            },
        ];

        let lessons = vec![
            LessonLearned {
                id: "les-001".to_string(),
                created_at: "2026-09-13 18:20:00".to_string(),
                symbol: "USDJPY".to_string(),
                rule: "4H上位足が上昇トレンドの局面では、5M逆張りショートは厳禁。".to_string(),
                context: "強い上昇相場における短期レジスタンスでのピンバーに飛びつきSL到達したトレードからの反省。".to_string(),
                active: true,
                trigger_trade_id: Some("trd-302".to_string()),
                category: "RISK".to_string(),
            },
            LessonLearned {
                id: "les-002".to_string(),
                created_at: "2026-09-14 02:45:10".to_string(),
                symbol: "ALL".to_string(),
                rule: "欧州ロンドンオープン直後（日本時間16:00-16:30）の初動ブレイクはダマシが多いため、プルバック確定を待つこと。".to_string(),
                context: "ロンドン開場直後のヒゲ狩り（Liquidity Hunt）に巻き込まれた教訓。".to_string(),
                active: true,
                trigger_trade_id: None,
                category: "TIMING".to_string(),
            },
        ];

        (metrics, positions, trades, cot_logs, lessons)
    }
}
