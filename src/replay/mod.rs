//! リプレイ環境: 過去の任意時刻で Snapshot を再生し、LLM 判断を採点・集計する。
//!
//! 本番と同じ `snapshot` / `llm` を使う。違いは「時刻 t を明示する」「バーを DB から読む」の2点だけ。

pub mod report;
pub mod score;

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Duration, TimeZone, Utc};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tracing::{info, warn};

use crate::ctrader::{BarPeriod, CandleBar};
use crate::guard::{self, GuardConfig, GuardContext};
use crate::llm::LlmBackend;
use crate::snapshot::measures::get_pip_size;
use crate::snapshot::{AccountState, MarketSnapshot, SnapshotBundle, SnapshotInput, SnapshotPipeline};
use crate::storage::{Db, ReplayDecisionRow};
use crate::strategy::PromptBuilder;
use score::{score_decision, ScoreConfig};

pub const SNAPSHOT_VERSION: &str = "snapshot-v2";

/// バー供給元の抽象。本番は cTrader、リプレイは SQLite。
pub trait BarSource: Send + Sync {
    /// `t` 時点で確定している直近 `count` 本を昇順で返す
    fn bars_until<'a>(
        &'a self,
        pair: &'a str,
        period: BarPeriod,
        t: DateTime<Utc>,
        count: usize,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<CandleBar>>> + Send + 'a>>;
}

/// SQLite (`bars_history`) からのバー供給
pub struct SqliteBarSource {
    db: Arc<Mutex<Db>>,
}

impl SqliteBarSource {
    pub fn new(db: Arc<Mutex<Db>>) -> Self {
        Self { db }
    }
}

impl BarSource for SqliteBarSource {
    fn bars_until<'a>(
        &'a self,
        pair: &'a str,
        period: BarPeriod,
        t: DateTime<Utc>,
        count: usize,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<CandleBar>>> + Send + 'a>> {
        Box::pin(async move {
            let db = self.db.lock().map_err(|_| anyhow!("db lock poisoned"))?;
            db.bars_until(pair, period, t, count)
        })
    }
}

/// 時刻 `t` の Snapshot を BarSource 経由で生成する（本番・リプレイ共通の入口）
pub async fn build_snapshot_at(
    source: &dyn BarSource,
    pipeline: &SnapshotPipeline,
    pair: &str,
    t: DateTime<Utc>,
    spread_pips: f64,
    bars_per_tf: usize,
    account_state: AccountState,
) -> Result<SnapshotBundle> {
    let b4h = source.bars_until(pair, BarPeriod::H4, t, bars_per_tf).await?;
    let b1h = source.bars_until(pair, BarPeriod::H1, t, bars_per_tf).await?;
    let b15 = source.bars_until(pair, BarPeriod::M15, t, bars_per_tf).await?;
    let b5 = source.bars_until(pair, BarPeriod::M5, t, bars_per_tf).await?;
    if b5.is_empty() {
        return Err(anyhow!("no 5M bars available at {t}"));
    }
    let bundle = pipeline.build(&SnapshotInput {
        pair,
        bars_4h: &b4h,
        bars_1h: &b1h,
        bars_15m: &b15,
        bars_5m: &b5,
        spread_pips,
        current_price: b5.last().map(|b| b.close),
        now: Some(t),
        account_state,
    })?;
    verify_no_future_leak(&bundle.snapshot, t)?;
    Ok(bundle)
}

/// Snapshot に `t` より後の時刻が含まれていないことを検証する
pub fn verify_no_future_leak(snapshot: &MarketSnapshot, t: DateTime<Utc>) -> Result<()> {
    let mut times: Vec<(&str, &str)> = Vec::new();
    for b in &snapshot.bars_5m {
        times.push(("bars_5m", &b.t));
    }
    for b in &snapshot.bars_15m {
        times.push(("bars_15m", &b.t));
    }
    for p in snapshot.swing_points.h4.iter().chain(&snapshot.swing_points.h1).chain(&snapshot.swing_points.m15) {
        times.push(("swing_points", &p.time));
    }
    for (label, v) in [
        ("latest_bars.4h", &snapshot.latest_bars.h4),
        ("latest_bars.1h", &snapshot.latest_bars.h1),
        ("latest_bars.15m", &snapshot.latest_bars.m15),
        ("latest_bars.5m", &snapshot.latest_bars.m5),
    ] {
        if let Some(s) = v {
            times.push((label, s));
        }
    }
    for (label, s) in times {
        let secs = crate::storage::parse_ts(s)?;
        let ts = Utc.timestamp_opt(secs, 0).single().ok_or_else(|| anyhow!("bad ts {s}"))?;
        if ts > t {
            return Err(anyhow!("future leak: {label} has {s} > t={t}"));
        }
    }
    // 各時間足の最新足は t 時点で確定していなければならない
    for (label, v, period) in [
        ("4h", &snapshot.latest_bars.h4, BarPeriod::H4),
        ("1h", &snapshot.latest_bars.h1, BarPeriod::H1),
        ("15m", &snapshot.latest_bars.m15, BarPeriod::M15),
        ("5m", &snapshot.latest_bars.m5, BarPeriod::M5),
    ] {
        if let Some(s) = v {
            let start = Utc.timestamp_opt(crate::storage::parse_ts(s)?, 0).single().unwrap();
            if start + period.duration() > t {
                return Err(anyhow!("future leak: latest {label} bar {s} is not closed at t={t}"));
            }
        }
    }
    Ok(())
}

