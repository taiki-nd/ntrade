//! 本番運用の記録: LLM 判断ログ（CoT）・保有ポジション・決済履歴・教訓。
//!
//! 判断と取引の対応は `cot_log_id` で辿る: `cot_logs.id` ← `positions.cot_log_id` / `trades.cot_log_id`、
//! 教訓は `lessons.trigger_trade_id` → `trades.id`。
//! 条件付きプランは「発注した記録（成立ログ）」と「根拠になった LLM 判断」が別ログになるため、
//! 取引は成立ログに紐づき、根拠は `cot_logs.origin_cot_log_id` で元の判断へ辿る。
//! 時刻は API と同じ "YYYY-MM-DD HH:MM:SS"（UTC）の文字列で持つ（辞書順 = 時系列順）。

use anyhow::Result;
use rusqlite::{named_params, params, OptionalExtension, Row};
use serde::Serialize;

use super::Db;
use crate::server::types::{CloseReason, CoTLog, LessonLearned, Position, TradeHistory, TradeQuery};

/// ページング結果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub items: Vec<T>,
    /// 条件に一致する全件数
    pub total: i64,
}

const COT_COLS: &str = "id, timestamp, symbol, action, confidence, entry_type, entry_price, stop_loss, take_profit,
    risk_reward_ratio, macro_context, order_flow, invalidation, conflicts, guard_result, reasoning, executed, spread_pips,
    origin_cot_log_id";

