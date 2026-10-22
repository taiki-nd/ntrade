//! バックテスト CLI
//!
//! ```text
//! cargo run --release --bin backtest -- fetch --pair USDJPY_z --pair EURUSD_z --from 2023-01-01
//! cargo run --release --bin backtest -- coverage
//! cargo run --release --bin backtest -- strategies
//! cargo run --release --bin backtest -- run   --pair USDJPY_z --from 2024-01-01 --to 2026-10-01 --strategy ema_cross --tf M15 --param fast=20 --param slow=50
//! cargo run --release --bin backtest -- sweep --pair USDJPY_z --from 2024-01-01 --to 2026-10-01 --strategy ema_cross --tf M15 --grid fast=10..30:5 --grid slow=50,100 --objective pf
//! cargo run --release --bin backtest -- walk-forward --pair USDJPY_z --from 2024-01-01 --to 2026-10-01 --strategy ema_cross --tf M15 --grid fast=10..30:5 --train-days 180 --test-days 60
//! cargo run --release --bin backtest -- list
//! cargo run --release --bin backtest -- show --run 1 --trades 20
//! ```

use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use ntrade::backtest::config::{self, BacktestFileConfig, GridRequest, RunRequest, WalkForwardWindows, DEFAULT_BACKTEST_CONFIG_PATH};
use ntrade::backtest::fetch::FetchRequest;
use ntrade::backtest::optimize::{Objective, Progress};
use ntrade::backtest::{self, BacktestResult, BacktestTrade, Metrics};
use ntrade::ctrader::{BarPeriod, CTraderConfig, CTraderService};
use ntrade::storage::{Db, DEFAULT_DB_PATH};
use ntrade::strategy::{self, Params, StrategySpec};

#[derive(Parser)]
#[command(name = "backtest", about = "ntrade バックテスト")]
struct Cli {
    /// SQLite ファイル
    #[arg(long, default_value = DEFAULT_DB_PATH)]
    db: PathBuf,
    /// 設定ファイル（コスト・初期資金・リスク設定の取り方）
    #[arg(long, default_value = DEFAULT_BACKTEST_CONFIG_PATH)]
    config: PathBuf,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Args, Clone)]
struct RunArgs {
    #[arg(long)]
    pair: String,
    /// YYYY-MM-DD（UTC）
    #[arg(long)]
    from: String,
    /// YYYY-MM-DD（UTC、この日の 0:00 まで）。省略時は現在
    #[arg(long)]
    to: Option<String>,
    #[arg(long)]
    strategy: String,
    /// 戦略の時間足（M1 / M5 / M15 / M30 / H1 / H4 / D1）
    #[arg(long, default_value = "M5")]
    tf: String,
    /// 戦略パラメータ（name=value、複数指定可）
    #[arg(long = "param")]
    params: Vec<String>,
    /// 初期資金（設定ファイルの値を上書き）
    #[arg(long)]
    balance: Option<f64>,
    /// スプレッド（pips。設定ファイルの値を上書き）
    #[arg(long)]
    spread: Option<f64>,
    /// 手数料（1 lot・片道。設定ファイルの値を上書き）
    #[arg(long)]
    commission: Option<f64>,
    /// 約定を判定する足（省略時は設定ファイル。auto なら 1 分足があればそれ）
    #[arg(long)]
    exec_tf: Option<String>,
    #[arg(long, default_value = "")]
    label: String,
}

#[derive(Args, Clone)]
struct GridArgs {
    /// 探索するパラメータ（fast=5..30:5 / slow=50,100 / sl_atr=1.5）。複数指定可
    #[arg(long = "grid", required = true)]
    grid: Vec<String>,
    /// 何を最大化するか（net / pf / sharpe / recovery / expectancy）
    #[arg(long, default_value = "pf")]
    objective: String,
    /// これ未満の取引数の組み合わせは選ばない
    #[arg(long, default_value_t = 30)]
    min_trades: usize,
    /// 並列数（省略時は CPU 数）
    #[arg(long)]
    threads: Option<usize>,
}

