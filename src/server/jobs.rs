//! 時間のかかるバックテスト系の処理（過去データの取得・グリッドサーチ・ウォークフォワード）を
//! エンジンの中で 1 件ずつ順番に走らせるジョブ。管理画面は一覧をポーリングして進み具合を出す。
//!
//! ジョブの記録はメモリだけに持つ（再起動で消える。結果は `backtest_runs` と `bars_history` に残る）。

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use tracing::{info, warn};

use super::state::AppState;
use crate::backtest::config::{self, BacktestFileConfig, GridRequest, WalkForwardWindows, DEFAULT_BACKTEST_CONFIG_PATH};
use crate::backtest::fetch::{self, FetchRequest};
use crate::backtest::optimize::Progress;
use crate::storage::Db;

/// 残しておく終わったジョブの数
const KEEP_FINISHED: usize = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    Fetch,
    Sweep,
    WalkForward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl JobStatus {
    fn finished(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
}

/// ジョブの依頼（POST /api/backtest/jobs）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum JobRequest {
    Fetch(FetchRequest),
    Sweep(GridRequest),
    #[serde(rename_all = "camelCase")]
    WalkForward {
        #[serde(flatten)]
        grid: GridRequest,
        train_days: i64,
        test_days: i64,
    },
}

impl JobRequest {
    fn kind(&self) -> JobKind {
        match self {
            Self::Fetch(_) => JobKind::Fetch,
            Self::Sweep(_) => JobKind::Sweep,
            Self::WalkForward { .. } => JobKind::WalkForward,
        }
    }

    /// 一覧に出す説明
    fn title(&self) -> String {
        let range = |from: chrono::DateTime<Utc>, to: chrono::DateTime<Utc>| format!("{} 〜 {}", from.format("%Y-%m-%d"), to.format("%Y-%m-%d"));
        match self {
            Self::Fetch(r) => format!("{} の足を取得（{} 〜）", r.pairs.join(", "), r.from.format("%Y-%m-%d")),
            Self::Sweep(g) => format!("グリッドサーチ {} {} {}", g.run.pair, g.run.strategy.name, range(g.run.from, g.run.to)),
            Self::WalkForward { grid, train_days, test_days } => format!(
                "ウォークフォワード {} {} {}（学習 {train_days} 日 / 検証 {test_days} 日）",
                grid.run.pair,
                grid.run.strategy.name,
                range(grid.run.from, grid.run.to)
            ),
        }
    }
}

/// 画面に出すジョブの状態
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobInfo {
    pub id: u64,
    pub kind: JobKind,
    pub title: String,
    pub status: JobStatus,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    /// 進み具合（取得なら区切りの数、探索ならバックテストの回数）
    pub done: usize,
    pub total: usize,
    /// 直近の進み具合の説明（取得中の区切りなど）
    pub message: Option<String>,
    /// 保存したバックテスト結果の ID（グリッドサーチ・ウォークフォワード）
    pub run_id: Option<i64>,
    pub error: Option<String>,
}

struct Entry {
    info: JobInfo,
    progress: Arc<Progress>,
}

/// ジョブの一覧と、1 件ずつ走らせるための順番待ち
#[derive(Clone, Default)]
pub struct Jobs {
    entries: Arc<Mutex<Vec<Entry>>>,
    runner: Arc<tokio::sync::Mutex<()>>,
}