/// CoT ログの絞り込み条件（?1 = ペア / ?2 = 検索語。いずれも NULL なら無条件）
const COT_SEARCH_WHERE: &str = r#"WHERE (?1 IS NULL OR symbol = ?1)
      AND (?2 IS NULL OR lower(
            symbol || ' ' || action || ' ' || timestamp || ' ' || COALESCE(entry_type, '') || ' ' ||
            macro_context || ' ' || order_flow || ' ' || invalidation || ' ' || conflicts || ' ' ||
            COALESCE(guard_result, '') || ' ' || reasoning
          ) LIKE ?2 ESCAPE '\')"#;

/// 検索語を LIKE の部分一致パターンにする（ワイルドカードは打ち消す）。空文字なら絞り込まない
fn search_pattern(query: Option<&str>) -> Option<String> {
    let q = query.map(str::trim).filter(|q| !q.is_empty())?;
    let escaped = q.to_lowercase().replace('\\', r"\\").replace('%', r"\%").replace('_', r"\_");
    Some(format!("%{escaped}%"))
}

const POSITION_COLS: &str = "id, symbol, side, volume_lots, entry_price, current_price, stop_loss, take_profit,
    pnl_pips, pnl_amount, open_time, invalidation_reason, cot_log_id";

const TRADE_COLS: &str = "id, symbol, side, volume_lots, entry_price, close_price, stop_loss, take_profit,
    pnl_pips, pnl_amount, close_reason, open_time, close_time, cot_log_id";

const LESSON_COLS: &str = "id, created_at, symbol, rule, context, active, trigger_trade_id, category";

/// 決済履歴の絞り込み条件（NULL の名前付きパラメータは絞り込まない）
const TRADE_SEARCH_WHERE: &str = r#"WHERE (:cot_log_id IS NULL OR cot_log_id = :cot_log_id)
      AND (:symbol IS NULL OR symbol = :symbol)
      AND (:side IS NULL OR side = :side)
      AND (:close_reason IS NULL OR close_reason = :close_reason)
      AND (:win IS NULL OR :win = (pnl_pips >= 0))
      AND (:from IS NULL OR close_time >= :from)
      AND (:to IS NULL OR close_time <= :to)
      AND (:min_pnl IS NULL OR pnl_pips >= :min_pnl)
      AND (:max_pnl IS NULL OR pnl_pips <= :max_pnl)
      AND (:q IS NULL OR lower(
            symbol || ' ' || side || ' ' || close_reason || ' ' ||
            open_time || ' ' || close_time || ' ' || COALESCE(cot_log_id, '')
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
    pub(super) fn migrate_journal(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS cot_logs (
              id                TEXT PRIMARY KEY,
              timestamp         TEXT NOT NULL,
              symbol            TEXT NOT NULL,
              action            TEXT NOT NULL,
              confidence        REAL NOT NULL,
              entry_type        TEXT,
              entry_price       REAL,
              stop_loss         REAL,
              take_profit       REAL,
              risk_reward_ratio REAL,
              macro_context     TEXT NOT NULL,
              order_flow        TEXT NOT NULL,
              invalidation      TEXT NOT NULL,
              conflicts         TEXT NOT NULL,
              guard_result      TEXT,
              reasoning         TEXT NOT NULL,
              executed          INTEGER NOT NULL,
              spread_pips       REAL NOT NULL,
              origin_cot_log_id TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_cot_logs_ts ON cot_logs (timestamp);
            CREATE INDEX IF NOT EXISTS idx_cot_logs_symbol_ts ON cot_logs (symbol, timestamp);
            CREATE TABLE IF NOT EXISTS positions (
              id                  TEXT PRIMARY KEY,
              symbol              TEXT NOT NULL,
              side                TEXT NOT NULL,
              volume_lots         REAL NOT NULL,
              entry_price         REAL NOT NULL,
              current_price       REAL NOT NULL,
              stop_loss           REAL NOT NULL,
              take_profit         REAL NOT NULL,
              pnl_pips            REAL NOT NULL,
              pnl_amount          REAL NOT NULL,
              open_time           TEXT NOT NULL,
              invalidation_reason TEXT NOT NULL,
              cot_log_id          TEXT
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
              cot_log_id   TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_trades_close_time ON trades (close_time);
            CREATE INDEX IF NOT EXISTS idx_trades_cot_log_id ON trades (cot_log_id);
            CREATE TABLE IF NOT EXISTS lessons (
              id               TEXT PRIMARY KEY,
              created_at       TEXT NOT NULL,
              symbol           TEXT NOT NULL,
              rule             TEXT NOT NULL,
              context          TEXT NOT NULL,
              active           INTEGER NOT NULL,
              trigger_trade_id TEXT,
              category         TEXT NOT NULL
            );
            UPDATE cot_logs
            SET risk_reward_ratio = ABS((take_profit - entry_price) / (entry_price - stop_loss))
            WHERE risk_reward_ratio IS NULL
              AND entry_price IS NOT NULL
              AND stop_loss IS NOT NULL
              AND take_profit IS NOT NULL
              AND ABS(entry_price - stop_loss) > 0;
            "#,
        )?;

        // 既存 DB への列追加。既にあれば "duplicate column name" になるので無視する
        let _ = self.conn.execute("ALTER TABLE cot_logs ADD COLUMN origin_cot_log_id TEXT", []);

        self.conn.execute_batch(
            r#"
            CREATE INDEX IF NOT EXISTS idx_cot_logs_origin ON cot_logs (origin_cot_log_id);
            -- 成立ログは元の判断 ID を本文にだけ持っていたので、列に移す（'元の判断: ' は6文字）
            UPDATE cot_logs
            SET origin_cot_log_id =
                  replace(substr(macro_context, instr(macro_context, '元の判断: ') + 6), '）', '')
            WHERE origin_cot_log_id IS NULL
              AND substr(macro_context, instr(macro_context, '元の判断: ') + 6, 4) = 'cot-';
            "#,
        )?;
        Ok(())
    }

    // ------------------------------------------------------------- cot_logs

    pub fn insert_cot_log(&self, log: &CoTLog) -> Result<()> {
        self.conn.execute(
            &format!("INSERT OR REPLACE INTO cot_logs ({COT_COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)"),
            params![
                log.id,
                log.timestamp,
                log.symbol,
                log.action,
                log.confidence,
                log.entry_type,
                log.entry_price,
                log.stop_loss,
                log.take_profit,
                log.risk_reward_ratio,
                log.macro_context,
                log.order_flow,
                log.invalidation,
                log.conflicts,
                log.guard_result,
                log.reasoning,
                log.executed,
                log.spread_pips,
                log.origin_cot_log_id
            ],
        )?;
        Ok(())
    }

    /// 新しい順。`symbol` 指定時はそのペアのみ、`query` 指定時は本文の部分一致（大文字小文字を無視）で絞り込む
    pub fn cot_logs(&self, symbol: Option<&str>, query: Option<&str>, limit: usize, offset: usize) -> Result<Page<CoTLog>> {
        let symbol = symbol.map(str::to_uppercase);
        let query = search_pattern(query);
        let total = self.conn.query_row(
            &format!("SELECT COUNT(*) FROM cot_logs {COT_SEARCH_WHERE}"),
            params![symbol, query],
            |r| r.get(0),
        )?;
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {COT_COLS} FROM cot_logs {COT_SEARCH_WHERE}
             ORDER BY timestamp DESC, rowid DESC LIMIT ?3 OFFSET ?4"
        ))?;
        let items = stmt
            .query_map(params![symbol, query, limit as i64, offset as i64], row_to_cot)?
            .collect::<std::result::Result<_, _>>()?;
        Ok(Page { items, total })
    }

    /// この判断から生まれた成立ログ（条件付きプランが成立して発注した記録）を古い順に返す
    pub fn cot_logs_by_origin(&self, origin_cot_log_id: &str) -> Result<Vec<CoTLog>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {COT_COLS} FROM cot_logs WHERE origin_cot_log_id = ?1 ORDER BY timestamp ASC, rowid ASC"
        ))?;
        let items = stmt
            .query_map(params![origin_cot_log_id], row_to_cot)?
            .collect::<std::result::Result<_, _>>()?;
        Ok(items)
    }

    pub fn cot_log(&self, id: &str) -> Result<Option<CoTLog>> {
        Ok(self
            .conn
            .query_row(&format!("SELECT {COT_COLS} FROM cot_logs WHERE id = ?1"), params![id], row_to_cot)
            .optional()?)
    }

    // ------------------------------------------------------------ positions

    /// 保有ポジションを丸ごと置き換える（件数が少なく、毎サイクル含み損益が変わるため）
    pub fn replace_positions(&mut self, positions: &[Position]) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM positions", [])?;
        {
            let mut stmt = tx.prepare(&format!(
                "INSERT INTO positions ({POSITION_COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"
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
                    p.invalidation_reason,
                    p.cot_log_id
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
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
            &format!("INSERT OR REPLACE INTO trades ({TRADE_COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"),
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
                t.cot_log_id
            ],
        )?;
        Ok(())
    }

    /// 決済の新しい順。`cot_log_id` 指定時はその判断から生まれた取引のみ
    pub fn trades(&self, cot_log_id: Option<&str>, limit: usize, offset: usize) -> Result<Page<TradeHistory>> {
        self.search_trades(&TradeQuery::by_cot_log(cot_log_id, limit, offset))
    }

    /// 銘柄・売買・決済理由・勝敗・期間・損益幅・フリーワードで絞り込んだ決済履歴
    pub fn search_trades(&self, q: &TradeQuery) -> Result<Page<TradeHistory>> {
        let symbol = non_empty(q.symbol.as_ref()).map(|s| s.to_uppercase());
        let side = non_empty(q.side.as_ref()).map(|s| s.to_uppercase());
        let close_reason = q.close_reason.as_ref().map(close_reason_str);
        let cot_log_id = non_empty(q.cot_log_id.as_ref());
        let pattern = search_pattern(q.q.as_deref());
        let from = non_empty(q.from.as_ref());
        let to = range_end(q.to.as_ref());

        // 絞り込み条件は件数・本体の双方で同じものを使う
        let filters = named_params! {
            ":cot_log_id": cot_log_id,
            ":symbol": symbol,
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

    // -------------------------------------------------------------- lessons

    pub fn upsert_lesson(&self, l: &LessonLearned) -> Result<()> {
        self.conn.execute(
            &format!("INSERT OR REPLACE INTO lessons ({LESSON_COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"),
            params![l.id, l.created_at, l.symbol, l.rule, l.context, l.active, l.trigger_trade_id, l.category],
        )?;
        Ok(())
    }

    /// 削除できたら true
    pub fn delete_lesson(&self, id: &str) -> Result<bool> {
        Ok(self.conn.execute("DELETE FROM lessons WHERE id = ?1", params![id])? > 0)
    }

    /// 新しい順
    pub fn lessons(&self) -> Result<Vec<LessonLearned>> {
        let mut stmt = self.conn.prepare(&format!("SELECT {LESSON_COLS} FROM lessons ORDER BY created_at DESC, rowid DESC"))?;
        let rows = stmt.query_map([], row_to_lesson)?.collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    pub fn lesson(&self, id: &str) -> Result<Option<LessonLearned>> {
        Ok(self
            .conn
            .query_row(&format!("SELECT {LESSON_COLS} FROM lessons WHERE id = ?1"), params![id], row_to_lesson)
            .optional()?)
    }
}

fn close_reason_str(r: &CloseReason) -> String {
    serde_json::to_value(r).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default()
}

fn parse_close_reason(s: &str) -> rusqlite::Result<CloseReason> {
    serde_json::from_value(serde_json::Value::String(s.to_string()))
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(10, rusqlite::types::Type::Text, Box::new(e)))
}

fn row_to_cot(r: &Row) -> rusqlite::Result<CoTLog> {
    Ok(CoTLog {
        id: r.get(0)?,
        timestamp: r.get(1)?,
        symbol: r.get(2)?,
        action: r.get(3)?,
        confidence: r.get(4)?,
        entry_type: r.get(5)?,
        entry_price: r.get(6)?,
        stop_loss: r.get(7)?,
        take_profit: r.get(8)?,
        risk_reward_ratio: r.get(9)?,
        macro_context: r.get(10)?,
        order_flow: r.get(11)?,
        invalidation: r.get(12)?,
        conflicts: r.get(13)?,
        guard_result: r.get(14)?,
        reasoning: r.get(15)?,
        executed: r.get(16)?,
        spread_pips: r.get(17)?,
        origin_cot_log_id: r.get(18)?,
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
        invalidation_reason: r.get(11)?,
        cot_log_id: r.get(12)?,
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
        cot_log_id: r.get(13)?,
    })
}

fn row_to_lesson(r: &Row) -> rusqlite::Result<LessonLearned> {
    Ok(LessonLearned {
        id: r.get(0)?,
        created_at: r.get(1)?,
        symbol: r.get(2)?,
        rule: r.get(3)?,
        context: r.get(4)?,
        active: r.get(5)?,
        trigger_trade_id: r.get(6)?,
        category: r.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::types::TradeSort;

    fn cot(id: &str, ts: &str, symbol: &str) -> CoTLog {
        CoTLog {
            id: id.into(),
            timestamp: ts.into(),
            symbol: symbol.into(),
            action: "BUY".into(),
            confidence: 0.8,
            entry_type: Some("MARKET".into()),
            entry_price: Some(154.2),
            stop_loss: Some(154.1),
            take_profit: None,
            risk_reward_ratio: None,
            macro_context: "m".into(),
            order_flow: "o".into(),
            invalidation: "i".into(),
            conflicts: String::new(),
            guard_result: None,
            reasoning: "r".into(),
            executed: true,
            spread_pips: 0.3,
            origin_cot_log_id: None,
        }
    }

    fn trade(id: &str, close_time: &str, cot_log_id: Option<&str>) -> TradeHistory {
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
            cot_log_id: cot_log_id.map(String::from),
        }
    }

    /// 成立ログは元の判断 ID を本文にしか持っていなかった。移行でその ID を列へ移せること
    #[test]
    fn migration_backfills_origin_from_plan_log_text() {
        let db = Db::open_in_memory().unwrap();
        let mut plan = cot("cot-plan-1", "2026-09-25 10:35:22", "USDJPY");
        plan.macro_context = "条件付きプラン plan-1790332320282 が成立（元の判断: cot-1790332320282）".into();
        db.insert_cot_log(&plan).unwrap();
        // 元の判断が無いものは埋めない
        let mut orphan = cot("cot-plan-2", "2026-09-25 10:40:00", "USDJPY");
        orphan.macro_context = "条件付きプラン plan-x が成立（元の判断: 不明）".into();
        db.insert_cot_log(&orphan).unwrap();

        db.migrate_journal().unwrap();

        let filled = db.cot_log("cot-plan-1").unwrap().unwrap();
        assert_eq!(filled.origin_cot_log_id.as_deref(), Some("cot-1790332320282"));
        assert!(db.cot_log("cot-plan-2").unwrap().unwrap().origin_cot_log_id.is_none());
        let children = db.cot_logs_by_origin("cot-1790332320282").unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].id, "cot-plan-1");
    }

    #[test]
    fn cot_logs_paginate_newest_first() {
        let db = Db::open_in_memory().unwrap();
        for i in 0..5 {
            db.insert_cot_log(&cot(&format!("cot-{i}"), &format!("2026-09-15 09:0{i}:00"), "USDJPY")).unwrap();
        }
        db.insert_cot_log(&cot("cot-e", "2026-09-15 09:09:00", "EURUSD")).unwrap();

        let page = db.cot_logs(None, None, 2, 0).unwrap();
        assert_eq!(page.total, 6);
        assert_eq!(page.items.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), ["cot-e", "cot-4"]);

        let page = db.cot_logs(Some("usdjpy"), None, 2, 3).unwrap();
        assert_eq!(page.total, 5);
        assert_eq!(page.items.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), ["cot-1", "cot-0"]);

        let got = db.cot_log("cot-2").unwrap().unwrap();
        assert_eq!(got.entry_type.as_deref(), Some("MARKET"));
        assert!(got.executed);
        assert!(db.cot_log("missing").unwrap().is_none());
    }

    #[test]
    fn cot_logs_search_matches_body_and_ignores_wildcards() {
        let db = Db::open_in_memory().unwrap();
        let mut hit = cot("cot-hit", "2026-09-15 09:00:00", "USDJPY");
        hit.reasoning = "レンジ上限を Break した".into();
        let mut miss = cot("cot-miss", "2026-09-15 09:01:00", "EURUSD");
        miss.reasoning = "様子見".into();
        miss.action = "HOLD".into();
        db.insert_cot_log(&hit).unwrap();
        db.insert_cot_log(&miss).unwrap();

        // 本文の部分一致（英字は大文字小文字を無視）
        let page = db.cot_logs(None, Some(" break "), 10, 0).unwrap();
        assert_eq!((page.total, page.items[0].id.as_str()), (1, "cot-hit"));
        // 日本語・アクション・ペアでも引ける
        assert_eq!(db.cot_logs(None, Some("レンジ"), 10, 0).unwrap().total, 1);
        assert_eq!(db.cot_logs(None, Some("HOLD"), 10, 0).unwrap().total, 1);
        assert_eq!(db.cot_logs(Some("USDJPY"), Some("様子見"), 10, 0).unwrap().total, 0);
        // 空文字は絞り込まない / ワイルドカードはリテラル扱い
        assert_eq!(db.cot_logs(None, Some("   "), 10, 0).unwrap().total, 2);
        assert_eq!(db.cot_logs(None, Some("%"), 10, 0).unwrap().total, 0);
    }

    #[test]
    fn trades_link_to_decision() {
        let db = Db::open_in_memory().unwrap();
        db.insert_trade(&trade("trd-1", "2026-09-15 10:00:00", Some("cot-1"))).unwrap();
        db.insert_trade(&trade("trd-2", "2026-09-15 11:00:00", None)).unwrap();

        let all = db.trades(None, 10, 0).unwrap();
        assert_eq!(all.total, 2);
        assert_eq!(all.items[0].id, "trd-2");

        let linked = db.trades(Some("cot-1"), 10, 0).unwrap();
        assert_eq!(linked.total, 1);
        assert!(matches!(linked.items[0].close_reason, CloseReason::TakeProfit));
        assert_eq!(db.trade("trd-1").unwrap().unwrap().cot_log_id.as_deref(), Some("cot-1"));
    }

    #[test]
    fn trades_search_filters_and_sorts() {
        let mut db = Db::open_in_memory().unwrap();
        let mut win = trade("trd-win", "2026-09-15 10:00:00", Some("cot-1"));
        let mut loss = trade("trd-loss", "2026-09-16 11:00:00", None);
        loss.symbol = "EURUSD".into();
        loss.side = "SELL".into();
        loss.close_reason = CloseReason::StopLoss;
        loss.pnl_pips = -12.0;
        loss.pnl_amount = -1200.0;
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
        // 銘柄・売買・決済理由・勝敗
        assert_eq!(search(|q| q.symbol = Some("eurusd".into())).total, 1);
        assert_eq!(search(|q| q.side = Some("buy".into())).items[0].id, "trd-win");
        assert_eq!(search(|q| q.close_reason = Some(CloseReason::StopLoss)).items[0].id, "trd-loss");
        assert_eq!(search(|q| q.result = Some("win".into())).items[0].id, "trd-win");
        assert_eq!(search(|q| q.result = Some("loss".into())).total, 1);
        // 期間（上限は日付だけでもその日いっぱいを含む）と損益幅
        assert_eq!(search(|q| q.to = Some("2026-09-15".into())).items[0].id, "trd-win");
        assert_eq!(search(|q| q.from = Some("2026-09-16".into())).items[0].id, "trd-loss");
        assert_eq!(search(|q| q.min_pnl_pips = Some(0.0)).total, 1);
        assert_eq!(search(|q| q.max_pnl_pips = Some(-1.0)).items[0].id, "trd-loss");
        // フリーワード（空文字は絞り込まない / ワイルドカードはリテラル扱い）
        assert_eq!(search(|q| q.q = Some("stop_loss".into())).items[0].id, "trd-loss");
        assert_eq!(search(|q| q.q = Some("cot-1".into())).items[0].id, "trd-win");
        assert_eq!(search(|q| q.q = Some("  ".into())).total, 2);
        assert_eq!(search(|q| q.q = Some("%".into())).total, 0);
        // 並び替え
        let by_pnl_asc = search(|q| {
            q.sort = TradeSort::PnlPips;
            q.asc = true;
        });
        assert_eq!(by_pnl_asc.items[0].id, "trd-loss");

        // 削除しても件数・結果に残らない
        assert!(db.delete_trade("trd-win").unwrap());
        assert!(!db.delete_trade("trd-win").unwrap());
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
            invalidation_reason: "i".into(),
            cot_log_id: Some("cot-1".into()),
        };
        db.replace_positions(std::slice::from_ref(&p)).unwrap();
        assert_eq!(db.positions().unwrap()[0].cot_log_id.as_deref(), Some("cot-1"));
        db.replace_positions(&[]).unwrap();
        assert!(db.positions().unwrap().is_empty());
    }

    #[test]
    fn lessons_crud() {
        let db = Db::open_in_memory().unwrap();
        let mut l = LessonLearned {
            id: "les-1".into(),
            created_at: "2026-09-15 09:00:00".into(),
            symbol: "USDJPY".into(),
            rule: "r".into(),
            context: "c".into(),
            active: false,
            trigger_trade_id: Some("trd-1".into()),
            category: "TIMING".into(),
        };
        db.upsert_lesson(&l).unwrap();
        l.active = true;
        db.upsert_lesson(&l).unwrap();
        let all = db.lessons().unwrap();
        assert_eq!(all.len(), 1);
        assert!(all[0].active);
        assert!(db.delete_lesson("les-1").unwrap());
        assert!(!db.delete_lesson("les-1").unwrap());
    }
}
