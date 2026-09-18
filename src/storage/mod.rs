//! SQLite 永続化。ヒストリカルバー・リプレイ結果・cTrader OAuth トークンを保存する。

use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

use crate::ctrader::{BarPeriod, CandleBar, TokenSet};

const CTRADER_PROVIDER: &str = "ctrader";

pub const DEFAULT_DB_PATH: &str = "data/ntrade.db";

pub struct Db {
    conn: Connection,
}

/// bars_history の収録範囲
#[derive(Debug, Clone, serde::Serialize)]
pub struct Coverage {
    pub pair: String,
    pub period: String,
    pub count: i64,
    pub first: Option<String>,
    pub last: Option<String>,
}

/// replay_runs 1行
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReplayRunRow {
    pub id: i64,
    pub created_at: String,
    pub label: String,
    pub pair: String,
    pub from_ts: String,
    pub to_ts: String,
    pub sampling: String,
    pub prompt_hash: String,
    pub snapshot_ver: String,
    pub guard_config: String,
    pub decisions: i64,
}

/// replay_decisions 1行
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReplayDecisionRow {
    pub run_id: i64,
    pub t: String,
    pub decision_json: String,
    pub guard_result: String,
    pub outcome: String,
    pub pnl_pips: Option<f64>,
    pub bars_to_exit: Option<i64>,
    pub chart_dir: String,
    pub session: String,
    pub confidence: f64,
    pub action: String,
}