fn now() -> String {
    Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

impl Jobs {
    fn with<T>(&self, f: impl FnOnce(&mut Vec<Entry>) -> T) -> T {
        f(&mut self.entries.lock().unwrap_or_else(|e| e.into_inner()))
    }

    fn update(&self, id: u64, f: impl FnOnce(&mut JobInfo)) {
        self.with(|entries| {
            if let Some(e) = entries.iter_mut().find(|e| e.info.id == id) {
                f(&mut e.info);
            }
        });
    }

    /// 新しい順
    pub fn list(&self) -> Vec<JobInfo> {
        self.with(|entries| {
            entries
                .iter()
                .rev()
                .map(|e| JobInfo {
                    done: e.progress.done.load(Ordering::Relaxed),
                    total: e.progress.total.load(Ordering::Relaxed),
                    ..e.info.clone()
                })
                .collect()
        })
    }

    /// 中止を指示する。待っているジョブは始まる前に止まり、走っているジョブは区切りのいいところで止まる
    pub fn cancel(&self, id: u64) -> bool {
        self.with(|entries| match entries.iter().find(|e| e.info.id == id && !e.info.status.finished()) {
            Some(e) => {
                e.progress.cancelled.store(true, Ordering::Relaxed);
                true
            }
            None => false,
        })
    }

    /// ジョブを受け付けて順番待ちに入れる
    pub fn submit(&self, state: AppState, req: JobRequest) -> JobInfo {
        let progress = Arc::new(Progress::default());
        let info = self.with(|entries| {
            // 終わったジョブは新しいものだけ残す
            let finished = entries.iter().filter(|e| e.info.status.finished()).count();
            let mut drop = finished.saturating_sub(KEEP_FINISHED - 1);
            entries.retain(|e| {
                if drop > 0 && e.info.status.finished() {
                    drop -= 1;
                    false
                } else {
                    true
                }
            });
            let id = entries.iter().map(|e| e.info.id).max().unwrap_or(0) + 1;
            let info = JobInfo {
                id,
                kind: req.kind(),
                title: req.title(),
                status: JobStatus::Queued,
                created_at: now(),
                started_at: None,
                finished_at: None,
                done: 0,
                total: 0,
                message: None,
                run_id: None,
                error: None,
            };
            entries.push(Entry { info: info.clone(), progress: progress.clone() });
            info
        });
        let jobs = self.clone();
        let id = info.id;
        tokio::spawn(async move {
            let _turn = jobs.runner.lock().await;
            if progress.is_cancelled() {
                jobs.update(id, |j| {
                    j.status = JobStatus::Cancelled;
                    j.finished_at = Some(now());
                });
                return;
            }
            jobs.update(id, |j| {
                j.status = JobStatus::Running;
                j.started_at = Some(now());
            });
            info!(job = id, kind = ?req.kind(), "backtest job started");
            let result = execute(&state, &jobs, id, &req, progress.clone()).await;
            jobs.update(id, |j| {
                j.finished_at = Some(now());
                match result {
                    Ok(run_id) => {
                        j.status = JobStatus::Succeeded;
                        j.run_id = run_id;
                    }
                    Err(_) if progress.is_cancelled() => j.status = JobStatus::Cancelled,
                    Err(e) => {
                        warn!(job = id, "backtest job failed: {e:#}");
                        j.status = JobStatus::Failed;
                        j.error = Some(format!("{e:#}"));
                    }
                }
            });
        });
        info
    }
}

/// グリッドサーチに使うスレッド数（エンジンの他の処理のために 1 つ空ける）
fn threads() -> usize {
    std::thread::available_parallelism().map(|n| n.get().saturating_sub(1)).unwrap_or(2).max(1)
}

async fn execute(state: &AppState, jobs: &Jobs, id: u64, req: &JobRequest, progress: Arc<Progress>) -> anyhow::Result<Option<i64>> {
    let db_path = state.db_path.clone();
    match req {
        JobRequest::Fetch(r) => {
            let service = state.ctrader_service.read().await.clone().ok_or_else(|| anyhow::anyhow!("cTrader に接続していません"))?;
            let n = fetch::fetch(&service, &db_path, r, &progress, |seg, n| {
                jobs.update(id, |j| j.message = Some(format!("{}  {n} 本", seg.label())));
            })
            .await?;
            jobs.update(id, |j| j.message = Some(format!("{n} 本を保存しました")));
            Ok(None)
        }
        JobRequest::Sweep(g) => {
            let g = g.clone();
            tokio::task::spawn_blocking(move || {
                let mut db = Db::open(&db_path)?;
                let file = BacktestFileConfig::load_or_default(DEFAULT_BACKTEST_CONFIG_PATH)?;
                let out = config::run_sweep(&mut db, &file, &g, threads(), &progress)?;
                Ok(Some(out.id))
            })
            .await?
        }
        JobRequest::WalkForward { grid, train_days, test_days } => {
            let (g, windows) = (grid.clone(), WalkForwardWindows { train_days: *train_days, test_days: *test_days });
            tokio::task::spawn_blocking(move || {
                let mut db = Db::open(&db_path)?;
                let file = BacktestFileConfig::load_or_default(DEFAULT_BACKTEST_CONFIG_PATH)?;
                let out = config::run_walk_forward(&mut db, &file, &g, windows, threads(), &progress)?;
                Ok(Some(out.id))
            })
            .await?
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_parse_from_the_dashboard_shape() {
        let sweep: JobRequest = serde_json::from_str(
            r#"{"kind":"sweep","pair":"USDJPY_z","from":"2025-01-01T00:00:00Z","to":"2026-01-01T00:00:00Z",
                "strategy":{"name":"ema_cross","timeframe":"M15","params":{"tp_rr":2}},"label":"",
                "grid":["fast=10..30:10","slow=50,100"],"objective":"profit_factor","minTrades":20}"#,
        )
        .unwrap();
        assert_eq!(sweep.kind(), JobKind::Sweep);
        let JobRequest::Sweep(g) = &sweep else { unreachable!() };
        assert_eq!((g.min_trades, g.grid.len()), (20, 2));
        assert!(sweep.title().contains("ema_cross"));

        let wf: JobRequest = serde_json::from_str(
            r#"{"kind":"walk_forward","pair":"USDJPY_z","from":"2025-01-01T00:00:00Z","to":"2026-01-01T00:00:00Z",
                "strategy":{"name":"ema_cross","timeframe":"M15"},"grid":["fast=10,20"],"objective":"net_profit",
                "trainDays":120,"testDays":30}"#,
        )
        .unwrap();
        assert!(matches!(wf, JobRequest::WalkForward { train_days: 120, test_days: 30, .. }));

        let fetch: JobRequest =
            serde_json::from_str(r#"{"kind":"fetch","pairs":["USDJPY_z"],"from":"2023-01-01T00:00:00Z","periods":["M1","H1"]}"#).unwrap();
        assert_eq!(fetch.kind(), JobKind::Fetch);
    }
}
