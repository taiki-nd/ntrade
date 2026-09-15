use chrono::{DateTime, TimeZone, Utc};
use ctrader_rs::proto::common::{ProtoOaTrendbar, ProtoOaTrendbarPeriod};
use serde::{Deserialize, Serialize};

/// ローソク足（OHLCV）データ
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CandleBar {
    pub timestamp: DateTime<Utc>,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: i64,
}

impl CandleBar {
    /// cTraderのProtoOaTrendbarからCandleBarに変換
    /// cTrader APIの価格は100,000倍（10^5）された整数値
    pub fn from_proto(bar: &ProtoOaTrendbar) -> Self {
        let low_raw = bar.low.unwrap_or(0);
        let delta_open = bar.delta_open.unwrap_or(0) as i64;
        let delta_high = bar.delta_high.unwrap_or(0) as i64;
        let delta_close = bar.delta_close.unwrap_or(0) as i64;

        let scale = 100_000.0;
        let low = low_raw as f64 / scale;
        let open = (low_raw + delta_open) as f64 / scale;
        let high = (low_raw + delta_high) as f64 / scale;
        let close = (low_raw + delta_close) as f64 / scale;

        let minutes = bar.utc_timestamp_in_minutes.unwrap_or(0) as i64;
        let timestamp_secs = minutes * 60;
        let timestamp = Utc
            .timestamp_opt(timestamp_secs, 0)
            .single()
            .unwrap_or_else(Utc::now);

        Self {
            timestamp,
            open,
            high,
            low,
            close,
            volume: bar.volume,
        }
    }
}

/// 時間足の定義
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BarPeriod {
    M1,
    M5,
    M15,
    M30,
    H1,
    H4,
    D1,
}

impl BarPeriod {
    pub fn as_str(&self) -> &'static str {
        match self {
            BarPeriod::M1 => "1M",
            BarPeriod::M5 => "5M",
            BarPeriod::M15 => "15M",
            BarPeriod::M30 => "30M",
            BarPeriod::H1 => "1H",
            BarPeriod::H4 => "4H",
            BarPeriod::D1 => "1D",
        }
    }

    /// 1本の足の長さ
    pub fn duration(&self) -> chrono::Duration {
        match self {
            BarPeriod::M1 => chrono::Duration::minutes(1),
            BarPeriod::M5 => chrono::Duration::minutes(5),
            BarPeriod::M15 => chrono::Duration::minutes(15),
            BarPeriod::M30 => chrono::Duration::minutes(30),
            BarPeriod::H1 => chrono::Duration::hours(1),
            BarPeriod::H4 => chrono::Duration::hours(4),
            BarPeriod::D1 => chrono::Duration::days(1),
        }
    }

    /// "5M" / "M5" / "1H" / "H1" などの表記から変換
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "1M" | "M1" => Some(BarPeriod::M1),
            "5M" | "M5" => Some(BarPeriod::M5),
            "15M" | "M15" => Some(BarPeriod::M15),
            "30M" | "M30" => Some(BarPeriod::M30),
            "1H" | "H1" => Some(BarPeriod::H1),
            "4H" | "H4" => Some(BarPeriod::H4),
            "1D" | "D1" => Some(BarPeriod::D1),
            _ => None,
        }
    }

    pub fn to_proto(&self) -> ProtoOaTrendbarPeriod {
        match self {
            BarPeriod::M1 => ProtoOaTrendbarPeriod::M1,
            BarPeriod::M5 => ProtoOaTrendbarPeriod::M5,
            BarPeriod::M15 => ProtoOaTrendbarPeriod::M15,
            BarPeriod::M30 => ProtoOaTrendbarPeriod::M30,
            BarPeriod::H1 => ProtoOaTrendbarPeriod::H1,
            BarPeriod::H4 => ProtoOaTrendbarPeriod::H4,
            BarPeriod::D1 => ProtoOaTrendbarPeriod::D1,
        }
    }

    pub fn to_proto_i32(&self) -> i32 {
        self.to_proto() as i32
    }
}

impl From<BarPeriod> for ProtoOaTrendbarPeriod {
    fn from(period: BarPeriod) -> Self {
        period.to_proto()
    }
}

impl From<BarPeriod> for i32 {
    fn from(period: BarPeriod) -> Self {
        period.to_proto_i32()
    }
}

/// シンボル（通貨ペア）情報
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolInfo {
    pub symbol_id: i64,
    pub symbol_name: String,
    pub digits: i32,
    pub pip_position: i32,
}

impl SymbolInfo {
    /// 通貨名から標準的な桁数とpip位置を推定
    pub fn new_with_defaults(symbol_id: i64, symbol_name: String) -> Self {
        let name_upper = symbol_name.to_uppercase();
        let (digits, pip_position) = if name_upper.contains("JPY") {
            (3, 2)
        } else if name_upper.contains("XAU") || name_upper.contains("GOLD") {
            (2, 2)
        } else {
            (5, 4)
        };

        Self {
            symbol_id,
            symbol_name,
            digits,
            pip_position,
        }
    }
}