/// 評価時刻の列を作る
pub fn sample_times(from: DateTime<Utc>, to: DateTime<Utc>, step_minutes: i64, limit: Option<usize>) -> Vec<DateTime<Utc>> {
    let step = Duration::minutes(step_minutes.max(5));
    // step の倍数に切り上げて整列
    let step_secs = step.num_seconds();
    let first_secs = ((from.timestamp() + step_secs - 1) / step_secs) * step_secs;
    let mut t = Utc.timestamp_opt(first_secs, 0).single().unwrap_or(from);
    let mut all = Vec::new();
    while t <= to {
        all.push(t);
        t += step;
    }
    match limit {
        Some(n) if n > 0 && n < all.len() => {
            // 均等間引き
            let stride = all.len() as f64 / n as f64;
            (0..n).map(|i| all[(i as f64 * stride) as usize]).collect()
        }
        _ => all,
    }
}

/// リプレイ実行の設定
#[derive(Debug, Clone)]
pub struct ReplayConfig {
    pub pair: String,
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub step_minutes: i64,
    pub limit: Option<usize>,
    pub label: String,
    pub parallel: usize,
    pub bars_per_tf: usize,
    pub spread_pips: f64,
    pub max_bars_to_exit: usize,
    pub chart_root: PathBuf,
    pub lessons: Vec<String>,
}

pub struct ReplayRunner {
    db: Arc<Mutex<Db>>,
    llm: Arc<dyn LlmBackend>,
    cfg: ReplayConfig,
    guard: GuardConfig,
}

impl ReplayRunner {
    pub fn new(db: Arc<Mutex<Db>>, llm: Arc<dyn LlmBackend>, cfg: ReplayConfig, guard: GuardConfig) -> Self {
        Self { db, llm, cfg, guard }
    }

    /// 新規 run を作成（または `resume_run` を続行）して全サンプルを評価する。run_id を返す。
    pub async fn run(&self, resume_run: Option<i64>) -> Result<i64> {
        let cfg = &self.cfg;
        let prompt_hash = prompt_fingerprint(&cfg.lessons);
        let guard_config = serde_json::json!({
            "guard": self.guard,
            "max_bars_to_exit": cfg.max_bars_to_exit,
            "spread_pips": cfg.spread_pips
        })
        .to_string();
        let sampling = format!("step:{}m{}", cfg.step_minutes, cfg.limit.map(|l| format!(":limit={l}")).unwrap_or_default());

        let (run_id, done) = {
            let db = self.db.lock().map_err(|_| anyhow!("db lock poisoned"))?;
            match resume_run {
                Some(id) => {
                    db.get_run(id)?.ok_or_else(|| anyhow!("run {id} not found"))?;
                    (id, db.decided_ts(id)?)
                }
                None => (
                    db.create_run(&cfg.label, &cfg.pair, cfg.from, cfg.to, &sampling, &prompt_hash, SNAPSHOT_VERSION, &guard_config)?,
                    Vec::new(),
                ),
            }
        };

        let times: Vec<DateTime<Utc>> = sample_times(cfg.from, cfg.to, cfg.step_minutes, cfg.limit)
            .into_iter()
            .filter(|t| !done.contains(t))
            .collect();
        info!(run_id, samples = times.len(), skipped = done.len(), "Replay run starting");

        let source = Arc::new(SqliteBarSource::new(self.db.clone()));
        let pipeline = Arc::new(SnapshotPipeline::new(cfg.chart_root.join("replay").join(run_id.to_string())));
        let sem = Arc::new(tokio::sync::Semaphore::new(cfg.parallel.max(1)));
        let mut handles = Vec::new();

        for t in times {
            let permit = sem.clone().acquire_owned().await?;
            let (db, llm, source, pipeline, cfg, guard) =
                (self.db.clone(), self.llm.clone(), source.clone(), pipeline.clone(), self.cfg.clone(), self.guard.clone());
            handles.push(tokio::spawn(async move {
                let _p = permit;
                let r = evaluate_one(&db, llm.as_ref(), source.as_ref(), &pipeline, &cfg, &guard, run_id, t).await;
                if let Err(e) = &r {
                    warn!(%t, "replay sample failed: {e:#}");
                }
                r
            }));
        }
        let mut ok = 0;
        let mut failed = 0;
        for h in handles {
            match h.await {
                Ok(Ok(())) => ok += 1,
                _ => failed += 1,
            }
        }
        info!(run_id, ok, failed, "Replay run finished");
        Ok(run_id)
    }
}