#[derive(Subcommand)]
enum Cmd {
    /// cTrader からヒストリカルバーを取得して保存（DB に無い期間だけ取る）
    Fetch {
        #[arg(long = "pair", required = true)]
        pairs: Vec<String>,
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: Option<String>,
        #[arg(long, default_value = "1M,5M,15M,1H,4H,1D")]
        periods: String,
    },
    /// DB の収録範囲を表示
    Coverage,
    /// 使える戦略とパラメータ
    Strategies,
    /// バックテストを 1 回実行して保存する
    Run {
        #[command(flatten)]
        run: RunArgs,
        /// 表示する取引の件数
        #[arg(long, default_value_t = 0)]
        trades: usize,
        /// 保存しない
        #[arg(long)]
        no_save: bool,
    },
    /// グリッドサーチ（全組み合わせを走らせてスコア順に並べる）
    Sweep {
        #[command(flatten)]
        run: RunArgs,
        #[command(flatten)]
        grid: GridArgs,
        /// 表示する件数
        #[arg(long, default_value_t = 20)]
        top: usize,
    },
    /// ウォークフォワード分析（学習期間で選んだパラメータを直後の期間で検証する）
    WalkForward {
        #[command(flatten)]
        run: RunArgs,
        #[command(flatten)]
        grid: GridArgs,
        #[arg(long, default_value_t = 180)]
        train_days: i64,
        #[arg(long, default_value_t = 60)]
        test_days: i64,
    },
    /// 保存した結果の一覧
    List {
        #[arg(long, default_value_t = 30)]
        limit: usize,
    },
    /// 保存した結果の詳細
    Show {
        #[arg(long)]
        run: i64,
        #[arg(long, default_value_t = 0)]
        trades: usize,
    },
    /// 保存した結果を削除
    Delete {
        #[arg(long)]
        run: i64,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "ntrade=warn".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();
    let mut db = Db::open(&cli.db)?;
    let file = BacktestFileConfig::load_or_default(&cli.config)?;

    match cli.cmd {
        Cmd::Fetch { pairs, from, to, periods } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(fetch(&db, &cli.db, &pairs, parse_date(&from)?, to.map(|s| parse_date(&s)).transpose()?.unwrap_or_else(Utc::now), &periods))?;
            print_coverage(&db)?;
        }
        Cmd::Coverage => print_coverage(&db)?,
        Cmd::Strategies => {
            for s in strategy::catalog() {
                println!("{}  — {}\n  {}", s.name, s.title, s.description);
                for p in s.params {
                    println!("    {:<14} 既定 {:<8} 範囲 {}..{}  {}", p.name, p.default, p.min, p.max, p.description);
                }
                println!();
            }
        }
        Cmd::Run { run, trades, no_save } => {
            let req = run_request(&run)?;
            let started = std::time::Instant::now();
            let (data, cfg) = config::prepare(&db, &file, &req, 0)?;
            let result = backtest::run(&data, &cfg)?;
            print_result(&result, started.elapsed());
            if trades > 0 {
                print_trades(&result.trades, trades);
            }
            if !no_save {
                let id = config::save_single(&mut db, &result, &run.label)?;
                println!("\nsaved as run #{id}");
            }
        }
        Cmd::Sweep { run, grid, top } => {
            let req = grid_request(&run, &grid)?;
            let started = std::time::Instant::now();
            let out = config::run_sweep(&mut db, &file, &req, threads(&grid), &Progress::default())?;
            let cfg = &out.config;
            println!(
                "{} combinations × {} ({} .. {}, exec {}) done in {:.1}s\n",
                out.combinations,
                cfg.pair,
                cfg.from,
                cfg.to,
                out.exec_period.as_str(),
                started.elapsed().as_secs_f64()
            );
            println!("{:>4}  {:>9}  {}  params", "#", "score", metrics_header());
            for (i, r) in out.rows.iter().take(top).enumerate() {
                println!(
                    "{:>4}  {:>9}  {}  {}",
                    i + 1,
                    r.score.map(|s| format!("{s:.3}")).unwrap_or_else(|| "-".into()),
                    metrics_row(&r.metrics),
                    params_label(&r.params)
                );
            }
            println!("\nsaved as run #{}", out.id);
        }
        Cmd::WalkForward { run, grid, train_days, test_days } => {
            let req = grid_request(&run, &grid)?;
            let started = std::time::Instant::now();
            let out = config::run_walk_forward(
                &mut db,
                &file,
                &req,
                WalkForwardWindows { train_days, test_days },
                threads(&grid),
                &Progress::default(),
            )?;
            let cfg = &out.config;
            println!(
                "walk-forward: {} combinations, train {train_days}d / test {test_days}d, {} .. {} done in {:.1}s\n",
                out.combinations,
                cfg.from,
                cfg.to,
                started.elapsed().as_secs_f64()
            );
            println!("{:<23} {:<23} {:>8} {:>8}  params", "train", "test", "train", "test");
            for w in &out.result.windows {
                let fmt = |m: &Option<Metrics>| m.as_ref().map(|m| format!("{:+.0}", m.net_profit)).unwrap_or_else(|| "-".into());
                println!(
                    "{} {} {:>8} {:>8}  {}",
                    range_label(w.train_from, w.train_to),
                    range_label(w.test_from, w.test_to),
                    fmt(&w.train),
                    fmt(&w.test),
                    w.params.as_ref().map(params_label).unwrap_or_else(|| "(no combination met the criteria)".into())
                );
            }
            println!("\n検証期間だけをつないだ成績（未知のデータでの成績）:");
            print_metrics(&out.result.oos);
            println!("\nsaved as run #{}", out.id);
        }
        Cmd::List { limit } => {
            println!("{:<5} {:<19} {:<12} {:<10} {:<23} {:>7} {:>11} {:>6}  strategy / label", "#", "created", "kind", "pair", "period", "trades", "net", "PF");
            for r in db.backtest_runs(limit)? {
                let m: Metrics = serde_json::from_value(r.metrics.clone()).unwrap_or_default();
                println!(
                    "{:<5} {:<19} {:<12} {:<10} {:<23} {:>7} {:>11.0} {:>6}  {} {}",
                    r.id,
                    r.created_at,
                    r.kind,
                    r.pair,
                    format!("{}..{}", &r.from_ts[..10], &r.to_ts[..10]),
                    m.trades,
                    m.net_profit,
                    m.profit_factor.map(|v| format!("{v:.2}")).unwrap_or_else(|| "-".into()),
                    r.strategy,
                    r.label
                );
            }
        }
        Cmd::Show { run, trades } => {
            let r = db.backtest_run(run)?.ok_or_else(|| anyhow!("run {run} not found"))?;
            println!("#{} {} {} {} {}..{} {}", r.id, r.kind, r.pair, r.strategy, r.from_ts, r.to_ts, r.label);
            let m: Metrics = serde_json::from_value(r.metrics)?;
            print_metrics(&m);
            if trades > 0 {
                print_trades(&db.backtest_trades(run)?, trades);
            }
        }
        Cmd::Delete { run } => {
            if db.delete_backtest_run(run)? {
                println!("deleted run #{run}");
            } else {
                bail!("run {run} not found");
            }
        }
    }
    Ok(())
}

