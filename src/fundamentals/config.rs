//! `config/fundamentals.toml` の読み込みと、銘柄 → 使う事実の解決。

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub const DEFAULT_FUNDAMENTALS_CONFIG_PATH: &str = "config/fundamentals.toml";
/// 設定ファイルが無いときの既定値（リポジトリの設定ファイルそのもの）
const BUILTIN: &str = include_str!("../../config/fundamentals.toml");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SourceToggles {
    pub fred: bool,
    pub treasury: bool,
    pub jgb: bool,
    pub cftc: bool,
    pub eia: bool,
    pub calendar: bool,
    pub press: bool,
}

impl Default for SourceToggles {
    fn default() -> Self {
        Self { fred: true, treasury: true, jgb: true, cftc: true, eia: true, calendar: true, press: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CalendarConfig {
    pub url: String,
    pub include_forecast: bool,
    pub include_impact: bool,
    pub lookback_hours: i64,
    pub lookahead_hours: i64,
}

impl Default for CalendarConfig {
    fn default() -> Self {
        Self {
            url: "https://nfs.faireconomy.media/ff_calendar_thisweek.json".into(),
            include_forecast: false,
            include_impact: true,
            lookback_hours: 24,
            lookahead_hours: 168,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PressConfig {
    pub lookback_days: i64,
    pub max_items: usize,
}

impl Default for PressConfig {
    fn default() -> Self {
        Self { lookback_days: 7, max_items: 12 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RefreshMinutes {
    pub series: i64,
    pub cot: i64,
    pub calendar: i64,
    pub press: i64,
}

impl Default for RefreshMinutes {
    fn default() -> Self {
        Self { series: 360, cot: 360, calendar: 60, press: 15 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EventTrigger {
    pub impacts: Vec<String>,
    pub window_minutes: i64,
}

impl Default for EventTrigger {
    fn default() -> Self {
        Self { impacts: vec!["High".into()], window_minutes: 10 }
    }
}

/// 1 通貨・1 銘柄ぶんの事実セット
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FactSet {
    pub series: Vec<String>,
    pub cot: Vec<String>,
    pub press: Vec<String>,
    /// 経済指標カレンダーの通貨コード（USD / JPY ...）
    pub calendar: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub name: String,
    #[serde(rename = "match")]
    pub matches: Vec<String>,
    pub currencies: Vec<String>,
    #[serde(flatten)]
    pub facts: FactSet,
    pub related: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FundamentalsConfig {
    pub enabled: bool,
    pub sources: SourceToggles,
    pub calendar: CalendarConfig,
    pub press: PressConfig,
    pub refresh_minutes: RefreshMinutes,
    pub event_trigger: EventTrigger,
    pub feeds: BTreeMap<String, String>,
    pub currencies: BTreeMap<String, FactSet>,
    pub profiles: Vec<Profile>,
}

impl Default for FundamentalsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sources: SourceToggles::default(),
            calendar: CalendarConfig::default(),
            press: PressConfig::default(),
            refresh_minutes: RefreshMinutes::default(),
            event_trigger: EventTrigger::default(),
            feeds: BTreeMap::new(),
            currencies: BTreeMap::new(),
            profiles: Vec::new(),
        }
    }
}

/// 銘柄に対して解決した、使う事実の一覧
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Selection {
    pub profile: Option<String>,
    pub series: Vec<String>,
    pub cot: Vec<String>,
    pub press: Vec<String>,
    pub calendar: Vec<String>,
    pub related: Vec<String>,
}

impl FundamentalsConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let text = std::fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read fundamentals config {:?}", path.as_ref()))?;
        toml::from_str(&text).context("Failed to parse fundamentals config")
    }

    /// ファイルが無ければリポジトリ同梱の既定値
    pub fn load_or_builtin(path: impl AsRef<Path>) -> Self {
        match Self::load(&path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("{e:#}; using the built-in fundamentals config");
                Self::builtin()
            }
        }
    }

    pub fn builtin() -> Self {
        toml::from_str(BUILTIN).expect("built-in fundamentals config must parse")
    }

    /// 銘柄に使う事実を決める。
    ///
    /// 1. `profiles` の `match` を銘柄名に含むプロファイル（最初の一致）
    /// 2. 無ければ基軸・建値の通貨（ブローカーの資産名、無ければ 6 文字の銘柄名から）
    ///
    /// どちらの場合も、挙げた通貨の事実セットを合わせる。
    pub fn select(&self, symbol: &str, base: Option<&str>, quote: Option<&str>) -> Selection {
        let upper = symbol.to_uppercase();
        let profile = self
            .profiles
            .iter()
            .find(|p| p.matches.iter().any(|m| !m.is_empty() && upper.contains(&m.to_uppercase())));

        let currencies: Vec<String> = match profile {
            Some(p) => p.currencies.clone(),
            None => {
                let letters: String = upper.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
                let from_name = |range: std::ops::Range<usize>| (letters.len() == 6).then(|| letters[range].to_string());
                [base.map(asset_code).or_else(|| from_name(0..3)), quote.map(asset_code).or_else(|| from_name(3..6))]
                    .into_iter()
                    .flatten()
                    .collect()
            }
        };

        let mut series = Vec::new();
        let mut cot = Vec::new();
        let mut press = Vec::new();
        let mut calendar = Vec::new();
        let mut add = |f: &FactSet| {
            series.extend(f.series.iter().cloned());
            cot.extend(f.cot.iter().cloned());
            press.extend(f.press.iter().cloned());
            calendar.extend(f.calendar.iter().cloned());
        };
        for c in &currencies {
            if let Some(f) = self.currencies.get(c) {
                add(f);
            }
        }
        if let Some(p) = profile {
            add(&p.facts);
        }
        Selection {
            profile: profile.map(|p| p.name.clone()),
            series: dedup(series),
            cot: dedup(cot),
            press: dedup(press),
            calendar: dedup(calendar),
            related: profile.map(|p| p.related.clone()).unwrap_or_default(),
        }
    }

    /// 全銘柄ぶんの取得対象（定期取得で使う）
    pub fn all_series(&self) -> BTreeSet<String> {
        self.currencies
            .values()
            .chain(self.profiles.iter().map(|p| &p.facts))
            .flat_map(|f| f.series.iter().cloned())
            .collect()
    }

    pub fn all_cot(&self) -> BTreeSet<String> {
        self.currencies
            .values()
            .chain(self.profiles.iter().map(|p| &p.facts))
            .flat_map(|f| f.cot.iter().cloned())
            .collect()
    }
}

/// ブローカーの資産名を通貨コードにそろえる。ゼロ口座では基軸が `USD_Z` のように口座サフィックス付きで返る
fn asset_code(name: &str) -> String {
    name.trim().to_uppercase().chars().take_while(|c| c.is_ascii_alphabetic()).collect()
}

fn dedup(v: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for x in v {
        if !out.contains(&x) {
            out.push(x);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_config_parses_and_selects_by_profile_or_currency() {
        let cfg = FundamentalsConfig::builtin();
        assert!(cfg.enabled);
        assert!(!cfg.calendar.include_forecast, "forecasts stay out unless explicitly enabled");

        let gold = cfg.select("XAUUSD_z", Some("XAU"), Some("USD"));
        assert_eq!(gold.profile.as_deref(), Some("gold"));
        assert!(gold.cot.contains(&"088691".to_string()));
        assert!(gold.series.contains(&"ust_real:10Y".to_string()), "USD facts are merged in: {:?}", gold.series);
        assert!(gold.related.contains(&"XAGUSD".to_string()));

        // FX はプロファイルなしで両通貨のセットを合わせる
        let usdjpy = cfg.select("USDJPY_z", None, None);
        assert_eq!(usdjpy.profile, None);
        assert!(usdjpy.series.contains(&"jgb:10Y".to_string()) && usdjpy.series.contains(&"ust:10Y".to_string()));
        assert_eq!(usdjpy.calendar, ["USD", "JPY"]);
        assert!(usdjpy.cot.contains(&"097741".to_string()));
        // ゼロ口座のブローカーは基軸を "USD_Z" で返す（実例）
        let zero = cfg.select("USDJPY_z", Some("USD_Z"), Some("JPY"));
        assert_eq!(zero.calendar, ["USD", "JPY"]);
        assert!(zero.series.contains(&"ust:10Y".to_string()));

        let oil = cfg.select("XTIUSD", None, None);
        assert_eq!(oil.profile.as_deref(), Some("crude_oil"));
        assert!(oil.series.iter().any(|s| s.starts_with("eia:")));
    }
}