#[allow(clippy::too_many_arguments)]
async fn evaluate_one(
    db: &Arc<Mutex<Db>>,
    llm: &dyn LlmBackend,
    source: &dyn BarSource,
    pipeline: &SnapshotPipeline,
    cfg: &ReplayConfig,
    guard_cfg: &GuardConfig,
    run_id: i64,
    t: DateTime<Utc>,
) -> Result<()> {
    let bundle = build_snapshot_at(source, pipeline, &cfg.pair, t, cfg.spread_pips, cfg.bars_per_tf, AccountState::default()).await?;
    let prompt = PromptBuilder::new().with_lessons(cfg.lessons.clone()).build(&bundle.snapshot, &bundle.charts);
    let decision = llm.infer(&prompt).await?;
    // リプレイでは口座状態を持たないので、ポジション数・日次損失は常にゼロとして評価する
    let guard = guard::evaluate(&decision, &bundle.snapshot, &GuardContext { now: Some(t), ..Default::default() }, guard_cfg).summary();

    let after = {
        let db = db.lock().map_err(|_| anyhow!("db lock poisoned"))?;
        db.bars_from(&cfg.pair, BarPeriod::M5, t, cfg.max_bars_to_exit + 400)?
    };
    let score = score_decision(
        &after,
        &decision,
        &ScoreConfig { pip_size: get_pip_size(&cfg.pair), spread_pips: cfg.spread_pips, max_bars: cfg.max_bars_to_exit },
    );

    let row = ReplayDecisionRow {
        run_id,
        t: t.format("%Y-%m-%d %H:%M:%S").to_string(),
        decision_json: serde_json::to_string(&decision)?,
        guard_result: guard,
        outcome: score.outcome.as_str().to_string(),
        pnl_pips: score.pnl_pips,
        bars_to_exit: score.bars_to_exit.map(|n| n as i64),
        chart_dir: bundle.charts.dir.to_string_lossy().to_string(),
        session: serde_json::to_value(bundle.snapshot.session)?.as_str().unwrap_or("").to_string(),
        confidence: decision.confidence,
        action: serde_json::to_value(decision.action)?.as_str().unwrap_or("").to_string(),
    };
    let db = db.lock().map_err(|_| anyhow!("db lock poisoned"))?;
    db.insert_decision(&row)?;
    info!(%t, action = %row.action, outcome = %row.outcome, guard = %row.guard_result, "replay sample done");
    Ok(())
}