async fn fetch(db: &Db, db_path: &std::path::Path, pairs: &[String], from: DateTime<Utc>, to: DateTime<Utc>, periods: &str) -> Result<()> {
    // bot が Refresh Token で更新したトークンは SQLite にあり、.env の方は古くなっている
    let mut cfg = CTraderConfig::from_env()?;
    if let Some(tokens) = db.load_ctrader_tokens()? {
        cfg = cfg.with_tokens(&tokens);
    }
    let service = CTraderService::connect(cfg).await?;
    let periods: Vec<BarPeriod> =
        periods.split(',').map(|p| BarPeriod::parse(p.trim()).ok_or_else(|| anyhow!("unknown period {p}"))).collect::<Result<_>>()?;
    let req = FetchRequest { pairs: pairs.to_vec(), from, to: Some(to), periods };
    backtest::fetch::fetch(&service, db_path, &req, &Progress::default(), |seg, n| println!("{}  {n} bars", seg.label())).await?;
    Ok(())
}

fn run_request(a: &RunArgs) -> Result<RunRequest> {
    let mut params = Params::new();
    for p in &a.params {
        let (k, v) = p.split_once('=').ok_or_else(|| anyhow!("--param '{p}' must be name=value"))?;
        params.insert(k.trim().to_string(), v.trim().parse().map_err(|e| anyhow!("--param '{p}': {e}"))?);
    }
    let exec_timeframe = match a.exec_tf.as_deref().map(str::trim) {
        None | Some("auto") => None,
        Some(s) => Some(BarPeriod::parse(s).ok_or_else(|| anyhow!("unknown --exec-tf {s}"))?),
    };
    Ok(RunRequest {
        pair: a.pair.clone(),
        from: parse_date(&a.from)?,
        to: a.to.as_deref().map(parse_date).transpose()?.unwrap_or_else(Utc::now),
        strategy: StrategySpec {
            name: a.strategy.clone(),
            timeframe: BarPeriod::parse(&a.tf).ok_or_else(|| anyhow!("unknown --tf {}", a.tf))?,
            params,
        },
        initial_balance: a.balance,
        spread_pips: a.spread,
        commission_per_lot: a.commission,
        exec_timeframe,
        label: a.label.clone(),
    })
}

