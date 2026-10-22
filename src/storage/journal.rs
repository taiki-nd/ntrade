//! 本番運用の記録: シグナルログ・保有ポジション・予約注文・決済履歴・残高の推移。
//!
//! シグナルと取引の対応は `signal_id` で辿る: `signals.id` ← `positions.signal_id` / `trades.signal_id`。
//! どの戦略（デプロイ）の取引かは `strategy` 列に持つ。
//! 予約注文は置いたシグナルと戦略を持ち、約定した建玉にそれを引き継ぐ。
//! 時刻は API と同じ "YYYY-MM-DD HH:MM:SS"（UTC）の文字列で持つ（辞書順 = 時系列順）。

use anyhow::Result;
use rusqlite::{named_params, params, OptionalExtension, Row};
use serde::Serialize;

use super::Db;
use crate::ctrader::BalanceEvent;
use crate::server::types::{BalancePoint, CloseReason, Position, SignalLog, SignalQuery, SignalStatus, TradeHistory, TradeQuery, WorkingOrder};

/// ページング結果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub items: Vec<T>,
    /// 条件に一致する全件数
    pub total: i64,
}

const SIGNAL_COLS: &str =
    "id, timestamp, deployment, strategy, symbol, action, side, reason, detail_json, guard_result, status, message, order_id, dry_run";

