//! 米国株の現物時間（NY 9:30〜16:00）を 30 分足から日ごとに切り出す。株価指数 CFD の戦略が使う。
//!
//! 30 分足は NY の :00 / :30 に揃っている（UTC との時差は整数時間）ので、9:30〜15:30 に始まる 13 本が 1 日分。
//! `closes[j]` は 9:30 + 30j 分に始まる足の終値 = 10:00 + 30j 分の価格。`closes[12]` が 16:00 の引け。

use chrono::{DateTime, NaiveDate, Timelike, Utc};
use chrono_tz::America::New_York;

use crate::ctrader::CandleBar;

/// 現物時間の 30 分足の本数（9:30〜15:30 に始まる足）
pub const SLOTS: usize = 13;
/// 1 日あたりの 30 分足のおおよその本数（CFD はほぼ 24 時間動く）。lookback の見積もりに使う
pub const BARS_PER_DAY: usize = 48;

/// 1 日分の現物時間
#[derive(Debug, Clone, PartialEq)]
pub struct CashDay {
    pub date: NaiveDate,
    /// 9:30 の始値
    pub open: f64,
    /// 各足の終値（`closes[j]` = 10:00 + 30j 分の価格）。足が無ければ None
    pub closes: [Option<f64>; SLOTS],
}

impl CashDay {
    /// 13 本そろっている（休場・短縮取引・欠けの無い）日か
    pub fn complete(&self) -> bool {
        self.closes.iter().all(Option::is_some)
    }

    /// 16:00 の引け
    pub fn close(&self) -> Option<f64> {
        self.closes[SLOTS - 1]
    }
}

/// 時刻が現物時間の何本目の足の終わりか（10:00 → 0, 10:30 → 1, ..., 16:00 → 12）。それ以外は None
pub fn slot_ending_at(t: DateTime<Utc>) -> Option<usize> {
    let ny = t.with_timezone(&New_York);
    let min = (ny.hour() * 60 + ny.minute()) as usize;
    (ny.second() == 0 && (600..=960).contains(&min) && min % 30 == 0).then(|| (min - 600) / 30)
}

/// 時刻の NY での日付
pub fn ny_date(t: DateTime<Utc>) -> NaiveDate {
    t.with_timezone(&New_York).date_naive()
}

/// 30 分足を NY の日ごとの現物時間にまとめる（古い順）。9:30 の足が無い日は入れない
pub fn cash_days(bars: &[CandleBar]) -> Vec<CashDay> {
    let mut days: Vec<CashDay> = Vec::new();
    for b in bars {
        let ny = b.timestamp.with_timezone(&New_York);
        let min = (ny.hour() * 60 + ny.minute()) as usize;
        if ny.second() != 0 || !(570..=930).contains(&min) || min % 30 != 0 {
            continue;
        }
        let slot = (min - 570) / 30;
        let date = ny.date_naive();
        if slot == 0 {
            days.push(CashDay { date, open: b.open, closes: [None; SLOTS] });
        }
        if let Some(day) = days.last_mut().filter(|d| d.date == date) {
            day.closes[slot] = Some(b.close);
        }
    }
    days
}

#[cfg(test)]
pub(crate) mod test_bars {
    use super::*;
    use chrono::{Duration, TimeZone};

    /// NY の `date` の現物時間の 30 分足 13 本と、前後の時間外の足。`path[j]` が 10:00 + 30j 分の価格
    pub fn day(date: NaiveDate, open: f64, path: &[f64; SLOTS]) -> Vec<CandleBar> {
        let start = New_York.from_local_datetime(&date.and_hms_opt(8, 0, 0).unwrap()).unwrap().with_timezone(&Utc);
        let mut out = Vec::new();
        let mut prev = open;
        // 8:00〜9:30 は始値のまま
        for i in 0..3 {
            out.push(CandleBar { timestamp: start + Duration::minutes(30 * i), open, high: open, low: open, close: open, volume: 1 });
        }
        for (j, &c) in path.iter().enumerate() {
            let t = start + Duration::minutes(90 + 30 * j as i64);
            out.push(CandleBar { timestamp: t, open: prev, high: prev.max(c), low: prev.min(c), close: c, volume: 1 });
            prev = c;
        }
        // 16:00〜17:00 は引けのまま
        for i in 0..2 {
            let t = start + Duration::minutes(480 + 30 * i);
            out.push(CandleBar { timestamp: t, open: prev, high: prev, low: prev, close: prev, volume: 1 });
        }
        out
    }

    /// 平日だけを `n` 日、`from` から並べる
    pub fn weekdays(from: NaiveDate, n: usize) -> Vec<NaiveDate> {
        use chrono::Datelike;
        let mut out = Vec::new();
        let mut d = from;
        while out.len() < n {
            if d.weekday().number_from_monday() <= 5 {
                out.push(d);
            }
            d += Duration::days(1);
        }
        out
    }

    /// その日の `slot` 本目の足が確定した時刻（UTC）
    pub fn slot_close(date: NaiveDate, slot: usize) -> DateTime<Utc> {
        let t = date.and_hms_opt(10, 0, 0).unwrap() + Duration::minutes(30 * slot as i64);
        New_York.from_local_datetime(&t).unwrap().with_timezone(&Utc)
    }
}

#[cfg(test)]
mod tests {
    use super::test_bars::*;
    use super::*;

    #[test]
    fn splits_cash_hours_by_ny_date_across_dst() {
        // 夏時間（7 月）と冬時間（1 月）で同じように切り出せる
        for date in [NaiveDate::from_ymd_opt(2025, 7, 15).unwrap(), NaiveDate::from_ymd_opt(2025, 1, 15).unwrap()] {
            let path: [f64; SLOTS] = std::array::from_fn(|j| 101.0 + j as f64);
            let days = cash_days(&day(date, 100.0, &path));
            assert_eq!(days.len(), 1);
            assert_eq!(days[0].date, date);
            assert_eq!(days[0].open, 100.0);
            assert!(days[0].complete());
            assert_eq!(days[0].closes[0], Some(101.0));
            assert_eq!(days[0].close(), Some(113.0));
            assert_eq!(slot_ending_at(slot_close(date, 0)), Some(0));
            assert_eq!(slot_ending_at(slot_close(date, 12)), Some(12));
            assert_eq!(slot_ending_at(slot_close(date, 13)), None);
        }
    }
}