fn grid_request(run: &RunArgs, g: &GridArgs) -> Result<GridRequest> {
    Ok(GridRequest { run: run_request(run)?, grid: g.grid.clone(), objective: Objective::parse(&g.objective)?, min_trades: g.min_trades })
}

fn threads(g: &GridArgs) -> usize {
    g.threads.unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4))
}

fn parse_date(s: &str) -> Result<DateTime<Utc>> {
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(Utc.from_utc_datetime(&d.and_hms_opt(0, 0, 0).unwrap()));
    }
    let secs = ntrade::storage::parse_ts(s)?;
    Utc.timestamp_opt(secs, 0).single().ok_or_else(|| anyhow!("bad date {s}"))
}

fn range_label(a: DateTime<Utc>, b: DateTime<Utc>) -> String {
    format!("{}..{}", a.format("%Y-%m-%d"), b.format("%y-%m-%d"))
}

fn params_label(p: &Params) -> String {
    p.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(" ")
}

fn metrics_header() -> String {
    format!("{:>6} {:>6} {:>11} {:>6} {:>7} {:>9}", "trades", "win%", "net", "PF", "maxDD%", "sharpe")
}

fn metrics_row(m: &Metrics) -> String {
    format!(
        "{:>6} {:>6.1} {:>11.0} {:>6} {:>7.2} {:>9}",
        m.trades,
        m.win_rate,
        m.net_profit,
        m.profit_factor.map(|v| format!("{v:.2}")).unwrap_or_else(|| "-".into()),
        m.max_drawdown_pct,
        m.sharpe.map(|v| format!("{v:.2}")).unwrap_or_else(|| "-".into())
    )
}

