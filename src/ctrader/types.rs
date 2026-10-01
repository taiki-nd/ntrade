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

/// ロット → cTrader プロトコルのボリューム（1/100 単位。1 lot = 100,000 units = 10,000,000）
pub fn lots_to_volume(lots: f64) -> i64 {
    (lots * 10_000_000.0).round() as i64
}

pub fn volume_to_lots(volume: i64) -> f64 {
    volume as f64 / 10_000_000.0
}

/// 口座サマリー（get_trader の結果）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AccountSummary {
    pub account_id: i64,
    /// cTrader アプリに表示される口座番号（ログイン番号）
    pub trader_login: Option<i64>,
    pub balance: f64,
    pub leverage: Option<f64>,
    pub is_live: bool,
    /// 口座側の取引権限（FULL_ACCESS / CLOSE_ONLY / NO_TRADING / NO_LOGIN）
    #[serde(default)]
    pub access_rights: Option<String>,
    /// 口座通貨（JPY など）。資産一覧から引けなかった場合は None
    #[serde(default)]
    pub deposit_currency: Option<String>,
}

/// ブローカー側の保有ポジション（reconcile の結果）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BrokerPosition {
    pub position_id: i64,
    pub symbol_id: i64,
    pub symbol_name: Option<String>,
    pub is_buy: bool,
    pub volume_lots: f64,
    pub entry_price: Option<f64>,
    pub stop_loss: Option<f64>,
    pub take_profit: Option<f64>,
    pub open_time: Option<DateTime<Utc>>,
}

/// ブローカー側で確定した決済（closing deal）。
///
/// 決済価格と実現損益をローカルの足から推定せず、約定そのものから確定させるために使う。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClosedDeal {
    pub position_id: i64,
    pub deal_id: i64,
    /// 決済の約定価格
    pub close_price: f64,
    pub close_time: DateTime<Utc>,
    /// ブローカーが決済時に確定した建値
    pub entry_price: f64,
    /// 実現損益（口座通貨。スワップ・手数料込み。cTrader の「純額」に相当）
    pub net_profit: f64,
    pub gross_profit: f64,
    pub swap: f64,
    pub commission: f64,
    pub volume_lots: f64,
}

/// シンボル（通貨ペア）情報
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolInfo {
    pub symbol_id: i64,
    pub symbol_name: String,
    pub digits: i32,
    pub pip_position: i32,
    /// 基軸通貨（EURUSD なら EUR）。ブローカーの資産一覧から引けなかった場合は None
    #[serde(default)]
    pub base_asset: Option<String>,
    /// 建値通貨（EURUSD なら USD）。ブローカーの資産一覧から引けなかった場合は None
    #[serde(default)]
    pub quote_asset: Option<String>,
}

/// ブローカーが持つ銘柄の取引仕様（ProtoOASymbol）
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SymbolSpec {
    /// 1 pip = 10^-pip_position
    pub pip_position: i32,
    pub digits: i32,
    /// 1 lot あたりの数量（通貨単位。FX は通常 100,000）
    pub lot_units: f64,
}

impl SymbolSpec {
    pub fn pip_size(&self) -> f64 {
        10f64.powi(-self.pip_position)
    }

    /// 1 lot・1 pip あたりの損益（口座通貨）。
    /// `quote_to_account` は建値通貨 1 単位を口座通貨に換算するレート（同じ通貨なら 1.0）。
    pub fn pip_value_per_lot(&self, quote_to_account: f64) -> f64 {
        self.pip_size() * self.lot_units * quote_to_account
    }
}

/// 6文字の通貨ペア名から建値通貨を推定する（資産一覧が引けないときの代替）
pub fn quote_currency_from_name(pair: &str) -> Option<String> {
    let p = pair.replace('/', "").to_uppercase();
    let letters: String = p.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    (letters.len() == 6).then(|| letters[3..].to_string())
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
            base_asset: None,
            quote_asset: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pip_value_follows_pip_position_lot_size_and_conversion() {
        let usdjpy = SymbolSpec { pip_position: 2, digits: 3, lot_units: 100_000.0 };
        // JPY 口座で JPY 建て: 0.01 * 100,000 = 1,000 円
        assert!((usdjpy.pip_value_per_lot(1.0) - 1000.0).abs() < 1e-6);
        let eurusd = SymbolSpec { pip_position: 4, digits: 5, lot_units: 100_000.0 };
        // JPY 口座で USD 建て: 10 USD * USDJPY 150 = 1,500 円
        assert!((eurusd.pip_value_per_lot(150.0) - 1500.0).abs() < 1e-6);
        // USD 口座で JPY 建て: 1,000 円 / USDJPY 150
        assert!((usdjpy.pip_value_per_lot(1.0 / 150.0) - 6.666_666).abs() < 1e-3);
    }

    #[test]
    fn quote_currency_is_inferred_from_six_letter_names() {
        assert_eq!(quote_currency_from_name("EURUSD").as_deref(), Some("USD"));
        assert_eq!(quote_currency_from_name("gbp/jpy").as_deref(), Some("JPY"));
        assert_eq!(quote_currency_from_name("USDJPY_z").as_deref(), Some("JPY"));
        assert_eq!(quote_currency_from_name("US30"), None);
    }
}
