//! SQLite 永続化。ヒストリカルバー・cTrader OAuth トークン・設定、
//! 本番のシグナルログ・ポジション・決済履歴（`journal`）、バックテスト結果（`backtest`）を保存する。
//!
//! LLM 時代のテーブル（cot_logs / lessons / replay_runs / replay_decisions）は作らなくなったが、
//! 既存の DB に残っているデータは消さない。

mod backtest;
mod journal;

pub use backtest::{BacktestRunRow, NewBacktestRun};

pub use journal::Page;

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
            CREATE TABLE IF NOT EXISTS oauth_tokens (
              provider      TEXT PRIMARY KEY,
              access_token  TEXT NOT NULL,
              refresh_token TEXT,
              expires_at    INTEGER,
              updated_at    INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS app_settings (
              key           TEXT PRIMARY KEY,
              value         TEXT NOT NULL,
              updated_at    INTEGER NOT NULL
            );
            -- 外部 API から取得したデータのキャッシュ（設定とは分けて持つ）
            CREATE TABLE IF NOT EXISTS api_cache (
              key           TEXT PRIMARY KEY,
              value         TEXT NOT NULL,
              updated_at    INTEGER NOT NULL
            );
            -- 以前は設定テーブルにキャッシュを置いていた。設定以外のものは設定テーブルに残さない
            DELETE FROM app_settings WHERE key = 'fundamentals_cache';
            "#,
        )?;
        self.migrate_journal()?;
        self.migrate_backtest()?;
        Ok(())
    }

    // ------------------------------------------------------------ settings

    /// 画面から変更できる設定（JSON で保存）。未保存なら None
    pub fn load_setting<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let raw: Option<String> = self
            .conn
            .query_row("SELECT value FROM app_settings WHERE key = ?1", params![key], |r| r.get(0))
            .optional()?;
        raw.map(|s| serde_json::from_str(&s).with_context(|| format!("invalid setting {key}")))
            .transpose()
    }

    pub fn save_setting<T: serde::Serialize>(&self, key: &str, value: &T) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO app_settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
            params![key, serde_json::to_string(value)?, Utc::now().timestamp()],
        )?;
        Ok(())
    }

    // ------------------------------------------------------------ api cache

    /// 外部 API のキャッシュ（JSON）。無ければ None
    pub fn load_cache<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let raw: Option<String> = self
            .conn
            .query_row("SELECT value FROM api_cache WHERE key = ?1", params![key], |r| r.get(0))
            .optional()?;
        raw.map(|s| serde_json::from_str(&s).with_context(|| format!("invalid cache {key}")))
            .transpose()
    }

    pub fn save_cache<T: serde::Serialize>(&self, key: &str, value: &T) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO api_cache (key, value, updated_at) VALUES (?1, ?2, ?3)",
            params![key, serde_json::to_string(value)?, Utc::now().timestamp()],
        )?;
        Ok(())
    }

    /// ブローカーから取得した銘柄仕様を保存する（未接続のバックテストで lot サイズ・pip 位置を使うため）
    pub fn save_instrument(&self, spec: &crate::instrument::InstrumentSpec) -> Result<()> {
        self.save_cache(&instrument_key(&spec.symbol_name), spec)
    }

    /// 保存済みの銘柄仕様を `instrument` に登録する。無ければ false（名前からの推測で動く）
    pub fn register_saved_instrument(&self, symbol: &str) -> Result<bool> {
        match self.load_cache::<crate::instrument::InstrumentSpec>(&instrument_key(symbol))? {
            Some(spec) => {
                crate::instrument::register(spec);
                Ok(true)
            }
            None => Ok(false),
        }
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

    /// 開始時刻が `[from, to)` の足を昇順で返す
    pub fn bars_range(&self, pair: &str, period: BarPeriod, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<CandleBar>> {
        let mut stmt = self.conn.prepare(
            "SELECT ts, open, high, low, close, volume FROM bars_history
             WHERE pair = ?1 AND period = ?2 AND ts >= ?3 AND ts < ?4
             ORDER BY ts ASC",
        )?;
        let rows = stmt
            .query_map(params![pair.to_uppercase(), period.as_str(), from.timestamp(), to.timestamp()], row_to_bar)?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    pub fn first_bar_ts(&self, pair: &str, period: BarPeriod) -> Result<Option<DateTime<Utc>>> {
        let ts: Option<i64> = self
            .conn
            .query_row(
                "SELECT MIN(ts) FROM bars_history WHERE pair = ?1 AND period = ?2",
                params![pair.to_uppercase(), period.as_str()],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        Ok(ts.and_then(|s| Utc.timestamp_opt(s, 0).single()))
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

}

fn instrument_key(symbol: &str) -> String {
    format!("instrument:{}", symbol.trim().to_uppercase())
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

    #[test]
    fn settings_roundtrip() {
        let db = Db::open_in_memory().unwrap();
        assert!(db.load_setting::<Vec<String>>("pairs").unwrap().is_none());
        db.save_setting("pairs", &vec!["USDJPY".to_string()]).unwrap();
        db.save_setting("pairs", &vec!["USDJPY".to_string(), "EURUSD".to_string()]).unwrap();
        assert_eq!(db.load_setting::<Vec<String>>("pairs").unwrap().unwrap(), ["USDJPY", "EURUSD"]);
    }
}