fn print_result(r: &BacktestResult, elapsed: std::time::Duration) {
    let c = &r.config;
    println!("{} {}  {} .. {}", c.pair, c.strategy.label(), c.from, c.to);
    println!(
        "exec {}  conversion {}  spread {} pips  slippage {} pips  commission {}/lot/side  decisions {}  ({:.1}s)",
        r.exec_period.as_str(),
        r.conversion,
        c.costs.spread_pips,
        c.costs.slippage_pips,
        c.costs.commission_per_lot,
        r.decisions,
        elapsed.as_secs_f64()
    );
    let s = &r.signals;
    println!(
        "signals: open {} (placed {}, rejected {}) close {} cancel {} modify {}",
        s.intents, s.placed, s.rejected_total, s.closes, s.cancels, s.modifies
    );
    if !s.rejected.is_empty() {
        let list: Vec<String> = s.rejected.iter().map(|(k, v)| format!("{k}={v}")).collect();
        println!("rejected by: {}", list.join(" "));
    }
    println!();
    print_metrics(&r.metrics);
}

fn print_metrics(m: &Metrics) {
    let opt = |v: Option<f64>| v.map(|v| format!("{v:.2}")).unwrap_or_else(|| "-".into());
    println!("  取引数        {:>10}   (買い {} / 売り {})", m.trades, m.long_trades, m.short_trades);
    println!("  勝率          {:>9.1}%   (買い {:.1}% / 売り {:.1}%)", m.win_rate, m.long_win_rate, m.short_win_rate);
    println!("  純損益        {:>10.0}   ({:+.2}%)  初期 {:.0} → {:.0}", m.net_profit, m.return_pct, m.initial_balance, m.final_balance);
    println!("  PF            {:>10}   総利益 {:.0} / 総損失 {:.0}", opt(m.profit_factor), m.gross_profit, m.gross_loss);
    println!("  期待値/取引   {:>10.0}   平均利益 {:.0} / 平均損失 {:.0} (ペイオフ {})", m.expectancy, m.avg_win, m.avg_loss, opt(m.payoff_ratio));
    println!("  pips          {:>10.1}   (平均 {:.1})", m.total_pips, m.avg_pips);
    println!("  最大DD        {:>10.0}   ({:.2}%)  リカバリー {}", m.max_drawdown, m.max_drawdown_pct, opt(m.recovery_factor));
    println!("  シャープ      {:>10}   最大連勝 {} / 最大連敗 {}", opt(m.sharpe), m.max_consecutive_wins, m.max_consecutive_losses);
    println!("  平均保有      {:>9.0}分   手数料 {:.0} / スワップ {:.0}", m.avg_hold_minutes, m.commission, m.swap);
    let reasons: Vec<String> = m.exit_reasons.iter().map(|(k, v)| format!("{k}={v}")).collect();
    println!("  決済理由      {}", reasons.join(" "));
    if !m.monthly.is_empty() {
        println!("\n  月別:");
        for mo in &m.monthly {
            println!("    {}  {:>4} 取引  {:>10.0}  {:>8.1} pips", mo.month, mo.trades, mo.net, mo.pips);
        }
    }
}

fn print_trades(trades: &[BacktestTrade], n: usize) {
    println!("\n  直近 {} 件の取引:", n.min(trades.len()));
    for t in trades.iter().rev().take(n).rev() {
        println!(
            "    {} {:<4} {:<6} {:>9.5} → {:>9.5} {} {:>7.1}p {:>9.0}  {:<12} {}",
            t.entry_time.format("%Y-%m-%d %H:%M"),
            t.side.as_str(),
            t.entry_kind,
            t.entry_price,
            t.exit_price,
            t.exit_time.format("%m-%d %H:%M"),
            t.pnl_pips,
            t.net,
            t.exit_reason.as_str(),
            t.entry_reason
        );
    }
}

fn print_coverage(db: &Db) -> Result<()> {
    println!("{:<10} {:<6} {:>9}  {:<20} {:<20}", "pair", "period", "bars", "first", "last");
    for c in db.coverage()? {
        println!("{:<10} {:<6} {:>9}  {:<20} {:<20}", c.pair, c.period, c.count, c.first.unwrap_or_default(), c.last.unwrap_or_default());
    }
    Ok(())
}