/// シグナルの絞り込み条件（NULL の名前付きパラメータは絞り込まない）
const SIGNAL_SEARCH_WHERE: &str = r#"WHERE (:symbol IS NULL OR symbol = :symbol COLLATE NOCASE)
      AND (:deployment IS NULL OR deployment = :deployment)
      AND (:status IS NULL OR status = :status)
      AND (:q IS NULL OR lower(
            symbol || ' ' || deployment || ' ' || strategy || ' ' || action || ' ' || COALESCE(side, '') || ' ' ||
            timestamp || ' ' || reason || ' ' || COALESCE(guard_result, '') || ' ' || COALESCE(message, '')
          ) LIKE :q ESCAPE '\')"#;

/// 検索語を LIKE の部分一致パターンにする（ワイルドカードは打ち消す）。空文字なら絞り込まない
fn search_pattern(query: Option<&str>) -> Option<String> {
    let q = query.map(str::trim).filter(|q| !q.is_empty())?;
    let escaped = q.to_lowercase().replace('\\', r"\\").replace('%', r"\%").replace('_', r"\_");
    Some(format!("%{escaped}%"))
}

const POSITION_COLS: &str = "id, symbol, side, volume_lots, entry_price, current_price, stop_loss, take_profit,
    pnl_pips, pnl_amount, open_time, note, signal_id, strategy";

const TRADE_COLS: &str = "id, symbol, side, volume_lots, entry_price, close_price, stop_loss, take_profit,
    pnl_pips, pnl_amount, close_reason, open_time, close_time, signal_id, strategy";

/// 決済履歴の絞り込み条件（NULL の名前付きパラメータは絞り込まない）
const TRADE_SEARCH_WHERE: &str = r#"WHERE (:signal_id IS NULL OR signal_id = :signal_id)
      AND (:symbol IS NULL OR symbol = :symbol COLLATE NOCASE)
      AND (:strategy IS NULL OR strategy = :strategy)
      AND (:side IS NULL OR side = :side)
      AND (:close_reason IS NULL OR close_reason = :close_reason)
      AND (:win IS NULL OR :win = (pnl_amount >= 0))
      AND (:from IS NULL OR close_time >= :from)
      AND (:to IS NULL OR close_time <= :to)
      AND (:min_pnl IS NULL OR pnl_pips >= :min_pnl)
      AND (:max_pnl IS NULL OR pnl_pips <= :max_pnl)
      AND (:q IS NULL OR lower(
            symbol || ' ' || side || ' ' || close_reason || ' ' ||
            open_time || ' ' || close_time || ' ' || COALESCE(signal_id, '') || ' ' || COALESCE(strategy, '')
          ) LIKE :q ESCAPE '\')"#;

/// 空文字を None に畳む（クエリ文字列の `&symbol=` を「指定なし」として扱う）
fn non_empty(v: Option<&String>) -> Option<String> {
    v.map(|s| s.trim()).filter(|s| !s.is_empty()).map(str::to_string)
}

/// 日付だけ（"YYYY-MM-DD"）の上限指定はその日の終わりまでを含める
fn range_end(v: Option<&String>) -> Option<String> {
    non_empty(v).map(|s| if s.len() == 10 { format!("{s} 23:59:59") } else { s })
}

impl Db {
    fn columns(&self, table: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let cols = stmt.query_map([], |r| r.get::<_, String>(1))?.collect::<std::result::Result<_, _>>()?;
        Ok(cols)
    }

    /// 列名の変更（旧名の列があり、新名の列が無いときだけ）
    fn rename_column(&self, table: &str, from: &str, to: &str) -> Result<()> {
        let cols = self.columns(table)?;
        if cols.iter().any(|c| c == from) && !cols.iter().any(|c| c == to) {
            self.conn.execute_batch(&format!("ALTER TABLE {table} RENAME COLUMN {from} TO {to}"))?;
        }
        Ok(())
    }

    fn add_column(&self, table: &str, column: &str, decl: &str) -> Result<()> {
        if !self.columns(table)?.iter().any(|c| c == column) {
            self.conn.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"))?;
        }
        Ok(())
    }

    pub(super) fn migrate_journal(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS signals (
              id           TEXT PRIMARY KEY,
              timestamp    TEXT NOT NULL,
              deployment   TEXT NOT NULL,
              strategy     TEXT NOT NULL,
              symbol       TEXT NOT NULL,
              action       TEXT NOT NULL,
              side         TEXT,
              reason       TEXT NOT NULL,
              detail_json  TEXT NOT NULL,
              guard_result TEXT,
              status       TEXT NOT NULL,
              message      TEXT,
              order_id     TEXT,
              dry_run      INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_signals_ts ON signals (timestamp);
            CREATE INDEX IF NOT EXISTS idx_signals_deployment_ts ON signals (deployment, timestamp);
            CREATE TABLE IF NOT EXISTS positions (
              id            TEXT PRIMARY KEY,
              symbol        TEXT NOT NULL,
              side          TEXT NOT NULL,
              volume_lots   REAL NOT NULL,
              entry_price   REAL NOT NULL,
              current_price REAL NOT NULL,
              stop_loss     REAL NOT NULL,
              take_profit   REAL NOT NULL,
              pnl_pips      REAL NOT NULL,
              pnl_amount    REAL NOT NULL,
              open_time     TEXT NOT NULL,
              note          TEXT NOT NULL,
              signal_id     TEXT,
              strategy      TEXT
            );
            CREATE TABLE IF NOT EXISTS trades (
              id           TEXT PRIMARY KEY,
              symbol       TEXT NOT NULL,
              side         TEXT NOT NULL,
              volume_lots  REAL NOT NULL,
              entry_price  REAL NOT NULL,
              close_price  REAL NOT NULL,
              stop_loss    REAL NOT NULL,
              take_profit  REAL NOT NULL,
              pnl_pips     REAL NOT NULL,
              pnl_amount   REAL NOT NULL,
              close_reason TEXT NOT NULL,
              open_time    TEXT NOT NULL,
              close_time   TEXT NOT NULL,
              signal_id    TEXT,
              strategy     TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_trades_close_time ON trades (close_time);
            -- 約定待ちの予約注文（再起動してもシグナル・戦略との対応を失わないため）
            CREATE TABLE IF NOT EXISTS working_orders (id TEXT PRIMARY KEY, created_at TEXT NOT NULL, json TEXT NOT NULL);
            -- 口座残高が動いた出来事（決済・入出金）。cTrader の履歴から取り込む。時刻は epoch 秒
            CREATE TABLE IF NOT EXISTS balance_history (
                account_id INTEGER NOT NULL,
                id TEXT NOT NULL,
                ts INTEGER NOT NULL,
                version INTEGER,
                balance REAL NOT NULL,
                PRIMARY KEY (account_id, id)
            );
            CREATE INDEX IF NOT EXISTS idx_balance_history_ts ON balance_history (account_id, ts);
            "#,
        )?;

        // LLM 時代の列名から移す（判断ログ ID → シグナル ID、無効化の根拠 → メモ）
        self.rename_column("positions", "cot_log_id", "signal_id")?;
        self.rename_column("positions", "invalidation_reason", "note")?;
        self.rename_column("trades", "cot_log_id", "signal_id")?;
        self.add_column("positions", "strategy", "TEXT")?;
        self.add_column("trades", "strategy", "TEXT")?;
        self.conn.execute_batch(
            "DROP INDEX IF EXISTS idx_trades_cot_log_id;
             CREATE INDEX IF NOT EXISTS idx_trades_signal_id ON trades (signal_id);
             CREATE INDEX IF NOT EXISTS idx_trades_strategy ON trades (strategy, close_time);",
        )?;
        Ok(())
    }

    // -------------------------------------------------------------- signals

    pub fn insert_signal(&self, s: &SignalLog) -> Result<()> {
        self.conn.execute(
            &format!("INSERT OR REPLACE INTO signals ({SIGNAL_COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"),
            params![
                s.id,
                s.timestamp,
                s.deployment,
                s.strategy,
                s.symbol,
                s.action,
                s.side,
                s.reason,
                serde_json::to_string(&s.detail)?,
                s.guard_result,
                s.status.as_str(),
                s.message,
                s.order_id,
                s.dry_run
            ],
        )?;
        Ok(())
    }

    /// 新しい順
    pub fn signals(&self, q: &SignalQuery) -> Result<Page<SignalLog>> {
        let filters = named_params! {
            ":symbol": non_empty(q.symbol.as_ref()),
            ":deployment": non_empty(q.deployment.as_ref()),
            ":status": non_empty(q.status.as_ref()).map(|s| s.to_uppercase()),
            ":q": search_pattern(q.q.as_deref()),
        };
        let total = self.conn.query_row(&format!("SELECT COUNT(*) FROM signals {SIGNAL_SEARCH_WHERE}"), filters, |r| r.get(0))?;
        let (limit, offset) = (q.limit(), q.offset());
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {SIGNAL_COLS} FROM signals {SIGNAL_SEARCH_WHERE}
             ORDER BY timestamp DESC, rowid DESC LIMIT {limit} OFFSET {offset}"
        ))?;
        let items = stmt.query_map(filters, row_to_signal)?.collect::<std::result::Result<_, _>>()?;
        Ok(Page { items, total })
    }

    pub fn signal(&self, id: &str) -> Result<Option<SignalLog>> {
        Ok(self
            .conn
            .query_row(&format!("SELECT {SIGNAL_COLS} FROM signals WHERE id = ?1"), params![id], row_to_signal)
            .optional()?)
    }

    // ------------------------------------------------------------ positions

    /// 保有ポジションを丸ごと置き換える（件数が少なく、毎サイクル含み損益が変わるため）
    pub fn replace_positions(&mut self, positions: &[Position]) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM positions", [])?;
        {
            let mut stmt = tx.prepare(&format!(
                "INSERT INTO positions ({POSITION_COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"
            ))?;
            for p in positions {
                stmt.execute(params![
                    p.id,
                    p.symbol,
                    p.side,
                    p.volume_lots,
                    p.entry_price,
                    p.current_price,
                    p.stop_loss,
                    p.take_profit,
                    p.pnl_pips,
                    p.pnl_amount,
                    p.open_time,
                    p.note,
                    p.signal_id,
                    p.strategy
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn replace_working_orders(&mut self, orders: &[WorkingOrder]) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM working_orders", [])?;
        {
            let mut stmt = tx.prepare("INSERT INTO working_orders (id, created_at, json) VALUES (?1, ?2, ?3)")?;
            for o in orders {
                stmt.execute(params![o.id, o.created_at, serde_json::to_string(o)?])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// 予約注文の古い順
    pub fn working_orders(&self) -> Result<Vec<WorkingOrder>> {
        let mut stmt = self.conn.prepare("SELECT json FROM working_orders ORDER BY created_at ASC, rowid ASC")?;
        let rows: Vec<String> = stmt.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
        rows.iter().map(|j| Ok(serde_json::from_str(j)?)).collect()
    }

    /// 建玉の古い順
    pub fn positions(&self) -> Result<Vec<Position>> {
        let mut stmt = self.conn.prepare(&format!("SELECT {POSITION_COLS} FROM positions ORDER BY open_time ASC, rowid ASC"))?;
        let rows = stmt.query_map([], row_to_position)?.collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    // --------------------------------------------------------------- trades

    pub fn insert_trade(&self, t: &TradeHistory) -> Result<()> {
        self.conn.execute(
            &format!("INSERT OR REPLACE INTO trades ({TRADE_COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)"),
            params![
                t.id,
                t.symbol,
                t.side,
                t.volume_lots,
                t.entry_price,
                t.close_price,
                t.stop_loss,
                t.take_profit,
                t.pnl_pips,
                t.pnl_amount,
                close_reason_str(&t.close_reason),
                t.open_time,
                t.close_time,
                t.signal_id,
                t.strategy
            ],
        )?;
        Ok(())
    }

    /// 銘柄・戦略・売買・決済理由・勝敗・期間・損益幅・フリーワードで絞り込んだ決済履歴
    pub fn search_trades(&self, q: &TradeQuery) -> Result<Page<TradeHistory>> {
        let symbol = non_empty(q.symbol.as_ref());
        let side = non_empty(q.side.as_ref()).map(|s| s.to_uppercase());
        let close_reason = q.close_reason.as_ref().map(close_reason_str);
        let signal_id = non_empty(q.signal_id.as_ref());
        let strategy = non_empty(q.strategy.as_ref());
        let pattern = search_pattern(q.q.as_deref());
        let from = non_empty(q.from.as_ref());
        let to = range_end(q.to.as_ref());

        // 絞り込み条件は件数・本体の双方で同じものを使う
        let filters = named_params! {
            ":signal_id": signal_id,
            ":symbol": symbol,
            ":strategy": strategy,
            ":side": side,
            ":close_reason": close_reason,
            ":win": q.win_only(),
            ":from": from,
            ":to": to,
            ":min_pnl": q.min_pnl_pips,
            ":max_pnl": q.max_pnl_pips,
            ":q": pattern,
        };

        let total = self
            .conn
            .query_row(&format!("SELECT COUNT(*) FROM trades {TRADE_SEARCH_WHERE}"), filters, |r| r.get(0))?;

        // 並び順・件数は列挙型と数値から組み立てるので、SQL へ直接埋め込んでも注入の余地はない
        let (col, dir) = (q.sort.column(), if q.asc { "ASC" } else { "DESC" });
        let (limit, offset) = (q.limit(), q.offset());
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {TRADE_COLS} FROM trades {TRADE_SEARCH_WHERE}
             ORDER BY {col} {dir}, rowid {dir} LIMIT {limit} OFFSET {offset}"
        ))?;
        let items = stmt.query_map(filters, row_to_trade)?.collect::<std::result::Result<_, _>>()?;
        Ok(Page { items, total })
    }

    /// `since`（"YYYY-MM-DD HH:MM:SS"）以降に決済した取引の損益合計（口座通貨）
    pub fn realized_pnl_since(&self, since: &str) -> Result<f64> {
        Ok(self
            .conn
            .query_row("SELECT COALESCE(SUM(pnl_amount), 0) FROM trades WHERE close_time >= ?1", params![since], |r| r.get(0))?)
    }

    /// 決済履歴を1件削除。削除できたら true
    pub fn delete_trade(&self, id: &str) -> Result<bool> {
        Ok(self.conn.execute("DELETE FROM trades WHERE id = ?1", params![id])? > 0)
    }

    /// 決済履歴をまとめて削除。削除できた件数を返す
    pub fn delete_trades(&mut self, ids: &[String]) -> Result<usize> {
        let tx = self.conn.transaction()?;
        let mut deleted = 0;
        {
            let mut stmt = tx.prepare("DELETE FROM trades WHERE id = ?1")?;
            for id in ids {
                deleted += stmt.execute(params![id])?;
            }
        }
        tx.commit()?;
        Ok(deleted)
    }

    pub fn trade(&self, id: &str) -> Result<Option<TradeHistory>> {
        Ok(self
            .conn
            .query_row(&format!("SELECT {TRADE_COLS} FROM trades WHERE id = ?1"), params![id], row_to_trade)
            .optional()?)
    }

    // ------------------------------------------------------ balance_history

    /// 取り込み済みの出来事は上書きする（同じ期間を取り直しても重複しない）
    pub fn insert_balance_events(&mut self, account_id: i64, events: &[BalanceEvent]) -> Result<()> {
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO balance_history (account_id, id, ts, version, balance) VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for e in events {
                stmt.execute(params![account_id, e.id, e.time.timestamp(), e.version, e.balance])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// 取り込み済みの最新の出来事の時刻（epoch 秒）
    pub fn latest_balance_ts(&self, account_id: i64) -> Result<Option<i64>> {
        Ok(self
            .conn
            .query_row("SELECT MAX(ts) FROM balance_history WHERE account_id = ?1", params![account_id], |r| r.get(0))?)
    }

    /// `from`（epoch 秒）以降の残高の推移を古い順に返す。期間の頭から線を引けるよう、
    /// それより前の最後の残高を `from` 時点の点として先頭に置く
    pub fn balance_history(&self, account_id: i64, from: Option<i64>) -> Result<Vec<BalancePoint>> {
        let point = |ts: i64, balance: f64| BalancePoint { time: super::fmt_ts(ts), balance };
        let mut points = Vec::new();
        if let Some(from) = from {
            let before: Option<f64> = self
                .conn
                .query_row(
                    "SELECT balance FROM balance_history WHERE account_id = ?1 AND ts < ?2
                     ORDER BY ts DESC, version DESC LIMIT 1",
                    params![account_id, from],
                    |r| r.get(0),
                )
                .optional()?;
            points.extend(before.map(|b| point(from, b)));
        }
        let mut stmt = self.conn.prepare(
            "SELECT ts, balance FROM balance_history WHERE account_id = ?1 AND (?2 IS NULL OR ts >= ?2)
             ORDER BY ts, version",
        )?;
        for row in stmt.query_map(params![account_id, from], |r| Ok(point(r.get(0)?, r.get(1)?)))? {
            points.push(row?);
        }
        Ok(points)
    }
}

fn close_reason_str(r: &CloseReason) -> String {
    serde_json::to_value(r).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default()
}

fn parse_close_reason(s: &str) -> rusqlite::Result<CloseReason> {
    serde_json::from_value(serde_json::Value::String(s.to_string()))
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(10, rusqlite::types::Type::Text, Box::new(e)))
}

fn row_to_signal(r: &Row) -> rusqlite::Result<SignalLog> {
    let detail: String = r.get(8)?;
    let status: String = r.get(10)?;
    Ok(SignalLog {
        id: r.get(0)?,
        timestamp: r.get(1)?,
        deployment: r.get(2)?,
        strategy: r.get(3)?,
        symbol: r.get(4)?,
        action: r.get(5)?,
        side: r.get(6)?,
        reason: r.get(7)?,
        detail: serde_json::from_str(&detail).unwrap_or(serde_json::Value::Null),
        guard_result: r.get(9)?,
        status: SignalStatus::parse(&status),
        message: r.get(11)?,
        order_id: r.get(12)?,
        dry_run: r.get(13)?,
    })
}

fn row_to_position(r: &Row) -> rusqlite::Result<Position> {
    Ok(Position {
        id: r.get(0)?,
        symbol: r.get(1)?,
        side: r.get(2)?,
        volume_lots: r.get(3)?,
        entry_price: r.get(4)?,
        current_price: r.get(5)?,
        stop_loss: r.get(6)?,
        take_profit: r.get(7)?,
        pnl_pips: r.get(8)?,
        pnl_amount: r.get(9)?,
        open_time: r.get(10)?,
        note: r.get(11)?,
        signal_id: r.get(12)?,
        strategy: r.get(13)?,
    })
}

fn row_to_trade(r: &Row) -> rusqlite::Result<TradeHistory> {
    Ok(TradeHistory {
        id: r.get(0)?,
        symbol: r.get(1)?,
        side: r.get(2)?,
        volume_lots: r.get(3)?,
        entry_price: r.get(4)?,
        close_price: r.get(5)?,
        stop_loss: r.get(6)?,
        take_profit: r.get(7)?,
        pnl_pips: r.get(8)?,
        pnl_amount: r.get(9)?,
        close_reason: parse_close_reason(&r.get::<_, String>(10)?)?,
        open_time: r.get(11)?,
        close_time: r.get(12)?,
        signal_id: r.get(13)?,
        strategy: r.get(14)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::types::TradeSort;

    fn trade(id: &str, close_time: &str, signal_id: Option<&str>) -> TradeHistory {
        TradeHistory {
            id: id.into(),
            symbol: "USDJPY".into(),
            side: "BUY".into(),
            volume_lots: 0.1,
            entry_price: 154.2,
            close_price: 154.4,
            stop_loss: 154.1,
            take_profit: 154.4,
            pnl_pips: 20.0,
            pnl_amount: 2000.0,
            close_reason: CloseReason::TakeProfit,
            open_time: "2026-09-15 09:00:00".into(),
            close_time: close_time.into(),
            signal_id: signal_id.map(String::from),
            strategy: Some("ema-usdjpy".into()),
        }
    }

    fn signal(id: &str, ts: &str, status: SignalStatus) -> SignalLog {
        SignalLog {
            id: id.into(),
            timestamp: ts.into(),
            deployment: "ema-usdjpy".into(),
            strategy: "ema_cross M5".into(),
            symbol: "USDJPY_z".into(),
            action: "OPEN".into(),
            side: Some("BUY".into()),
            reason: "EMA20 が EMA50 を上抜け".into(),
            detail: serde_json::json!({"lots": 0.1}),
            guard_result: Some("PASS".into()),
            status,
            message: None,
            order_id: Some("123".into()),
            dry_run: false,
        }
    }

    /// LLM 時代の DB（cot_log_id / invalidation_reason 列）を開いても、列名を移して読めること
    #[test]
    fn migration_renames_llm_era_columns() {
        let db = Db::open_in_memory().unwrap();
        db.conn
            .execute_batch(
                "DROP TABLE positions; DROP TABLE trades;
                 CREATE TABLE positions (id TEXT PRIMARY KEY, symbol TEXT NOT NULL, side TEXT NOT NULL, volume_lots REAL NOT NULL,
                   entry_price REAL NOT NULL, current_price REAL NOT NULL, stop_loss REAL NOT NULL, take_profit REAL NOT NULL,
                   pnl_pips REAL NOT NULL, pnl_amount REAL NOT NULL, open_time TEXT NOT NULL, invalidation_reason TEXT NOT NULL,
                   cot_log_id TEXT, close_stop REAL);
                 CREATE TABLE trades (id TEXT PRIMARY KEY, symbol TEXT NOT NULL, side TEXT NOT NULL, volume_lots REAL NOT NULL,
                   entry_price REAL NOT NULL, close_price REAL NOT NULL, stop_loss REAL NOT NULL, take_profit REAL NOT NULL,
                   pnl_pips REAL NOT NULL, pnl_amount REAL NOT NULL, close_reason TEXT NOT NULL, open_time TEXT NOT NULL,
                   close_time TEXT NOT NULL, cot_log_id TEXT);
                 INSERT INTO trades VALUES ('trd-1','USDJPY_z','SELL',0.01,157.0,156.9,157.2,156.8,10.0,84.0,'INVALIDATED',
                   '2026-10-01 00:00:00','2026-10-01 01:00:00','cot-1');",
            )
            .unwrap();
        db.migrate_journal().unwrap();
        let t = db.trade("trd-1").unwrap().unwrap();
        assert_eq!(t.signal_id.as_deref(), Some("cot-1"));
        assert!(t.strategy.is_none());
        assert!(matches!(t.close_reason, CloseReason::Invalidated));
        assert!(db.positions().unwrap().is_empty());
        // 2回目は何もしない
        db.migrate_journal().unwrap();
    }

    #[test]
    fn signals_are_searchable_newest_first() {
        let db = Db::open_in_memory().unwrap();
        db.insert_signal(&signal("sig-1", "2026-10-20 09:00:00", SignalStatus::Executed)).unwrap();
        let mut rejected = signal("sig-2", "2026-10-20 09:05:00", SignalStatus::Rejected);
        rejected.guard_result = Some("MAX_POSITIONS".into());
        db.insert_signal(&rejected).unwrap();

        let all = db.signals(&SignalQuery::default()).unwrap();
        assert_eq!(all.total, 2);
        assert_eq!(all.items[0].id, "sig-2");
        assert_eq!(all.items[1].detail["lots"], 0.1);
        let q = SignalQuery { status: Some("rejected".into()), ..Default::default() };
        assert_eq!(db.signals(&q).unwrap().items[0].guard_result.as_deref(), Some("MAX_POSITIONS"));
        let q = SignalQuery { q: Some("上抜け".into()), deployment: Some("ema-usdjpy".into()), ..Default::default() };
        assert_eq!(db.signals(&q).unwrap().total, 2);
        assert_eq!(db.signal("sig-1").unwrap().unwrap().status, SignalStatus::Executed);
    }

    #[test]
    fn trades_search_filters_and_sorts() {
        let mut db = Db::open_in_memory().unwrap();
        let mut win = trade("trd-win", "2026-09-15 10:00:00", Some("sig-1"));
        let mut loss = trade("trd-loss", "2026-09-16 11:00:00", None);
        loss.symbol = "EURUSD".into();
        loss.side = "SELL".into();
        loss.close_reason = CloseReason::StopLoss;
        loss.pnl_pips = -12.0;
        loss.pnl_amount = -1200.0;
        loss.strategy = None;
        win.pnl_pips = 20.0;
        db.insert_trade(&win).unwrap();
        db.insert_trade(&loss).unwrap();

        fn find(db: &Db, f: impl FnOnce(&mut TradeQuery)) -> Page<TradeHistory> {
            let mut q = TradeQuery { limit: Some(10), ..Default::default() };
            f(&mut q);
            db.search_trades(&q).unwrap()
        }
        let search = |f: fn(&mut TradeQuery)| find(&db, f);

        // 既定は決済の新しい順
        assert_eq!(search(|_| {}).items[0].id, "trd-loss");
        assert_eq!(search(|q| q.symbol = Some("eurusd".into())).total, 1);
        assert_eq!(search(|q| q.strategy = Some("ema-usdjpy".into())).items[0].id, "trd-win");
        assert_eq!(search(|q| q.signal_id = Some("sig-1".into())).total, 1);
        assert_eq!(search(|q| q.side = Some("buy".into())).items[0].id, "trd-win");
        assert_eq!(search(|q| q.close_reason = Some(CloseReason::StopLoss)).items[0].id, "trd-loss");
        assert_eq!(search(|q| q.result = Some("win".into())).items[0].id, "trd-win");
        assert_eq!(search(|q| q.to = Some("2026-09-15".into())).items[0].id, "trd-win");
        assert_eq!(search(|q| q.max_pnl_pips = Some(-1.0)).items[0].id, "trd-loss");
        assert_eq!(search(|q| q.q = Some("%".into())).total, 0);
        let by_pnl_asc = search(|q| {
            q.sort = TradeSort::PnlPips;
            q.asc = true;
        });
        assert_eq!(by_pnl_asc.items[0].id, "trd-loss");
        assert_eq!(db.realized_pnl_since("2026-09-16 00:00:00").unwrap(), -1200.0);

        assert!(db.delete_trade("trd-win").unwrap());
        assert_eq!(db.delete_trades(&["trd-loss".into(), "missing".into()]).unwrap(), 1);
        assert_eq!(find(&db, |_| {}).total, 0);
    }

    #[test]
    fn positions_are_replaced() {
        let mut db = Db::open_in_memory().unwrap();
        let p = Position {
            id: "pos-1".into(),
            symbol: "USDJPY".into(),
            side: "BUY".into(),
            volume_lots: 0.1,
            entry_price: 154.2,
            current_price: 154.25,
            stop_loss: 154.1,
            take_profit: 154.4,
            pnl_pips: 5.0,
            pnl_amount: 500.0,
            open_time: "2026-09-15 09:00:00".into(),
            note: "n".into(),
            signal_id: Some("sig-1".into()),
            strategy: Some("ema-usdjpy".into()),
        };
        db.replace_positions(std::slice::from_ref(&p)).unwrap();
        let got = &db.positions().unwrap()[0];
        assert_eq!((got.signal_id.as_deref(), got.strategy.as_deref()), (Some("sig-1"), Some("ema-usdjpy")));
        db.replace_positions(&[]).unwrap();
        assert!(db.positions().unwrap().is_empty());
    }

    #[test]
    fn balance_history_starts_at_range_and_dedupes() {
        use chrono::TimeZone;
        let mut db = Db::open_in_memory().unwrap();
        let ev = |id: &str, ts: i64, version: i64, balance: f64| BalanceEvent {
            id: id.into(),
            time: chrono::Utc.timestamp_opt(ts, 0).unwrap(),
            version: Some(version),
            balance,
        };
        assert_eq!(db.latest_balance_ts(1).unwrap(), None);
        db.insert_balance_events(1, &[ev("cash-1", 100, 1, 1000.0), ev("deal-1", 200, 2, 1100.0), ev("deal-2", 200, 3, 1050.0)])
            .unwrap();
        db.insert_balance_events(1, &[ev("deal-2", 200, 3, 1050.0)]).unwrap();
        db.insert_balance_events(2, &[ev("deal-9", 150, 1, 5.0)]).unwrap();
        assert_eq!(db.latest_balance_ts(1).unwrap(), Some(200));
        let all = db.balance_history(1, None).unwrap();
        assert_eq!(all.iter().map(|p| p.balance).collect::<Vec<_>>(), vec![1000.0, 1100.0, 1050.0]);
        let recent = db.balance_history(1, Some(150)).unwrap();
        assert_eq!(recent[0], BalancePoint { time: crate::storage::fmt_ts(150), balance: 1000.0 });
        assert_eq!(recent.len(), 3);
    }
}
