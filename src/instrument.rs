//! 銘柄仕様のレジストリ（ブローカーから取得した pip 位置・桁数・ロットサイズ・発注量の刻み・取引時間）。
//!
//! FX・貴金属・株価指数・エネルギーで pip の定義も 1 lot の数量も違うため、銘柄名から推測せず
//! cTrader の ProtoOASymbol を正とする。取得は `CTraderService::symbol_spec` が行い、ここに登録する。
//! 未登録（未接続のリプレイなど）の銘柄だけ、名前からの推測にフォールバックする。

use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};

/// 取引時間の区間（週の始まり＝日曜 00:00 からの秒数。`time_zone` の現地時刻）
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WeeklyInterval {
    pub start_second: u32,
    pub end_second: u32,
}

/// ブローカーの休場日
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Holiday {
    pub name: String,
    /// 1970-01-01 からの日数
    pub date_days: i64,
    /// 毎年同じ月日に繰り返す
    pub recurring: bool,
    /// その日の中の休場区間（秒）。無ければ終日
    pub start_second: Option<i32>,
    pub end_second: Option<i32>,
    pub time_zone: String,
}

/// 銘柄の取引仕様
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstrumentSpec {
    pub symbol_name: String,
    /// 1 pip = 10^-pip_position
    pub pip_position: i32,
    pub digits: i32,
    /// 1 lot あたりの数量（FX は通常 100,000 通貨、XAUUSD は 100 oz など）
    pub lot_units: f64,
    /// cTrader の発注量（数量の 1/100 単位）の最小・刻み・最大
    pub min_volume: Option<i64>,
    pub step_volume: Option<i64>,
    pub max_volume: Option<i64>,
    /// ブローカーの資産クラス名（Forex / Metals / Indices / Energy など。ブローカーの表記のまま）
    pub asset_class: Option<String>,
    /// ブローカーの銘柄カテゴリ名
    pub category: Option<String>,
    pub description: Option<String>,
    pub base_asset: Option<String>,
    pub quote_asset: Option<String>,
    /// 数量の単位（oz, barrel など）
    pub measurement_units: Option<String>,
    /// ENABLED / CLOSE_ONLY_MODE / DISABLED_* など
    pub trading_mode: Option<String>,
    pub schedule: Vec<WeeklyInterval>,
    pub schedule_time_zone: Option<String>,
    pub holidays: Vec<Holiday>,
}

impl InstrumentSpec {
    pub fn pip_size(&self) -> f64 {
        10f64.powi(-self.pip_position)
    }

    /// 1 lot・1 pip あたりの損益（口座通貨）。
    /// `quote_to_account` は建値通貨 1 単位を口座通貨に換算するレート（同じ通貨なら 1.0）。
    pub fn pip_value_per_lot(&self, quote_to_account: f64) -> f64 {
        self.pip_size() * self.lot_units * quote_to_account
    }

    /// cTrader の発注量 1 lot 分（数量の 1/100 単位）
    pub fn volume_per_lot(&self) -> f64 {
        self.lot_units * 100.0
    }

    pub fn volume_to_lots(&self, volume: i64) -> f64 {
        volume as f64 / self.volume_per_lot()
    }

    /// ロット → 発注量。刻みに切り捨て、最大で抑える。最小未満なら None（勝手に増やすとリスクが膨らむため）
    pub fn lots_to_volume(&self, lots: f64) -> Option<i64> {
        let raw = (lots * self.volume_per_lot()).round() as i64;
        let step = self.step_volume.filter(|s| *s > 0).unwrap_or(1);
        let mut v = raw / step * step;
        if let Some(max) = self.max_volume.filter(|m| *m > 0) {
            v = v.min(max / step * step);
        }
        let min = self.min_volume.filter(|m| *m > 0).unwrap_or(step);
        (v >= min && v > 0).then_some(v)
    }

    /// 最小発注量（lot）
    pub fn min_lots(&self) -> Option<f64> {
        self.min_volume.filter(|m| *m > 0).map(|m| self.volume_to_lots(m))
    }

    /// FX（通貨ペア）か。ブローカーの資産クラスを優先し、無ければ基軸・建値がどちらも通貨コードかで判断する
    pub fn is_forex(&self) -> bool {
        if let Some(c) = &self.asset_class {
            return c.to_lowercase().contains("forex") || c.to_lowercase() == "fx";
        }
        name_looks_like_fx(&self.symbol_name)
    }

    /// `now` に取引時間内か。スケジュールが空なら常時取引可能とみなす（24/7 の銘柄や未取得）
    pub fn is_open_at(&self, now: DateTime<Utc>) -> bool {
        if self.holidays.iter().any(|h| holiday_covers(h, now)) {
            return false;
        }
        if self.schedule.is_empty() {
            return true;
        }
        let sec = seconds_of_week(now, self.schedule_time_zone.as_deref());
        self.schedule.iter().any(|i| sec >= i.start_second && sec < i.end_second)
    }

