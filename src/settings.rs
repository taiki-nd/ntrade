//! 画面から変更できる取引設定（対象ペア・足確定後の待ち秒数・事後ガード）。
//!
//! SQLite の `app_settings` に保存し、保存し直すと再起動せずに次の判断サイクルから反映される。
//! 初回起動時（未保存）は環境変数 `NTRADE_PAIRS` / `NTRADE_BAR_DELAY_SECS` と `config/guard.toml` を初期値にする。
//!
//! ペアはブローカーの銘柄名そのもの（ゼロ口座なら `USDJPY_z`）で持ち、画面の表示・判断ログ・発注まで同じ名前を使う。
//! 素の `USDJPY` も一覧には存在するが、ゼロ口座では発注が TRADING_DISABLED で拒否されるので区別が要る。

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{info, warn};

use crate::guard::{GuardConfig, DEFAULT_GUARD_CONFIG_PATH};
use crate::storage::Db;

pub const SETTINGS_KEY: &str = "trading";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingSettings {
    /// 判断サイクルを回す銘柄名（ブローカーの表記どおり。先頭が画面・API の既定ペア）
    pub pairs: Vec<String>,
    /// 5M 足確定からデータ取得までの待ち秒数
    pub bar_delay_secs: i64,
    /// LLM に判断させる間隔（分。5 の倍数、UTC 0:00 起点）。条件付きプランと終値判定の損切りは
    /// この間隔に関係なく 5M 足ごとに評価し、プランが成立したらその足で判断する
    #[serde(default = "default_decision_interval")]
    pub decision_interval_minutes: u32,
    /// 重要指標の発表直後（5M 足 1 本確定後）に、判断間隔を待たずに判断する（既定はオフ）
    #[serde(default)]
    pub event_decisions: bool,
    /// 事後ガード（キーは snake_case のまま）
    pub guard: GuardConfig,
}

fn default_decision_interval() -> u32 {
    std::env::var("NTRADE_DECISION_INTERVAL_MIN").ok().and_then(|s| s.parse().ok()).unwrap_or(5)
}

/// 初期値のペア。`NTRADE_PAIRS` は素の名前で書かれていることがあるので、`CTRADER_SYMBOL_SUFFIX` が
/// あれば補う（ここ以外では補わない。保存後は画面で入力した銘柄名が正）
fn pairs_from_env() -> Vec<String> {
    let suffix = crate::ctrader::symbol_suffix_from_env();
    let pairs = std::env::var("NTRADE_PAIRS")
        .ok()
        .map(|s| normalize_pairs(s.split(',').map(str::to_string).collect()))
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| vec!["USDJPY".to_string()]);
    with_suffix(pairs, suffix.as_deref())
}

fn with_suffix(pairs: Vec<String>, suffix: Option<&str>) -> Vec<String> {
    match suffix {
        Some(sfx) => pairs
            .into_iter()
            .map(|p| if p.to_lowercase().ends_with(&sfx.to_lowercase()) { p } else { format!("{p}{sfx}") })
            .collect(),
        None => pairs,
    }
}

/// 空白と "/" の除去・重複除去（大文字小文字は区別せず、順序と表記は保つ）
pub fn normalize_pairs(pairs: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in pairs {
        let p = p.trim().replace('/', "");
        if !p.is_empty() && !out.iter().any(|o| o.eq_ignore_ascii_case(&p)) {
            out.push(p);
        }
    }
    out
}

