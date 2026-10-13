use ctrader_rs::proto::common::ProtoOaTrendbar;
use ntrade::ctrader::{BarPeriod, CandleBar};

#[test]
fn test_candle_bar_from_proto() {
    // low: 15000000 (150.000)
    // delta_open: 20000 (0.200) -> open: 150.200
    // delta_high: 50000 (0.500) -> high: 150.500
    // delta_close: 10000 (0.100) -> close: 150.100
    let proto = ProtoOaTrendbar {
        volume: 1234,
        period: None,
        low: Some(15_000_000),
        delta_open: Some(20_000),
        delta_high: Some(50_000),
        delta_close: Some(10_000),
        utc_timestamp_in_minutes: Some(29_000_000), // 適当な分数
    };

    let bar = CandleBar::from_proto(&proto);
    assert!((bar.low - 150.0).abs() < 1e-5);
    assert!((bar.open - 150.2).abs() < 1e-5);
    assert!((bar.high - 150.5).abs() < 1e-5);
    assert!((bar.close - 150.1).abs() < 1e-5);
    assert_eq!(bar.volume, 1234);
}

#[test]
fn test_bar_period_conversions() {
    assert_eq!(BarPeriod::M5.as_str(), "5M");
    assert_eq!(BarPeriod::H1.as_str(), "1H");
    assert_eq!(BarPeriod::H4.as_str(), "4H");

    let proto_period = BarPeriod::M5.to_proto();
    assert_eq!(proto_period as i32, BarPeriod::M5.to_proto_i32());
}

#[test]
fn test_pip_size_fallbacks_without_broker_specs() {
    // ブローカーの仕様が未取得のときだけ使う、銘柄名からの推測
    assert_eq!(ntrade::instrument::pip_size("USDJPY"), 0.01);
    assert_eq!(ntrade::instrument::price_digits("USDJPY"), 3);
    assert_eq!(ntrade::instrument::pip_size("EURUSD"), 0.0001);
    assert_eq!(ntrade::instrument::price_digits("EURUSD"), 5);
    assert_eq!(ntrade::instrument::pip_size("XAUUSD"), 0.1);
    assert_eq!(ntrade::instrument::price_digits("XAUUSD"), 2);
}
