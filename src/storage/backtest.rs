//! バックテスト結果の保存。
//!
//! - `backtest_runs`: 1 回の実行（単発 / グリッドサーチ / ウォークフォワード）。条件と成績は JSON で持つ
//! - `backtest_trades`: 単発・ウォークフォワード（検証期間）の取引
//! - `backtest_equity`: 単発の資産の推移（間引いて保存）

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use super::{fmt_ts, Db};
use crate::backtest::{BacktestTrade, EquityPoint};

/// backtest_runs の 1 行
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BacktestRunRow {
    pub id: i64,
    pub created_at: String,
    pub label: String,
    /// single / sweep / walk_forward
    pub kind: String,
    pub pair: String,
    /// `StrategySpec::label`
    pub strategy: String,
    pub from_ts: String,
    pub to_ts: String,
    /// 実行条件（`BacktestConfig` と実行時の補足）
    pub config: serde_json::Value,
    /// 成績（単発・ウォークフォワードは `Metrics`、グリッドサーチは最良の組み合わせの `Metrics`）
    pub metrics: serde_json::Value,
    /// グリッドサーチの全結果・ウォークフォワードの窓ごとの結果
    pub summary: serde_json::Value,
}

/// 保存する実行
pub struct NewBacktestRun<'a> {
    pub label: &'a str,
    pub kind: &'a str,
    pub pair: &'a str,
    pub strategy: &'a str,
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub config: serde_json::Value,
    pub metrics: serde_json::Value,
    pub summary: serde_json::Value,
}

const RUN_COLS: &str = "id, created_at, label, kind, pair, strategy, from_ts, to_ts, config_json, metrics_json, summary_json";

fn json(s: String) -> serde_json::Value {
    serde_json::from_str(&s).unwrap_or(serde_json::Value::Null)
}

fn row_to_run(r: &Row) -> rusqlite::Result<BacktestRunRow> {
    Ok(BacktestRunRow {
        id: r.get(0)?,
        created_at: fmt_ts(r.get(1)?),
        label: r.get(2)?,
        kind: r.get(3)?,
        pair: r.get(4)?,
        strategy: r.get(5)?,
        from_ts: fmt_ts(r.get(6)?),
        to_ts: fmt_ts(r.get(7)?),
        config: json(r.get(8)?),
        metrics: json(r.get(9)?),
        summary: json(r.get::<_, Option<String>>(10)?.unwrap_or_default()),
    })
}

