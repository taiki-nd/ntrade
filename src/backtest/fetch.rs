//! 過去の足の取得: cTrader から DB に無い期間だけを取り、区切りごとに `bars_history` に保存する。
//! CLI（`backtest fetch`）と管理画面のジョブが同じ手順で取る。

use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::Ordering;

use super::optimize::Progress;
use crate::ctrader::{BarPeriod, CTraderService};
use crate::storage::Db;

/// 既定で取る時間足
pub const DEFAULT_PERIODS: [BarPeriod; 6] = [BarPeriod::M1, BarPeriod::M5, BarPeriod::M15, BarPeriod::H1, BarPeriod::H4, BarPeriod::D1];

/// 取得の指定（API のリクエスト）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchRequest {
    pub pairs: Vec<String>,
    pub from: DateTime<Utc>,
    /// 省略時は現在
    #[serde(default)]
    pub to: Option<DateTime<Utc>>,
    /// 省略時は `DEFAULT_PERIODS`
    #[serde(default)]
    pub periods: Vec<BarPeriod>,
}

/// 1 回の要求で取る区切り
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub pair: String,
    pub period: BarPeriod,
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
}

impl Segment {
    pub fn label(&self) -> String {
        format!("{} {} {} .. {}", self.pair, self.period.as_str(), self.from.format("%Y-%m-%d"), self.to.format("%Y-%m-%d"))
    }
}

/// DB に無い前側・後ろ側の期間を、1 分足は 30 日・他は 180 日ごとに区切る
pub fn plan(db: &Db, pairs: &[String], from: DateTime<Utc>, to: DateTime<Utc>, periods: &[BarPeriod]) -> Result<Vec<Segment>> {
    if from >= to {
        bail!("from ({from}) must be before to ({to})");
    }
    if pairs.is_empty() {
        bail!("銘柄を 1 つ以上指定してください");
    }
    let mut out = Vec::new();
    for pair in pairs {
        for &period in periods {
            let mut ranges = Vec::new();
            match (db.first_bar_ts(pair, period)?, db.last_bar_ts(pair, period)?) {
                (Some(first), Some(last)) => {
                    if from < first {
                        ranges.push((from, first));
                    }
                    if last.max(from) < to {
                        ranges.push((last.max(from), to));
                    }
                }
                _ => ranges.push((from, to)),
            }
            let step = Duration::days(if period == BarPeriod::M1 { 30 } else { 180 });
            for (a, b) in ranges {
                let mut cursor = a;
                while cursor < b {
                    let end = (cursor + step).min(b);
                    out.push(Segment { pair: pair.clone(), period, from: cursor, to: end });
                    cursor = end;
                }
            }
        }
    }
    Ok(out)
}

/// 取得して保存する。`on_segment` は区切りを 1 つ保存するたびに（区切り, 保存した本数）で呼ばれる。
/// 中止されたら、保存し終えた区切りまでで止める（続きは次回 DB に無い期間として取る）。
/// DB は書き込みのたびに開く（接続を持ったまま cTrader の応答を待たないため）
pub async fn fetch(
    service: &CTraderService,
    db_path: &Path,
    req: &FetchRequest,
    progress: &Progress,
    mut on_segment: impl FnMut(&Segment, usize),
) -> Result<usize> {
    let periods = if req.periods.is_empty() { DEFAULT_PERIODS.to_vec() } else { req.periods.clone() };
    let to = req.to.unwrap_or_else(Utc::now);
    // 銘柄名はブローカーの表記にそろえ、銘柄仕様（lot サイズ・pip 位置）も保存しておく（ロット計算に使う）
    let mut pairs = Vec::new();
    for p in &req.pairs {
        let name = service.canonical_symbol(p).await.map_err(|e| anyhow!("{e:#}"))?;
        let spec = service.symbol_spec(&name).await?;
        Db::open(db_path)?.save_instrument(&spec)?;
        pairs.push(name);
    }
    let segments = plan(&Db::open(db_path)?, &pairs, req.from, to, &periods)?;
    progress.total.store(segments.len(), Ordering::Relaxed);
    let mut stored = 0;
    for seg in &segments {
        if progress.is_cancelled() {
            bail!("cancelled");
        }
        let bars = service.get_trendbars_range(&seg.pair, seg.period, seg.from, seg.to).await?;
        let n = Db::open(db_path)?.insert_bars(&seg.pair, seg.period, &bars)?;
        stored += n;
        progress.done.fetch_add(1, Ordering::Relaxed);
        on_segment(seg, n);
    }
    Ok(stored)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctrader::CandleBar;
    use chrono::TimeZone;

    #[test]
    fn plan_only_covers_missing_ranges() {
        let mut db = Db::open_in_memory().unwrap();
        let t = |m, d| Utc.with_ymd_and_hms(2026, m, d, 0, 0, 0).unwrap();
        let bar = |ts| CandleBar { timestamp: ts, open: 1.0, high: 1.0, low: 1.0, close: 1.0, volume: 1 };
        db.insert_bars("USDJPY_z", BarPeriod::M1, &[bar(t(3, 1)), bar(t(4, 1))]).unwrap();
        let pairs = vec!["USDJPY_z".to_string()];
        let segs = plan(&db, &pairs, t(1, 1), t(5, 1), &[BarPeriod::M1, BarPeriod::H1]).unwrap();
        let m1: Vec<&Segment> = segs.iter().filter(|s| s.period == BarPeriod::M1).collect();
        // 前側 1/1〜3/1 を 30 日ずつ、後ろ側 4/1〜5/1
        assert_eq!(m1.first().unwrap().from, t(1, 1));
        assert!(m1.iter().all(|s| s.to - s.from <= Duration::days(30)));
        assert!(!m1.iter().any(|s| s.from >= t(3, 1) && s.to <= t(4, 1)));
        assert_eq!(m1.last().unwrap().to, t(5, 1));
        // DB に無い時間足は全期間を 180 日ずつ
        let h1: Vec<&Segment> = segs.iter().filter(|s| s.period == BarPeriod::H1).collect();
        assert_eq!(h1.len(), 1);
        assert!(plan(&db, &pairs, t(5, 1), t(1, 1), &[BarPeriod::M1]).is_err());
    }
}