impl Db {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path.as_ref())
            .with_context(|| format!("Failed to open SQLite at {:?}", path.as_ref()))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS bars_history (
              pair    TEXT    NOT NULL,
              period  TEXT    NOT NULL,
              ts      INTEGER NOT NULL,
              open REAL NOT NULL, high REAL NOT NULL, low REAL NOT NULL, close REAL NOT NULL,
              volume  INTEGER,
              PRIMARY KEY (pair, period, ts)
            );
            CREATE TABLE IF NOT EXISTS replay_runs (
              id            INTEGER PRIMARY KEY AUTOINCREMENT,
              created_at    INTEGER NOT NULL,
              label         TEXT NOT NULL DEFAULT '',
              pair          TEXT NOT NULL,
              from_ts       INTEGER NOT NULL,
              to_ts         INTEGER NOT NULL,
              sampling      TEXT NOT NULL,
              prompt_hash   TEXT NOT NULL,
              snapshot_ver  TEXT NOT NULL,
              guard_config  TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS replay_decisions (
              run_id        INTEGER NOT NULL REFERENCES replay_runs(id),
              t             INTEGER NOT NULL,
              decision_json TEXT NOT NULL,
              guard_result  TEXT NOT NULL,
              outcome       TEXT NOT NULL,
              pnl_pips      REAL,
              bars_to_exit  INTEGER,
              chart_dir     TEXT NOT NULL,
              session       TEXT NOT NULL,
              confidence    REAL NOT NULL,
              action        TEXT NOT NULL,
              PRIMARY KEY (run_id, t)
            );
            CREATE TABLE IF NOT EXISTS oauth_tokens (
              provider      TEXT PRIMARY KEY,
              access_token  TEXT NOT NULL,
              refresh_token TEXT,
              expires_at    INTEGER,
              updated_at    INTEGER NOT NULL
            );
            "#,
        )?;
        Ok(())
    }

    // -------------------------------------------------------------- tokens

    pub fn save_ctrader_tokens(&self, tokens: &TokenSet) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO oauth_tokens (provider, access_token, refresh_token, expires_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                CTRADER_PROVIDER,
                tokens.access_token,
                tokens.refresh_token,
                tokens.expires_at,
                Utc::now().timestamp()
            ],
        )?;
        Ok(())
    }

    pub fn load_ctrader_tokens(&self) -> Result<Option<TokenSet>> {
        Ok(self
            .conn
            .query_row(
                "SELECT access_token, refresh_token, expires_at FROM oauth_tokens WHERE provider = ?1",
                params![CTRADER_PROVIDER],
                |r| Ok(TokenSet { access_token: r.get(0)?, refresh_token: r.get(1)?, expires_at: r.get(2)? }),
            )
            .optional()?)
    }

    // ---------------------------------------------------------------- bars

    pub fn insert_bars(&mut self, pair: &str, period: BarPeriod, bars: &[CandleBar]) -> Result<usize> {
        let tx = self.conn.transaction()?;
        let mut n = 0;
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO bars_history (pair, period, ts, open, high, low, close, volume)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?;
            for b in bars {
                n += stmt.execute(params![
                    pair.to_uppercase(),
                    period.as_str(),
                    b.timestamp.timestamp(),
                    b.open,
                    b.high,
                    b.low,
                    b.close,
                    b.volume
                ])?;
            }
        }
        tx.commit()?;
        Ok(n)
    }

    /// `t` 時点で確定している（開始時刻 + 足の長さ <= t）直近 `count` 本を昇順で返す
    pub fn bars_until(&self, pair: &str, period: BarPeriod, t: DateTime<Utc>, count: usize) -> Result<Vec<CandleBar>> {
        let latest_start = (t - period.duration()).timestamp();
        let mut stmt = self.conn.prepare(
            "SELECT ts, open, high, low, close, volume FROM bars_history
             WHERE pair = ?1 AND period = ?2 AND ts <= ?3
             ORDER BY ts DESC LIMIT ?4",
        )?;
        let mut rows: Vec<CandleBar> = stmt
            .query_map(params![pair.to_uppercase(), period.as_str(), latest_start, count as i64], row_to_bar)?
            .collect::<std::result::Result<_, _>>()?;
        rows.reverse();
        Ok(rows)
    }

    /// `t` 以降に開始する足を昇順で最大 `limit` 本
    pub fn bars_from(&self, pair: &str, period: BarPeriod, t: DateTime<Utc>, limit: usize) -> Result<Vec<CandleBar>> {
        let mut stmt = self.conn.prepare(
            "SELECT ts, open, high, low, close, volume FROM bars_history
             WHERE pair = ?1 AND period = ?2 AND ts >= ?3
             ORDER BY ts ASC LIMIT ?4",
        )?;
        let rows = stmt
            .query_map(params![pair.to_uppercase(), period.as_str(), t.timestamp(), limit as i64], row_to_bar)?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    pub fn last_bar_ts(&self, pair: &str, period: BarPeriod) -> Result<Option<DateTime<Utc>>> {
        let ts: Option<i64> = self
            .conn
            .query_row(
                "SELECT MAX(ts) FROM bars_history WHERE pair = ?1 AND period = ?2",
                params![pair.to_uppercase(), period.as_str()],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        Ok(ts.and_then(|s| Utc.timestamp_opt(s, 0).single()))
    }

    pub fn coverage(&self) -> Result<Vec<Coverage>> {
        let mut stmt = self.conn.prepare(
            "SELECT pair, period, COUNT(*), MIN(ts), MAX(ts) FROM bars_history GROUP BY pair, period ORDER BY pair, period",
        )?;
        let rows = stmt
            .query_map([], |r| {
                let min: Option<i64> = r.get(3)?;
                let max: Option<i64> = r.get(4)?;
                Ok(Coverage {
                    pair: r.get(0)?,
                    period: r.get(1)?,
                    count: r.get(2)?,
                    first: min.map(fmt_ts),
                    last: max.map(fmt_ts),
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    // -------------------------------------------------------------- replay

    #[allow(clippy::too_many_arguments)]
    pub fn create_run(
        &self,
        label: &str,
        pair: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        sampling: &str,
        prompt_hash: &str,
        snapshot_ver: &str,
        guard_config: &str,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO replay_runs (created_at, label, pair, from_ts, to_ts, sampling, prompt_hash, snapshot_ver, guard_config)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                Utc::now().timestamp(),
                label,
                pair.to_uppercase(),
                from.timestamp(),
                to.timestamp(),
                sampling,
                prompt_hash,
                snapshot_ver,
                guard_config
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn insert_decision(&self, row: &ReplayDecisionRow) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO replay_decisions
             (run_id, t, decision_json, guard_result, outcome, pnl_pips, bars_to_exit, chart_dir, session, confidence, action)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                row.run_id,
                parse_ts(&row.t)?,
                row.decision_json,
                row.guard_result,
                row.outcome,
                row.pnl_pips,
                row.bars_to_exit,
                row.chart_dir,
                row.session,
                row.confidence,
                row.action
            ],
        )?;
        Ok(())
    }

    /// 既に評価済みの時刻（再開用）
    pub fn decided_ts(&self, run_id: i64) -> Result<Vec<DateTime<Utc>>> {
        let mut stmt = self.conn.prepare("SELECT t FROM replay_decisions WHERE run_id = ?1")?;
        let rows = stmt
            .query_map(params![run_id], |r| r.get::<_, i64>(0))?
            .filter_map(|r| r.ok())
            .filter_map(|s| Utc.timestamp_opt(s, 0).single())
            .collect();
        Ok(rows)
    }

    pub fn list_runs(&self) -> Result<Vec<ReplayRunRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT r.id, r.created_at, r.label, r.pair, r.from_ts, r.to_ts, r.sampling, r.prompt_hash, r.snapshot_ver, r.guard_config,
                    (SELECT COUNT(*) FROM replay_decisions d WHERE d.run_id = r.id)
             FROM replay_runs r ORDER BY r.id DESC",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(ReplayRunRow {
                    id: r.get(0)?,
                    created_at: fmt_ts(r.get(1)?),
                    label: r.get(2)?,
                    pair: r.get(3)?,
                    from_ts: fmt_ts(r.get(4)?),
                    to_ts: fmt_ts(r.get(5)?),
                    sampling: r.get(6)?,
                    prompt_hash: r.get(7)?,
                    snapshot_ver: r.get(8)?,
                    guard_config: r.get(9)?,
                    decisions: r.get(10)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    pub fn get_run(&self, run_id: i64) -> Result<Option<ReplayRunRow>> {
        Ok(self.list_runs()?.into_iter().find(|r| r.id == run_id))
    }

    pub fn decisions(&self, run_id: i64) -> Result<Vec<ReplayDecisionRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT run_id, t, decision_json, guard_result, outcome, pnl_pips, bars_to_exit, chart_dir, session, confidence, action
             FROM replay_decisions WHERE run_id = ?1 ORDER BY t ASC",
        )?;
        let rows = stmt
            .query_map(params![run_id], |r| {
                Ok(ReplayDecisionRow {
                    run_id: r.get(0)?,
                    t: fmt_ts(r.get(1)?),
                    decision_json: r.get(2)?,
                    guard_result: r.get(3)?,
                    outcome: r.get(4)?,
                    pnl_pips: r.get(5)?,
                    bars_to_exit: r.get(6)?,
                    chart_dir: r.get(7)?,
                    session: r.get(8)?,
                    confidence: r.get(9)?,
                    action: r.get(10)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }
}

fn row_to_bar(r: &rusqlite::Row) -> rusqlite::Result<CandleBar> {
    let ts: i64 = r.get(0)?;
    Ok(CandleBar {
        timestamp: Utc.timestamp_opt(ts, 0).single().unwrap_or_else(Utc::now),
        open: r.get(1)?,
        high: r.get(2)?,
        low: r.get(3)?,
        close: r.get(4)?,
        volume: r.get::<_, Option<i64>>(5)?.unwrap_or(0),
    })
}

pub fn fmt_ts(ts: i64) -> String {
    Utc.timestamp_opt(ts, 0)
        .single()
        .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_default()
}

pub fn parse_ts(s: &str) -> Result<i64> {
    let s = s.trim().trim_end_matches(" UTC");
    let naive = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M"))
        .with_context(|| format!("invalid timestamp: {s}"))?;
    Ok(naive.and_utc().timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn bar(t: DateTime<Utc>, c: f64) -> CandleBar {
        CandleBar { timestamp: t, open: c, high: c + 0.1, low: c - 0.1, close: c, volume: 1 }
    }

    #[test]
    fn bars_until_returns_only_closed_bars() {
        let mut db = Db::open_in_memory().unwrap();
        let base = Utc.with_ymd_and_hms(2026, 9, 15, 9, 0, 0).unwrap();
        let bars: Vec<CandleBar> = (0..10).map(|i| bar(base + Duration::minutes(5 * i), 1.0 + i as f64)).collect();
        db.insert_bars("usdjpy", BarPeriod::M5, &bars).unwrap();

        // t = 09:17 -> 09:10 の足（09:15 に確定）まで。09:15 の足は 09:20 確定なので含まない
        let t = base + Duration::minutes(17);
        let got = db.bars_until("USDJPY", BarPeriod::M5, t, 100).unwrap();
        assert_eq!(got.len(), 3);
        assert_eq!(got.last().unwrap().timestamp, base + Duration::minutes(10));

        // ちょうど確定時刻は含む
        let t = base + Duration::minutes(15);
        let got = db.bars_until("USDJPY", BarPeriod::M5, t, 100).unwrap();
        assert_eq!(got.last().unwrap().timestamp, base + Duration::minutes(10));

        let after = db.bars_from("USDJPY", BarPeriod::M5, t, 100).unwrap();
        assert_eq!(after.first().unwrap().timestamp, base + Duration::minutes(15));
        assert_eq!(after.len(), 7);
    }

    #[test]
    fn run_and_decision_roundtrip() {
        let db = Db::open_in_memory().unwrap();
        let from = Utc.with_ymd_and_hms(2026, 9, 1, 0, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2026, 9, 2, 0, 0, 0).unwrap();
        let id = db.create_run("test", "USDJPY", from, to, "15m", "abc", "v2", "{}").unwrap();
        db.insert_decision(&ReplayDecisionRow {
            run_id: id,
            t: "2026-09-01 09:00:00".into(),
            decision_json: "{}".into(),
            guard_result: "PASS".into(),
            outcome: "TP_HIT".into(),
            pnl_pips: Some(12.5),
            bars_to_exit: Some(7),
            chart_dir: "charts/x".into(),
            session: "LONDON".into(),
            confidence: 0.8,
            action: "BUY".into(),
        })
        .unwrap();
        let runs = db.list_runs().unwrap();
        assert_eq!(runs[0].decisions, 1);
        let d = db.decisions(id).unwrap();
        assert_eq!(d[0].outcome, "TP_HIT");
        assert_eq!(db.decided_ts(id).unwrap().len(), 1);
    }

    #[test]
    fn ctrader_tokens_roundtrip() {
        let db = Db::open_in_memory().unwrap();
        assert!(db.load_ctrader_tokens().unwrap().is_none());
        let mut t = TokenSet { access_token: "a1".into(), refresh_token: Some("r1".into()), expires_at: Some(100) };
        db.save_ctrader_tokens(&t).unwrap();
        t.access_token = "a2".into();
        t.expires_at = None;
        db.save_ctrader_tokens(&t).unwrap();
        assert_eq!(db.load_ctrader_tokens().unwrap(), Some(t));
    }
}
