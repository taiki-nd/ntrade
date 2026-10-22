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
    /// 口座の開設日時（残高の推移を遡る起点）
    #[serde(default)]
    pub registered_at: Option<DateTime<Utc>>,
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
    /// この建玉が使っている証拠金（口座通貨）
    #[serde(default)]
    pub used_margin: Option<f64>,
    /// 発注時の comment（予約注文から建った建玉は判断 ID を引き継ぐ）
    #[serde(default)]
    pub comment: Option<String>,
}

/// 予約注文の種類
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum PendingKind {
    /// 指値（今より有利な価格まで戻ったら約定）
    Limit,
    /// 逆指値（価格がその水準を抜けたら約定）
    Stop,
}

/// ブローカー側に置かれている予約注文（reconcile の結果）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BrokerOrder {
    pub order_id: i64,
    pub symbol_id: i64,
    pub symbol_name: Option<String>,
    pub is_buy: bool,
    /// LIMIT / STOP 以外（ストップリミットなど）は None
    pub kind: Option<PendingKind>,
    pub volume_lots: f64,
    pub price: Option<f64>,
    pub stop_loss: Option<f64>,
    pub take_profit: Option<f64>,
    pub expires_at: Option<DateTime<Utc>>,
    /// 発注時に付けた comment（ntrade は判断 ID を入れる）
    pub comment: Option<String>,
}

/// 注文 ID から引いた注文の結末
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderFate {
    /// 約定して建玉になった（建玉 ID）
    Filled { position_id: i64 },
    Cancelled,
    Expired,
    Rejected,
    /// まだブローカーに置かれている
    Working,
    Unknown,
}

/// Spot 購読で受け取った直近の気配値
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Quote {
    pub bid: f64,
    pub ask: f64,
    pub time: DateTime<Utc>,
}

impl Quote {
    pub fn spread(&self) -> f64 {
        (self.ask - self.bid).max(0.0)
    }
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

/// 口座残高が動いた出来事（決済・入出金）と、その直後の残高
#[derive(Debug, Clone, PartialEq)]
pub struct BalanceEvent {
    /// "deal-<id>" / "cash-<id>"（同じ出来事を二重に数えないためのキー）
    pub id: String,
    pub time: DateTime<Utc>,
    /// 残高の版。同時刻の出来事の前後関係に使う
    pub version: Option<i64>,
    pub balance: f64,
}

/// シンボル（銘柄一覧の1行）。仕様の詳細は `instrument::InstrumentSpec`（`CTraderService::symbol_spec`）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolInfo {
    pub symbol_id: i64,
    pub symbol_name: String,
    /// 基軸通貨・資産（EURUSD なら EUR、XAUUSD なら XAU）。ブローカーの資産一覧から引けなかった場合は None
    #[serde(default)]
    pub base_asset: Option<String>,
    /// 建値通貨（EURUSD なら USD）。ブローカーの資産一覧から引けなかった場合は None
    #[serde(default)]
    pub quote_asset: Option<String>,
    /// ブローカーの銘柄カテゴリ名と資産クラス名（Forex / Metals / Indices など。ブローカーの表記のまま）
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub asset_class: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub enabled: bool,
}

/// 6文字の通貨ペア名から建値通貨を推定する（資産一覧が引けないときの代替）
pub fn quote_currency_from_name(pair: &str) -> Option<String> {
    let p = pair.replace('/', "").to_uppercase();
    let letters: String = p.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    (letters.len() == 6).then(|| letters[3..].to_string())
}

impl SymbolInfo {
    pub fn new(symbol_id: i64, symbol_name: String) -> Self {
        Self {
            symbol_id,
            symbol_name,
            base_asset: None,
            quote_asset: None,
            category: None,
            asset_class: None,
            description: None,
            enabled: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_currency_is_inferred_from_six_letter_names() {
        assert_eq!(quote_currency_from_name("EURUSD").as_deref(), Some("USD"));
        assert_eq!(quote_currency_from_name("gbp/jpy").as_deref(), Some("JPY"));
        assert_eq!(quote_currency_from_name("USDJPY_z").as_deref(), Some("JPY"));
        assert_eq!(quote_currency_from_name("US30"), None);
    }
}