    /// `now` 以降で次に状態（開→閉 / 閉→開）が変わる時刻。分単位で最大 8 日先まで探す
    pub fn next_transition(&self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        if self.schedule.is_empty() && self.holidays.is_empty() {
            return None;
        }
        let open = self.is_open_at(now);
        let start = now.with_second(0)?.with_nanosecond(0)?;
        (1..=(8 * 24 * 60))
            .map(|m| start + Duration::minutes(m))
            .find(|t| self.is_open_at(*t) != open)
    }
}

/// 6 文字の英字（+ 口座サフィックス）で、両側が既知の通貨コードなら FX とみなす
pub fn name_looks_like_fx(name: &str) -> bool {
    const CCY: [&str; 18] = [
        "USD", "JPY", "EUR", "GBP", "AUD", "NZD", "CAD", "CHF", "SEK", "NOK", "DKK", "SGD", "HKD", "MXN", "ZAR", "TRY",
        "PLN", "CNH",
    ];
    let letters: String = name.to_uppercase().chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    letters.len() == 6 && CCY.contains(&&letters[..3]) && CCY.contains(&&letters[3..])
}

fn tz_offset_secs(tz: Option<&str>, at: DateTime<Utc>) -> i64 {
    let Some(name) = tz.filter(|t| !t.is_empty()) else { return 0 };
    match name.parse::<chrono_tz::Tz>() {
        Ok(tz) => {
            let local = tz.from_utc_datetime(&at.naive_utc());
            (local.naive_local() - at.naive_utc()).num_seconds()
        }
        Err(_) => 0,
    }
}

/// 日曜 00:00（現地）からの秒数
fn seconds_of_week(now: DateTime<Utc>, tz: Option<&str>) -> u32 {
    let local = now.naive_utc() + Duration::seconds(tz_offset_secs(tz, now));
    let day = local.weekday().num_days_from_sunday();
    day * 86_400 + local.num_seconds_from_midnight()
}

fn holiday_covers(h: &Holiday, now: DateTime<Utc>) -> bool {
    let local = now.naive_utc() + Duration::seconds(tz_offset_secs(Some(&h.time_zone), now));
    let Some(date) = NaiveDate::from_ymd_opt(1970, 1, 1).and_then(|d| d.checked_add_signed(Duration::days(h.date_days)))
    else {
        return false;
    };
    let same_day = if h.recurring {
        date.month() == local.month() && date.day() == local.day()
    } else {
        date == local.date()
    };
    if !same_day {
        return false;
    }
    let sec = local.num_seconds_from_midnight() as i32;
    match (h.start_second, h.end_second) {
        (Some(s), Some(e)) if e > s => sec >= s && sec < e,
        _ => true,
    }
}

static REGISTRY: LazyLock<RwLock<HashMap<String, InstrumentSpec>>> = LazyLock::new(|| RwLock::new(HashMap::new()));

/// 銘柄仕様を登録する（ブローカーから取得したとき）
pub fn register(spec: InstrumentSpec) {
    let mut reg = REGISTRY.write().unwrap_or_else(|e| e.into_inner());
    reg.insert(spec.symbol_name.to_uppercase(), spec);
}

/// 登録済みの銘柄仕様
pub fn get(symbol: &str) -> Option<InstrumentSpec> {
    REGISTRY.read().unwrap_or_else(|e| e.into_inner()).get(&symbol.trim().to_uppercase()).cloned()
}

/// 1 pip の価格幅。ブローカーの仕様を優先し、未登録なら銘柄名から推測する
pub fn pip_size(symbol: &str) -> f64 {
    get(symbol).map(|s| s.pip_size()).unwrap_or_else(|| guess_pip_size(symbol))
}

/// 価格の表示桁数
pub fn price_digits(symbol: &str) -> usize {
    get(symbol).map(|s| s.digits.max(0) as usize).unwrap_or_else(|| guess_digits(symbol))
}

/// FX（通貨ペア）か
pub fn is_forex(symbol: &str) -> bool {
    get(symbol).map(|s| s.is_forex()).unwrap_or_else(|| name_looks_like_fx(symbol))
}

