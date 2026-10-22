//! ロジック戦略。戦略は `Strategy` を実装し、`catalog` / `build` に登録する。

pub mod donchian;
pub mod ema_cross;
pub mod types;

use anyhow::{bail, Result};

pub use types::{
    Action, Entry, OrderIntent, OrderView, ParamSpec, Params, PositionView, Side, Strategy, StrategyContext, StrategyInfo,
    StrategySpec,
};

/// 使える戦略の一覧
pub fn catalog() -> Vec<StrategyInfo> {
    vec![ema_cross::info(), donchian::info()]
}

pub fn info(name: &str) -> Option<StrategyInfo> {
    catalog().into_iter().find(|i| i.name == name)
}

/// 既定値で埋め、未知の名前と範囲外の値を弾いたパラメータ
pub fn resolve_params(spec: &StrategySpec) -> Result<Params> {
    let Some(info) = info(&spec.name) else {
        let names: Vec<&str> = catalog().iter().map(|i| i.name).collect();
        bail!("unknown strategy '{}' (available: {})", spec.name, names.join(", "));
    };
    if let Some(unknown) = spec.params.keys().find(|k| !info.params.iter().any(|p| p.name == k.as_str())) {
        let names: Vec<&str> = info.params.iter().map(|p| p.name).collect();
        bail!("{}: unknown parameter '{unknown}' (available: {})", spec.name, names.join(", "));
    }
    let mut out = Params::new();
    for p in &info.params {
        let v = spec.params.get(p.name).copied().unwrap_or(p.default);
        if !v.is_finite() || v < p.min || v > p.max {
            bail!("{}: {} = {v} is out of range {}..={}", spec.name, p.name, p.min, p.max);
        }
        out.insert(p.name.to_string(), if p.integer { v.round() } else { v });
    }
    Ok(out)
}

/// 戦略を組み立てる
pub fn build(spec: &StrategySpec) -> Result<Box<dyn Strategy>> {
    let params = resolve_params(spec)?;
    let get = |k: &str| params[k];
    Ok(match spec.name.as_str() {
        ema_cross::NAME => Box::new(ema_cross::EmaCross::from_params(get)?),
        donchian::NAME => Box::new(donchian::DonchianBreakout::from_params(get, spec.timeframe)?),
        other => bail!("unknown strategy '{other}'"),
    })
}

/// ATR に倍率を掛けた損切り幅と、RR 倍の利確幅から SL / TP を決める（`rr` が 0 なら TP なし）
pub fn stops_from(side: Side, entry: f64, sl_distance: f64, rr: f64) -> (f64, Option<f64>) {
    let sl = entry - side.sign() * sl_distance;
    let tp = (rr > 0.0).then(|| entry + side.sign() * sl_distance * rr);
    (sl, tp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctrader::BarPeriod;

    #[test]
    fn params_are_defaulted_and_validated() {
        let spec = StrategySpec { name: "ema_cross".into(), timeframe: BarPeriod::M5, params: Params::from([("fast".into(), 9.6)]) };
        let p = resolve_params(&spec).unwrap();
        assert_eq!(p["fast"], 10.0);
        assert_eq!(p["slow"], 50.0);
        assert!(build(&spec).is_ok());

        let bad = StrategySpec { params: Params::from([("speed".into(), 1.0)]), ..spec.clone() };
        assert!(resolve_params(&bad).unwrap_err().to_string().contains("unknown parameter"));
        let out_of_range = StrategySpec { params: Params::from([("fast".into(), -1.0)]), ..spec.clone() };
        assert!(resolve_params(&out_of_range).is_err());
        assert!(build(&StrategySpec { name: "nope".into(), ..spec }).is_err());
    }

    #[test]
    fn stops_follow_the_side() {
        assert_eq!(stops_from(Side::Buy, 100.0, 1.0, 2.0), (99.0, Some(102.0)));
        assert_eq!(stops_from(Side::Sell, 100.0, 1.0, 0.0), (101.0, None));
    }
}