impl Db {
    pub(super) fn migrate_backtest(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS backtest_runs (
              id           INTEGER PRIMARY KEY AUTOINCREMENT,
              created_at   INTEGER NOT NULL,
              label        TEXT NOT NULL DEFAULT '',
              kind         TEXT NOT NULL,
              pair         TEXT NOT NULL,
              strategy     TEXT NOT NULL,
              from_ts      INTEGER NOT NULL,
              to_ts        INTEGER NOT NULL,
              config_json  TEXT NOT NULL,
              metrics_json TEXT NOT NULL,
              summary_json TEXT
            );
            CREATE TABLE IF NOT EXISTS backtest_trades (
              run_id INTEGER NOT NULL REFERENCES backtest_runs(id) ON DELETE CASCADE,
              seq    INTEGER NOT NULL,
              json   TEXT NOT NULL,
              PRIMARY KEY (run_id, seq)
            );
            CREATE TABLE IF NOT EXISTS backtest_equity (
              run_id  INTEGER NOT NULL REFERENCES backtest_runs(id) ON DELETE CASCADE,
              ts      INTEGER NOT NULL,
              balance REAL NOT NULL,
              equity  REAL NOT NULL,
              PRIMARY KEY (run_id, ts)
            );
            "#,
        )?;
        Ok(())
    }

    /// 実行を保存して ID を返す
    pub fn insert_backtest_run(&mut self, run: &NewBacktestRun, trades: &[BacktestTrade], equity: &[EquityPoint]) -> Result<i64> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO backtest_runs (created_at, label, kind, pair, strategy, from_ts, to_ts, config_json, metrics_json, summary_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                Utc::now().timestamp(),
                run.label,
                run.kind,
                run.pair.to_uppercase(),
                run.strategy,
                run.from.timestamp(),
                run.to.timestamp(),
                run.config.to_string(),
                run.metrics.to_string(),
                run.summary.to_string()
            ],
        )?;
        let id = tx.last_insert_rowid();
        {
            let mut stmt = tx.prepare("INSERT INTO backtest_trades (run_id, seq, json) VALUES (?1, ?2, ?3)")?;
            for (i, t) in trades.iter().enumerate() {
                stmt.execute(params![id, i as i64, serde_json::to_string(t)?])?;
            }
            let mut stmt = tx.prepare("INSERT OR REPLACE INTO backtest_equity (run_id, ts, balance, equity) VALUES (?1, ?2, ?3, ?4)")?;
            for p in equity {
                stmt.execute(params![id, p.time.timestamp(), p.balance, p.equity])?;
            }
        }
        tx.commit()?;
        Ok(id)
    }

    /// 新しい順
    pub fn backtest_runs(&self, limit: usize) -> Result<Vec<BacktestRunRow>> {
        let mut stmt = self.conn.prepare(&format!("SELECT {RUN_COLS} FROM backtest_runs ORDER BY id DESC LIMIT ?1"))?;
        let rows = stmt.query_map(params![limit as i64], row_to_run)?.collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    pub fn backtest_run(&self, id: i64) -> Result<Option<BacktestRunRow>> {
        Ok(self
            .conn
            .query_row(&format!("SELECT {RUN_COLS} FROM backtest_runs WHERE id = ?1"), params![id], row_to_run)
            .optional()?)
    }

    pub fn backtest_trades(&self, run_id: i64) -> Result<Vec<BacktestTrade>> {
        let mut stmt = self.conn.prepare("SELECT json FROM backtest_trades WHERE run_id = ?1 ORDER BY seq")?;
        let rows: Vec<String> = stmt.query_map(params![run_id], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
        rows.iter().map(|j| Ok(serde_json::from_str(j)?)).collect()
    }

    pub fn backtest_equity(&self, run_id: i64) -> Result<Vec<EquityPoint>> {
        let mut stmt = self.conn.prepare("SELECT ts, balance, equity FROM backtest_equity WHERE run_id = ?1 ORDER BY ts")?;
        let rows = stmt
            .query_map(params![run_id], |r| {
                Ok(EquityPoint {
                    time: DateTime::from_timestamp(r.get(0)?, 0).unwrap_or_default(),
                    balance: r.get(1)?,
                    equity: r.get(2)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    /// 削除できたら true
    pub fn delete_backtest_run(&mut self, id: i64) -> Result<bool> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM backtest_trades WHERE run_id = ?1", params![id])?;
        tx.execute("DELETE FROM backtest_equity WHERE run_id = ?1", params![id])?;
        let n = tx.execute("DELETE FROM backtest_runs WHERE id = ?1", params![id])?;
        tx.commit()?;
        Ok(n > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backtest::ExitReason;
    use crate::strategy::Side;
    use chrono::TimeZone;

    #[test]
    fn runs_roundtrip_with_trades_and_equity() {
        let mut db = Db::open_in_memory().unwrap();
        let t = Utc.with_ymd_and_hms(2026, 9, 1, 0, 0, 0).unwrap();
        let trade = BacktestTrade {
            id: "pos-1".into(),
            side: Side::Buy,
            volume_lots: 0.1,
            entry_kind: "MARKET".into(),
            entry_time: t,
            entry_price: 150.0,
            exit_time: t,
            exit_price: 150.1,
            stop_loss: Some(149.9),
            take_profit: None,
            pnl_pips: 10.0,
            gross: 1000.0,
            commission: 160.0,
            swap: 0.0,
            net: 840.0,
            exit_reason: ExitReason::Strategy,
            entry_reason: "r".into(),
            exit_note: String::new(),
            mae_pips: 1.0,
            mfe_pips: 12.0,
        };
        let run = NewBacktestRun {
            label: "x",
            kind: "single",
            pair: "usdjpy_z",
            strategy: "ema_cross M5",
            from: t,
            to: t,
            config: serde_json::json!({"a": 1}),
            metrics: serde_json::json!({"trades": 1}),
            summary: serde_json::Value::Null,
        };
        let id = db.insert_backtest_run(&run, std::slice::from_ref(&trade), &[EquityPoint { time: t, balance: 1.0, equity: 1.0 }]).unwrap();
        let row = db.backtest_run(id).unwrap().unwrap();
        assert_eq!((row.pair.as_str(), row.metrics["trades"].as_i64()), ("USDJPY_Z", Some(1)));
        assert_eq!(db.backtest_trades(id).unwrap(), vec![trade]);
        assert_eq!(db.backtest_equity(id).unwrap().len(), 1);
        assert_eq!(db.backtest_runs(10).unwrap().len(), 1);
        assert!(db.delete_backtest_run(id).unwrap());
        assert!(db.backtest_trades(id).unwrap().is_empty());
    }
}
