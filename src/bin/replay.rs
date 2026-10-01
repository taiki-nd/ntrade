//! リプレイ環境 CLI
//!
//! ```text
//! cargo run --bin replay -- fetch      --pair USDJPY --from 2026-06-01 --to 2026-09-01
//! cargo run --bin replay -- coverage
//! cargo run --bin replay -- check-leak --pair USDJPY --samples 200
//! cargo run --bin replay -- run        --pair USDJPY --from 2026-08-01 --to 2026-08-31 --step 15 --limit 200 --label "prompt-v3"
//! cargo run --bin replay -- report     --run 1
//! cargo run --bin replay -- diff       --run 1 --run 2
//! cargo run --bin replay -- rescore    --run 1 --exit touch:0.5 --exit close:0:2
//! cargo run --bin replay -- list
//! ```

use anyhow::{anyhow, Result};
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use ntrade::ctrader::{BarPeriod, CTraderConfig, CTraderService};
use ntrade::guard::{GuardConfig, DEFAULT_GUARD_CONFIG_PATH};
use ntrade::llm::{LlmClient, LlmClientConfig};
use ntrade::replay::report::{build_report, format_compare, format_diff, format_report};
use ntrade::replay::score::ExitRule;
use ntrade::replay::{check_leak, rescore_run, ReplayConfig, ReplayRunner};
use ntrade::storage::{Db, DEFAULT_DB_PATH};