/// プロンプト本文（教訓含む）のフィンガープリント。変更検知用。
pub fn prompt_fingerprint(lessons: &[String]) -> String {
    use std::hash::{Hash, Hasher};
    let dummy_snapshot = MarketSnapshot::build(
        &SnapshotInput {
            pair: "USDJPY",
            bars_4h: &[],
            bars_1h: &[],
            bars_15m: &[],
            bars_5m: &[],
            spread_pips: 0.0,
            current_price: Some(1.0),
            now: Some(Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap()),
            account_state: AccountState::default(),
        },
        &crate::snapshot::SnapshotConfig::default(),
    );
    let charts = crate::snapshot::ChartSet {
        dir: PathBuf::from("x"),
        h4: PathBuf::from("x/4H.png"),
        h1: PathBuf::from("x/1H.png"),
        m15: PathBuf::from("x/15M.png"),
        m5: PathBuf::from("x/5M.png"),
    };
    let text = PromptBuilder::new().with_lessons(lessons.to_vec()).build(&dummy_snapshot, &charts);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// `check-leak`: DB 内のランダムな時刻で Snapshot を作り、未来漏れが無いことを確認する
pub async fn check_leak(db: Arc<Mutex<Db>>, pair: &str, samples: usize, chart_root: &Path) -> Result<usize> {
    let (first, last) = {
        let d = db.lock().map_err(|_| anyhow!("db lock poisoned"))?;
        let cov = d.coverage()?;
        let c = cov
            .iter()
            .find(|c| c.pair == pair.to_uppercase() && c.period == "5M")
            .ok_or_else(|| anyhow!("no 5M bars for {pair} in DB"))?;
        (
            Utc.timestamp_opt(crate::storage::parse_ts(c.first.as_deref().unwrap_or(""))?, 0).single().unwrap(),
            Utc.timestamp_opt(crate::storage::parse_ts(c.last.as_deref().unwrap_or(""))?, 0).single().unwrap(),
        )
    };
    let source = SqliteBarSource::new(db);
    let pipeline = SnapshotPipeline::new(chart_root.join("leakcheck"));
    // 序盤は上位足が足りないので、範囲の 20% 以降から取る
    let span = (last - first).num_seconds();
    let start = first + Duration::seconds(span / 5);
    let times = sample_times(start, last, 5, Some(samples));
    let mut checked = 0;
    for t in times {
        build_snapshot_at(&source, &pipeline, pair, t, 0.0, 60, AccountState::default())
            .await
            .with_context(|| format!("leak check failed at {t}"))?;
        checked += 1;
    }
    let _ = std::fs::remove_dir_all(chart_root.join("leakcheck"));
    Ok(checked)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::SnapshotConfig;

    fn make(count: usize, step_min: i64, end: DateTime<Utc>) -> Vec<CandleBar> {
        (0..count)
            .map(|i| {
                let p = 154.0 + ((i as f64) * 0.3).sin() * 0.2;
                CandleBar {
                    timestamp: end - Duration::minutes(step_min * (count as i64 - 1 - i as i64)),
                    open: p,
                    high: p + 0.1,
                    low: p - 0.1,
                    close: p + 0.03,
                    volume: 1,
                }
            })
            .collect()
    }

    #[test]
    fn sample_times_align_and_limit() {
        let from = Utc.with_ymd_and_hms(2026, 9, 1, 0, 7, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2026, 9, 1, 2, 0, 0).unwrap();
        let all = sample_times(from, to, 15, None);
        assert_eq!(all[0], Utc.with_ymd_and_hms(2026, 9, 1, 0, 15, 0).unwrap());
        assert_eq!(all.len(), 8);
        let few = sample_times(from, to, 15, Some(4));
        assert_eq!(few.len(), 4);
    }

    #[test]
    fn leak_detector_catches_future_bars() {
        let t = Utc.with_ymd_and_hms(2026, 9, 15, 9, 0, 0).unwrap();
        let ok_bars = make(30, 5, t - Duration::minutes(5)); // 最新足 08:55 → 09:00 確定
        let snap = MarketSnapshot::build(
            &SnapshotInput {
                pair: "USDJPY",
                bars_4h: &[],
                bars_1h: &[],
                bars_15m: &[],
                bars_5m: &ok_bars,
                spread_pips: 0.0,
                current_price: None,
                now: Some(t),
                account_state: AccountState::default(),
            },
            &SnapshotConfig::default(),
        );
        assert!(verify_no_future_leak(&snap, t).is_ok());

        let leaky = make(30, 5, t); // 最新足 09:00 は 09:05 確定 → 未確定
        let snap = MarketSnapshot::build(
            &SnapshotInput {
                pair: "USDJPY",
                bars_4h: &[],
                bars_1h: &[],
                bars_15m: &[],
                bars_5m: &leaky,
                spread_pips: 0.0,
                current_price: None,
                now: Some(t),
                account_state: AccountState::default(),
            },
            &SnapshotConfig::default(),
        );
        assert!(verify_no_future_leak(&snap, t).is_err());
    }

    #[tokio::test]
    async fn sqlite_source_feeds_snapshot_without_leak() {
        let mut db = Db::open_in_memory().unwrap();
        let end = Utc.with_ymd_and_hms(2026, 9, 15, 12, 0, 0).unwrap();
        db.insert_bars("USDJPY", BarPeriod::H4, &make(80, 240, end)).unwrap();
        db.insert_bars("USDJPY", BarPeriod::H1, &make(80, 60, end)).unwrap();
        db.insert_bars("USDJPY", BarPeriod::M15, &make(80, 15, end)).unwrap();
        db.insert_bars("USDJPY", BarPeriod::M5, &make(80, 5, end)).unwrap();
        let db = Arc::new(Mutex::new(db));
        let source = SqliteBarSource::new(db);
        let root = std::env::temp_dir().join(format!("ntrade_replay_{}", std::process::id()));
        let pipeline = SnapshotPipeline::new(&root);

        let t = end - Duration::minutes(30);
        let bundle = build_snapshot_at(&source, &pipeline, "USDJPY", t, 0.2, 60, AccountState::default()).await.unwrap();
        assert_eq!(bundle.snapshot.latest_bars.m5.as_deref(), Some("2026-09-15 11:25"));
        assert_eq!(bundle.snapshot.timestamp, "2026-09-15 11:30:00 UTC");
        let _ = std::fs::remove_dir_all(root);
    }

}