/// 名前からの推測（ブローカー未接続時のフォールバック）
fn guess(symbol: &str) -> (i32, i32) {
    let p = symbol.to_uppercase();
    if p.contains("XAU") || p.contains("GOLD") {
        (2, 1) // 2 桁表示、1 pip = 0.1
    } else if p.contains("XAG") || p.contains("SILVER") {
        (3, 2)
    } else if p.contains("OIL") || p.contains("WTI") || p.contains("BRENT") || p.contains("XTI") || p.contains("XBR") {
        (2, 2)
    } else if name_looks_like_fx(&p) {
        if p.contains("JPY") {
            (3, 2)
        } else {
            (5, 4)
        }
    } else if p.chars().any(|c| c.is_ascii_digit()) {
        // US500 / US30 / NAS100 / JP225 などの株価指数
        (1, 0)
    } else {
        (5, 4)
    }
}

fn guess_pip_size(symbol: &str) -> f64 {
    10f64.powi(-guess(symbol).1)
}

fn guess_digits(symbol: &str) -> usize {
    guess(symbol).0 as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(name: &str) -> InstrumentSpec {
        InstrumentSpec {
            symbol_name: name.into(),
            pip_position: 2,
            digits: 2,
            lot_units: 100.0,
            min_volume: Some(100),
            step_volume: Some(100),
            max_volume: Some(1_000_000),
            asset_class: Some("Metals".into()),
            category: None,
            description: None,
            base_asset: Some("XAU".into()),
            quote_asset: Some("USD".into()),
            measurement_units: Some("oz".into()),
            trading_mode: Some("ENABLED".into()),
            schedule: Vec::new(),
            schedule_time_zone: None,
            holidays: Vec::new(),
        }
    }

    #[test]
    fn volume_uses_the_broker_lot_size_and_step() {
        // XAUUSD: 1 lot = 100 oz → 発注量 10,000。0.01 lot = 100（= 1 oz）
        let s = spec("XAUUSD_z");
        assert_eq!(s.lots_to_volume(0.01), Some(100));
        assert_eq!(s.lots_to_volume(0.237), Some(2300));
        // 最小未満は発注しない（勝手に最小へ切り上げない）
        assert_eq!(s.lots_to_volume(0.004), None);
        assert!((s.volume_to_lots(2300) - 0.23).abs() < 1e-12);
        // 1 lot・1 pip（0.01 USD）= 100 oz × 0.01 = 1 USD
        assert!((s.pip_value_per_lot(1.0) - 1.0).abs() < 1e-12);
        // 最大で抑える
        assert_eq!(s.lots_to_volume(1000.0), Some(1_000_000));
    }

    #[test]
    fn schedule_decides_open_and_close() {
        // UTC で 月曜 01:00〜金曜 21:00 のみ取引
        let mut s = spec("US500");
        s.schedule = vec![WeeklyInterval { start_second: 86_400 + 3_600, end_second: 5 * 86_400 + 21 * 3_600 }];
        s.schedule_time_zone = Some("UTC".into());
        let sat = Utc.with_ymd_and_hms(2026, 10, 3, 12, 0, 0).unwrap();
        let wed = Utc.with_ymd_and_hms(2026, 9, 30, 12, 0, 0).unwrap();
        assert!(!s.is_open_at(sat));
        assert!(s.is_open_at(wed));
        assert_eq!(s.next_transition(sat), Some(Utc.with_ymd_and_hms(2026, 10, 5, 1, 0, 0).unwrap()));

        // 休場日（終日）
        let days = (NaiveDate::from_ymd_opt(2026, 9, 30).unwrap() - NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()).num_days();
        s.holidays.push(Holiday {
            name: "x".into(),
            date_days: days,
            recurring: false,
            start_second: None,
            end_second: None,
            time_zone: "UTC".into(),
        });
        assert!(!s.is_open_at(wed));
    }

    #[test]
    fn schedule_respects_the_time_zone() {
        // ニューヨーク時間の 月曜 00:00〜 を UTC で見ると 月曜 04:00（夏時間）
        let mut s = spec("US500");
        s.schedule = vec![WeeklyInterval { start_second: 86_400, end_second: 2 * 86_400 }];
        s.schedule_time_zone = Some("America/New_York".into());
        assert!(!s.is_open_at(Utc.with_ymd_and_hms(2026, 10, 5, 3, 0, 0).unwrap()));
        assert!(s.is_open_at(Utc.with_ymd_and_hms(2026, 10, 5, 5, 0, 0).unwrap()));
    }

    #[test]
    fn guesses_cover_fx_metals_indices_and_oil() {
        assert_eq!(guess_pip_size("USDJPY_z"), 0.01);
        assert_eq!(guess_pip_size("EURUSD"), 0.0001);
        assert_eq!(guess_pip_size("XAUUSD"), 0.1);
        assert_eq!(guess_pip_size("US500"), 1.0);
        assert!(name_looks_like_fx("GBPJPY_z"));
        assert!(!name_looks_like_fx("XAUUSD"));
        assert!(!name_looks_like_fx("US500"));
    }
}