#[derive(Parser)]
#[command(name = "replay", about = "ntrade リプレイ環境")]
struct Cli {
    /// SQLite ファイル
    #[arg(long, default_value = DEFAULT_DB_PATH)]
    db: PathBuf,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// cTrader からヒストリカルバーを取得して保存（差分取得対応）
    Fetch {
        #[arg(long, default_value = "USDJPY")]
        pair: String,
        /// YYYY-MM-DD
        #[arg(long)]
        from: String,
        /// YYYY-MM-DD（省略時は現在）
        #[arg(long)]
        to: Option<String>,
        /// 取得する時間足（カンマ区切り）
        #[arg(long, default_value = "5M,15M,1H,4H")]
        periods: String,
    },
    /// DB の収録範囲を表示
    Coverage,
    /// ランダムな時刻で Snapshot を生成し、未来漏れが無いことを検証
    CheckLeak {
        #[arg(long, default_value = "USDJPY")]
        pair: String,
        #[arg(long, default_value_t = 50)]
        samples: usize,
    },
    /// リプレイ実行（Snapshot → LLM → ガード → 採点 → 保存）
    Run {
        #[arg(long, default_value = "USDJPY")]
        pair: String,
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        /// サンプリング間隔（分）
        #[arg(long, default_value_t = 15)]
        step: i64,
        /// 最大サンプル数（均等間引き）
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long, default_value = "")]
        label: String,
        /// claude -p の同時実行数
        #[arg(long, default_value_t = 2)]
        parallel: usize,
        /// 採点で TIMEOUT にするまでの5M本数（48 = 4時間）
        #[arg(long, default_value_t = 48)]
        max_bars: usize,
        /// 想定スプレッド（pips）
        #[arg(long, default_value_t = 0.3)]
        spread: f64,
        /// 未評価サンプルだけを続行する run_id
        #[arg(long)]
        resume: Option<i64>,
        /// 教訓（複数指定可）
        #[arg(long)]
        lesson: Vec<String>,
        /// LLM モデル名（省略時は CLI デフォルト）
        #[arg(long)]
        model: Option<String>,
        /// 決済ルール（touch / touch:<SLバッファATR倍> / close:<SLバッファ>:<ハードSL>）
        #[arg(long, default_value = "touch", value_parser = parse_exit)]
        exit: ExitRule,
        /// 条件付きプランを成立時の終値で機械的に執行したとみなして採点する（再判断を入れる前の旧方式）。
        /// 省略時は本番と同じく、成立した足で LLM に判断し直させる
        #[arg(long)]
        mechanical_plans: bool,
    },
    /// 保存済み run の判断を、LLM を呼ばずに別の決済ルールで採点し直す（ルールごとに新しい run を作る）
    Rescore {
        #[arg(long)]
        run: i64,
        /// 決済ルール（複数指定可）。例: --exit touch --exit touch:0.5 --exit close:0:2
        #[arg(long = "exit", required = true, value_parser = parse_exit)]
        exits: Vec<ExitRule>,
    },
    /// run の集計を表示
    Report {
        #[arg(long)]
        run: i64,
        #[arg(long)]
        json: bool,
    },
    /// 2つの run を比較
    Diff {
        #[arg(long, num_args = 2)]
        run: Vec<i64>,
    },
    /// run 一覧
    List,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "ntrade=info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();
    let db = Arc::new(Mutex::new(Db::open(&cli.db)?));

    match cli.cmd {
        Cmd::Fetch { pair, from, to, periods } => {
            let from = parse_date(&from)?;
            let to = to.map(|s| parse_date(&s)).transpose()?.unwrap_or_else(Utc::now);
            let service = CTraderService::connect(CTraderConfig::from_env()?).await?;
            for p in periods.split(',') {
                let period = BarPeriod::parse(p.trim()).ok_or_else(|| anyhow!("unknown period {p}"))?;
                let start = {
                    let d = db.lock().unwrap();
                    d.last_bar_ts(&pair, period)?.map(|t| t.max(from)).unwrap_or(from)
                };
                println!("fetching {pair} {} from {start} to {to} ...", period.as_str());
                let bars = service.get_trendbars_range(&pair, period, start, to).await?;
                let n = db.lock().unwrap().insert_bars(&pair, period, &bars)?;
                println!("  stored {n} bars");
            }
            print_coverage(&db)?;
        }
        Cmd::Coverage => print_coverage(&db)?,
        Cmd::CheckLeak { pair, samples } => {
            let n = check_leak(db.clone(), &pair, samples, &PathBuf::from("charts")).await?;
            println!("check-leak OK: {n} snapshots generated with no future data");
        }
        Cmd::Run { pair, from, to, step, limit, label, parallel, max_bars, spread, resume, lesson, model, exit, mechanical_plans } => {
            let llm = Arc::new(LlmClient::new(LlmClientConfig { model, ..Default::default() }));
            let cfg = ReplayConfig {
                pair,
                from: parse_date(&from)?,
                to: parse_date(&to)?,
                step_minutes: step,
                limit,
                label,
                parallel,
                bars_per_tf: 200,
                spread_pips: spread,
                max_bars_to_exit: max_bars,
                chart_root: PathBuf::from("charts"),
                lessons: lesson,
                exit,
                plan_rejudge: !mechanical_plans,
            };
            let guard = GuardConfig::load_or_default(DEFAULT_GUARD_CONFIG_PATH);
            let run_id = ReplayRunner::new(db.clone(), llm, cfg, guard).run(resume).await?;
            let rows = db.lock().unwrap().decisions(run_id)?;
            println!("\n{}", format_report(&build_report(run_id, &rows)));
        }
        Cmd::Report { run, json } => {
            let rows = db.lock().unwrap().decisions(run)?;
            let r = build_report(run, &rows);
            if json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                println!("{}", format_report(&r));
            }
        }
        Cmd::Rescore { run, exits } => {
            let source_label = db.lock().unwrap().get_run(run)?.ok_or_else(|| anyhow!("run {run} not found"))?.label;
            let mut reports = vec![build_report(run, &db.lock().unwrap().decisions(run)?)];
            println!("{:<8} {}", format!("#{run}"), "(source)");
            for exit in exits {
                let label = format!("{source_label} [{}]", exit.label()).trim().to_string();
                let id = rescore_run(&db, run, exit, &label)?;
                println!("{:<8} {}", format!("#{id}"), exit.label());
                reports.push(build_report(id, &db.lock().unwrap().decisions(id)?));
            }
            println!("\n{}", format_compare(&reports));
        }
        Cmd::Diff { run } => {
            let d = db.lock().unwrap();
            let a = build_report(run[0], &d.decisions(run[0])?);
            let b = build_report(run[1], &d.decisions(run[1])?);
            println!("{}", format_diff(&a, &b));
        }
        Cmd::List => {
            for r in db.lock().unwrap().list_runs()? {
                println!(
                    "#{:<4} {}  {:<8} {} .. {}  {:<24} decisions={:<5} label={}",
                    r.id, r.created_at, r.pair, r.from_ts, r.to_ts, r.sampling, r.decisions, r.label
                );
            }
        }
    }
    Ok(())
}

fn parse_date(s: &str) -> Result<DateTime<Utc>> {
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(Utc.from_utc_datetime(&d.and_hms_opt(0, 0, 0).unwrap()));
    }
    let secs = ntrade::storage::parse_ts(s)?;
    Utc.timestamp_opt(secs, 0).single().ok_or_else(|| anyhow!("bad date {s}"))
}

fn parse_exit(s: &str) -> Result<ExitRule, String> {
    ExitRule::parse(s).ok_or_else(|| format!("invalid exit rule '{s}' (touch / touch:0.5 / close:0.5:2)"))
}

fn print_coverage(db: &Arc<Mutex<Db>>) -> Result<()> {
    println!("{:<8} {:<6} {:>8}  {:<20} {:<20}", "pair", "period", "bars", "first", "last");
    for c in db.lock().unwrap().coverage()? {
        println!(
            "{:<8} {:<6} {:>8}  {:<20} {:<20}",
            c.pair,
            c.period,
            c.count,
            c.first.unwrap_or_default(),
            c.last.unwrap_or_default()
        );
    }
    Ok(())
}