impl TradingSettings {
    /// 未保存時の初期値（環境変数と config/guard.toml）
    pub fn from_env_and_file() -> Self {
        Self {
            pairs: pairs_from_env(),
            bar_delay_secs: std::env::var("NTRADE_BAR_DELAY_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(15),
            decision_interval_minutes: default_decision_interval(),
            event_decisions: false,
            guard: GuardConfig::load_or_default(DEFAULT_GUARD_CONFIG_PATH),
        }
    }

    pub fn normalize(&mut self) {
        self.pairs = normalize_pairs(std::mem::take(&mut self.pairs));
        self.guard.normalize();
    }

    pub fn validate(&self) -> Result<()> {
        if self.pairs.is_empty() {
            anyhow::bail!("対象ペアを1つ以上指定してください");
        }
        if let Some(bad) = self.pairs.iter().find(|p| !p.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')) {
            anyhow::bail!("ペア名 {bad} に使えない文字が含まれています");
        }
        if !(0..=240).contains(&self.bar_delay_secs) {
            anyhow::bail!("足確定後の待ち秒数は 0〜240 秒");
        }
        if self.decision_interval_minutes == 0 || !self.decision_interval_minutes.is_multiple_of(5) || self.decision_interval_minutes > 1440 {
            anyhow::bail!("判断間隔は 5〜1440 分の 5 の倍数");
        }
        self.guard.validate()
    }

    /// この 5M 足確定（`tick`）で LLM に判断させるか
    pub fn is_decision_tick(&self, tick: chrono::DateTime<chrono::Utc>) -> bool {
        use chrono::Timelike;
        let minute_of_day = tick.hour() * 60 + tick.minute();
        minute_of_day.is_multiple_of(self.decision_interval_minutes.max(5))
    }

    pub fn default_pair(&self) -> String {
        self.pairs.first().cloned().unwrap_or_else(|| "USDJPY".to_string())
    }

    /// SQLite の保存値を読む。未保存なら初期値を保存してから返す（以後は SQLite が正）
    pub fn load_or_seed(db_path: &Path) -> Self {
        let loaded = Db::open(db_path).and_then(|db| db.load_setting::<TradingSettings>(SETTINGS_KEY));
        match loaded {
            Ok(Some(s)) => {
                info!(pairs = ?s.pairs, "Loaded trading settings from SQLite");
                s
            }
            Ok(None) => {
                let s = Self::from_env_and_file();
                match Db::open(db_path).and_then(|db| db.save_setting(SETTINGS_KEY, &s)) {
                    Ok(()) => info!(pairs = ?s.pairs, "Seeded trading settings into SQLite from env / guard.toml"),
                    Err(e) => warn!("Failed to seed trading settings: {e:#}"),
                }
                s
            }
            Err(e) => {
                warn!("Failed to load trading settings from SQLite; using env / guard.toml: {e:#}");
                Self::from_env_and_file()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairs_are_normalized_and_deduplicated() {
        let p = normalize_pairs(vec![" USDJPY_z".into(), "EUR/USD_z".into(), "".into(), "usdjpy_Z".into()]);
        assert_eq!(p, ["USDJPY_z", "EURUSD_z"]);
    }

    #[test]
    fn env_pairs_get_the_account_suffix_only_when_missing() {
        let p = with_suffix(vec!["USDJPY".into(), "EURUSD_z".into()], Some("_z"));
        assert_eq!(p, ["USDJPY_z", "EURUSD_z"]);
        assert_eq!(with_suffix(vec!["USDJPY".into()], None), ["USDJPY"]);
    }

    #[test]
    fn validate_requires_pairs_and_sane_guard() {
        let mut s = TradingSettings { pairs: vec![], bar_delay_secs: 15, decision_interval_minutes: 5, event_decisions: true, guard: GuardConfig::default() };
        assert!(s.validate().is_err());
        s.pairs = vec!["USDJPY".into(), "EURUSD".into()];
        assert!(s.validate().is_ok());
        s.pairs.push("US DJPY".into());
        assert!(s.validate().is_err());
        s.pairs.pop();
        s.guard.max_positions_total = 0;
        assert!(s.validate().is_err());
        s.guard.max_positions_total = 1;
        s.decision_interval_minutes = 7;
        assert!(s.validate().is_err());
    }

    #[test]
    fn decision_ticks_follow_the_interval_from_utc_midnight() {
        use chrono::TimeZone;
        let mut s = TradingSettings { pairs: vec!["USDJPY".into()], bar_delay_secs: 15, decision_interval_minutes: 60, event_decisions: true, guard: GuardConfig::default() };
        // 足確定の待ち秒数ぶん遅れても、分で判定する
        assert!(s.is_decision_tick(chrono::Utc.with_ymd_and_hms(2026, 10, 5, 9, 0, 15).unwrap()));
        assert!(!s.is_decision_tick(chrono::Utc.with_ymd_and_hms(2026, 10, 5, 9, 5, 15).unwrap()));
        s.decision_interval_minutes = 240;
        assert!(s.is_decision_tick(chrono::Utc.with_ymd_and_hms(2026, 10, 5, 8, 0, 15).unwrap()));
        assert!(!s.is_decision_tick(chrono::Utc.with_ymd_and_hms(2026, 10, 5, 9, 0, 15).unwrap()));
        // 旧い保存値（項目なし）でも読める
        let old: TradingSettings = serde_json::from_str(r#"{"pairs":["USDJPY"],"barDelaySecs":15,"guard":{}}"#).unwrap();
        assert!(!old.event_decisions && old.decision_interval_minutes.is_multiple_of(5));
        assert!(old.guard.max_spread_atr.is_none() && old.guard.max_margin_usage_pct.is_none());
    }

    #[test]
    fn settings_json_keeps_guard_keys_snake_case() {
        let s = TradingSettings { pairs: vec!["USDJPY".into()], bar_delay_secs: 15, decision_interval_minutes: 5, event_decisions: true, guard: GuardConfig::default() };
        let v = serde_json::to_value(&s).unwrap();
        assert!(v.get("barDelaySecs").is_some());
        assert!(v["guard"].get("max_positions_total").is_some());
    }
}
