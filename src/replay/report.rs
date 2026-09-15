//! リプレイ結果の集計。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::storage::ReplayDecisionRow;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Bucket {
    pub n: usize,
    pub trades: usize,
    pub tp_hit: usize,
    pub sl_hit: usize,
    pub same_bar: usize,
    pub timeout: usize,
    pub win_rate: Option<f64>,
    pub avg_pnl_pips: Option<f64>,
    pub total_pnl_pips: f64,
}

impl Bucket {
    fn add(&mut self, d: &ReplayDecisionRow) {
        self.n += 1;
        match d.outcome.as_str() {
            "TP_HIT" => { self.trades += 1; self.tp_hit += 1; }
            "SL_HIT" => { self.trades += 1; self.sl_hit += 1; }
            "SAME_BAR" => { self.trades += 1; self.same_bar += 1; }
            "TIMEOUT" => { self.trades += 1; self.timeout += 1; }
            _ => {}
        }
        if let Some(p) = d.pnl_pips {
            self.total_pnl_pips += p;
        }
    }

    fn finish(&mut self) {
        if self.trades > 0 {
            self.win_rate = Some(self.tp_hit as f64 / self.trades as f64);
            self.avg_pnl_pips = Some(self.total_pnl_pips / self.trades as f64);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub run_id: i64,
    pub total: usize,
    pub guard_pass: usize,
    pub by_action: BTreeMap<String, usize>,
    pub by_outcome: BTreeMap<String, usize>,
    pub by_guard: BTreeMap<String, usize>,
    /// ガード通過分のみ。キーは "0.6-0.7" のような確信度帯
    pub by_confidence: BTreeMap<String, Bucket>,
    pub by_session: BTreeMap<String, Bucket>,
    pub overall: Bucket,
    /// ガードで弾かれた判断を仮に執行していた場合の集計（ガードの妥当性検証用）
    pub rejected: Bucket,
    pub unobserved_rate: f64,
    pub hold_rate: f64,
    pub plan_rate: f64,
    pub plan_triggered: usize,
    pub conflicts_empty_rate: f64,
}

pub fn build_report(run_id: i64, rows: &[ReplayDecisionRow]) -> Report {
    let mut by_action = BTreeMap::new();
    let mut by_outcome = BTreeMap::new();
    let mut by_guard = BTreeMap::new();
    let mut by_confidence: BTreeMap<String, Bucket> = BTreeMap::new();
    let mut by_session: BTreeMap<String, Bucket> = BTreeMap::new();
    let mut overall = Bucket::default();
    let mut rejected = Bucket::default();
    let mut unobserved = 0;
    let mut holds = 0;
    let mut plans = 0;
    let mut plan_triggered = 0;
    let mut conflicts_empty = 0;
    let mut guard_pass = 0;

    for d in rows {
        *by_action.entry(d.action.clone()).or_insert(0) += 1;
        *by_outcome.entry(d.outcome.clone()).or_insert(0) += 1;
        *by_guard.entry(d.guard_result.clone()).or_insert(0) += 1;

        if d.guard_result.contains("OBSERVED") {
            unobserved += 1;
        }
        if d.action == "HOLD" {
            holds += 1;
        }
        let v: serde_json::Value = serde_json::from_str(&d.decision_json).unwrap_or_default();
        if v.get("conditional_plan").map(|p| !p.is_null()).unwrap_or(false) {
            plans += 1;
            if matches!(d.outcome.as_str(), "TP_HIT" | "SL_HIT" | "SAME_BAR" | "TIMEOUT") && d.action == "HOLD" {
                plan_triggered += 1;
            }
        }
        if v.pointer("/analysis/conflicts").and_then(|c| c.as_str()).map(|s| s.trim().is_empty()).unwrap_or(true) {
            conflicts_empty += 1;
        }

        if d.guard_result == "PASS" {
            guard_pass += 1;
            overall.add(d);
            let lo = ((d.confidence * 10.0).floor() / 10.0).clamp(0.0, 0.9);
            let key = format!("{:.1}-{:.1}", lo, lo + 0.1);
            by_confidence.entry(key).or_default().add(d);
            by_session.entry(d.session.clone()).or_default().add(d);
        } else {
            rejected.add(d);
        }
    }

    overall.finish();
    rejected.finish();
    by_confidence.values_mut().for_each(Bucket::finish);
    by_session.values_mut().for_each(Bucket::finish);

    let total = rows.len().max(1) as f64;
    Report {
        run_id,
        total: rows.len(),
        guard_pass,
        by_action,
        by_outcome,
        by_guard,
        by_confidence,
        by_session,
        overall,
        rejected,
        unobserved_rate: unobserved as f64 / total,
        hold_rate: holds as f64 / total,
        plan_rate: plans as f64 / total,
        plan_triggered,
        conflicts_empty_rate: conflicts_empty as f64 / total,
    }
}

/// CLI 向けのテキスト整形
pub fn format_report(r: &Report) -> String {
    let mut s = String::new();
    s.push_str(&format!("Run #{}  decisions={}  guard_pass={}\n", r.run_id, r.total, r.guard_pass));
    s.push_str(&format!(
        "hold_rate={:.2}  plan_rate={:.2}  plan_triggered={}  unobserved_rate={:.2}  conflicts_empty_rate={:.2}\n",
        r.hold_rate, r.plan_rate, r.plan_triggered, r.unobserved_rate, r.conflicts_empty_rate
    ));
    s.push_str("\n[by action]     ");
    for (k, v) in &r.by_action { s.push_str(&format!("{k}={v} ")); }
    s.push_str("\n[by outcome]    ");
    for (k, v) in &r.by_outcome { s.push_str(&format!("{k}={v} ")); }
    s.push_str("\n[by guard]      ");
    for (k, v) in &r.by_guard { s.push_str(&format!("{k}={v} ")); }
    s.push('\n');

    let row = |label: &str, b: &Bucket| {
        format!(
            "{:<12} n={:<4} trades={:<4} tp={:<4} sl={:<4} same={:<3} to={:<4} win={:<6} avg={:<8} total={:.1}\n",
            label,
            b.n,
            b.trades,
            b.tp_hit,
            b.sl_hit,
            b.same_bar,
            b.timeout,
            b.win_rate.map(|w| format!("{:.2}", w)).unwrap_or("-".into()),
            b.avg_pnl_pips.map(|w| format!("{:.1}", w)).unwrap_or("-".into()),
            b.total_pnl_pips
        )
    };
    s.push_str("\n[overall / guard PASS]\n");
    s.push_str(&row("all", &r.overall));
    s.push_str(&row("rejected", &r.rejected));
    s.push_str("\n[by confidence]\n");
    for (k, b) in &r.by_confidence { s.push_str(&row(k, b)); }
    s.push_str("\n[by session]\n");
    for (k, b) in &r.by_session { s.push_str(&row(k, b)); }
    s
}

/// 2つのレポートの差分（B - A）を主要指標だけ並べる
pub fn format_diff(a: &Report, b: &Report) -> String {
    let f = |x: Option<f64>| x.map(|v| format!("{:.2}", v)).unwrap_or("-".into());
    let mut s = String::new();
    s.push_str(&format!("{:<22} {:>10} {:>10}\n", "metric", format!("run#{}", a.run_id), format!("run#{}", b.run_id)));
    s.push_str(&format!("{:<22} {:>10} {:>10}\n", "decisions", a.total, b.total));
    s.push_str(&format!("{:<22} {:>10} {:>10}\n", "guard_pass", a.guard_pass, b.guard_pass));
    s.push_str(&format!("{:<22} {:>10} {:>10}\n", "trades", a.overall.trades, b.overall.trades));
    s.push_str(&format!("{:<22} {:>10} {:>10}\n", "win_rate", f(a.overall.win_rate), f(b.overall.win_rate)));
    s.push_str(&format!("{:<22} {:>10} {:>10}\n", "avg_pnl_pips", f(a.overall.avg_pnl_pips), f(b.overall.avg_pnl_pips)));
    s.push_str(&format!("{:<22} {:>10.1} {:>10.1}\n", "total_pnl_pips", a.overall.total_pnl_pips, b.overall.total_pnl_pips));
    s.push_str(&format!("{:<22} {:>10.2} {:>10.2}\n", "hold_rate", a.hold_rate, b.hold_rate));
    s.push_str(&format!("{:<22} {:>10.2} {:>10.2}\n", "plan_rate", a.plan_rate, b.plan_rate));
    s.push_str(&format!("{:<22} {:>10.2} {:>10.2}\n", "unobserved_rate", a.unobserved_rate, b.unobserved_rate));
    s.push_str(&format!("{:<22} {:>10.2} {:>10.2}\n", "conflicts_empty_rate", a.conflicts_empty_rate, b.conflicts_empty_rate));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(action: &str, outcome: &str, guard: &str, conf: f64, pnl: Option<f64>) -> ReplayDecisionRow {
        ReplayDecisionRow {
            run_id: 1,
            t: "2026-09-01 09:00:00".into(),
            decision_json: r#"{"analysis":{"conflicts":"x"},"conditional_plan":null}"#.into(),
            guard_result: guard.into(),
            outcome: outcome.into(),
            pnl_pips: pnl,
            bars_to_exit: None,
            chart_dir: String::new(),
            session: "LONDON".into(),
            confidence: conf,
            action: action.into(),
        }
    }

    #[test]
    fn aggregates_by_confidence_and_excludes_rejected() {
        let rows = vec![
            row("BUY", "TP_HIT", "PASS", 0.85, Some(20.0)),
            row("BUY", "SL_HIT", "PASS", 0.82, Some(-10.0)),
            row("SELL", "TP_HIT", "OBSERVED_MISMATCH", 0.9, Some(15.0)),
            row("HOLD", "HOLD", "PASS", 0.4, None),
        ];
        let r = build_report(1, &rows);
        assert_eq!(r.total, 4);
        assert_eq!(r.guard_pass, 3);
        assert_eq!(r.overall.trades, 2);
        assert_eq!(r.overall.win_rate, Some(0.5));
        assert_eq!(r.by_confidence["0.8-0.9"].trades, 2);
        assert_eq!(r.rejected.trades, 1);
        assert!((r.unobserved_rate - 0.25).abs() < 1e-9);
        assert!((r.hold_rate - 0.25).abs() < 1e-9);
        assert!(format_report(&r).contains("0.8-0.9"));
    }
}
