//! 画面から変更できる取引設定（対象ペア・足確定後の待ち秒数・リスク設定・戦略のデプロイ）。
//!
//! SQLite の `app_settings` に保存し、保存し直すと再起動せずに次の足から反映される。
//! 初回起動時（未保存）は環境変数 `NTRADE_PAIRS` / `NTRADE_BAR_DELAY_SECS` と `config/guard.toml` を初期値にする。
//!
//! ペアはブローカーの銘柄名そのもの（ゼロ口座なら `USDJPY_z`）で持ち、画面の表示・シグナルログ・発注まで同じ名前を使う。
//! 素の `USDJPY` も一覧には存在するが、ゼロ口座では発注が TRADING_DISABLED で拒否されるので区別が要る。

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{info, warn};

use crate::guard::{GuardConfig, DEFAULT_GUARD_CONFIG_PATH};
use crate::storage::Db;
use crate::strategy::StrategySpec;

pub const SETTINGS_KEY: &str = "trading";

/// 初期値の対象銘柄（素の名前。口座のサフィックスは `CTRADER_SYMBOL_SUFFIX` から補う）
pub const DEFAULT_PAIRS: [&str; 3] = ["USDJPY", "EURUSD", "XAUUSD"];

fn default_true() -> bool {
    true
}

/// 戦略を 1 銘柄で動かす設定
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Deployment {
    /// 一意の ID（英数字と - _）。注文の comment と取引履歴の strategy 列に入り、建玉の持ち主を決める
    pub id: String,
    /// 銘柄（`pairs` にあるもの）
    pub pair: String,
    pub strategy: StrategySpec,
    /// 動かすか
    #[serde(default)]
    pub enabled: bool,
    /// true なら発注せず、シグナルを記録するだけ（新しい戦略を本番の足で確かめる用）
    #[serde(default = "default_true")]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingSettings {
    /// 監視する銘柄名（ブローカーの表記どおり。先頭が画面・API の既定ペア）
    pub pairs: Vec<String>,
    /// 足確定からデータ取得までの待ち秒数
    pub bar_delay_secs: i64,
    /// リスク設定（キーは snake_case のまま）
    pub guard: GuardConfig,
    /// 動かす戦略
    #[serde(default)]
    pub deployments: Vec<Deployment>,
}

/// 初期値のペア。`NTRADE_PAIRS` は素の名前で書かれていることがあるので、`CTRADER_SYMBOL_SUFFIX` が
/// あれば補う（ここ以外では補わない。保存後は画面で入力した銘柄名が正）
fn pairs_from_env() -> Vec<String> {
    let suffix = crate::ctrader::symbol_suffix_from_env();
    let pairs = std::env::var("NTRADE_PAIRS")
        .ok()
        .map(|s| normalize_pairs(s.split(',').map(str::to_string).collect()))
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| DEFAULT_PAIRS.iter().map(|p| p.to_string()).collect());
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

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 48 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

impl TradingSettings {
    /// 未保存時の初期値（環境変数と config/guard.toml）。戦略はまだ動かさない
    pub fn from_env_and_file() -> Self {
        Self {
            pairs: pairs_from_env(),
            bar_delay_secs: std::env::var("NTRADE_BAR_DELAY_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(15),
            guard: GuardConfig::load_or_default(DEFAULT_GUARD_CONFIG_PATH),
            deployments: Vec::new(),
        }
    }

    pub fn normalize(&mut self) {
        self.pairs = normalize_pairs(std::mem::take(&mut self.pairs));
        self.guard.normalize();
        for d in self.deployments.iter_mut() {
            d.id = d.id.trim().to_string();
            // 銘柄の表記は pairs にそろえる
            if let Some(p) = self.pairs.iter().find(|p| p.eq_ignore_ascii_case(d.pair.trim())) {
                d.pair = p.clone();
            }
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.pairs.is_empty() {
            bail!("対象ペアを1つ以上指定してください");
        }
        if let Some(bad) = self.pairs.iter().find(|p| !p.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')) {
            bail!("ペア名 {bad} に使えない文字が含まれています");
        }
        if !(0..=240).contains(&self.bar_delay_secs) {
            bail!("足確定後の待ち秒数は 0〜240 秒");
        }
        for (i, d) in self.deployments.iter().enumerate() {
            if !valid_id(&d.id) {
                bail!("戦略 ID「{}」は英数字と - _ の 48 文字以内", d.id);
            }
            if self.deployments[..i].iter().any(|o| o.id == d.id) {
                bail!("戦略 ID「{}」が重複しています", d.id);
            }
            if !self.pairs.iter().any(|p| p.eq_ignore_ascii_case(&d.pair)) {
                bail!("戦略「{}」の銘柄 {} が対象ペアにありません", d.id, d.pair);
            }
            crate::strategy::build(&d.strategy).map_err(|e| anyhow::anyhow!("戦略「{}」: {e:#}", d.id))?;
        }
        self.guard.validate()
    }

    pub fn default_pair(&self) -> String {
        self.pairs.first().cloned().unwrap_or_else(|| "USDJPY".to_string())
    }

    pub fn deployment(&self, id: &str) -> Option<&Deployment> {
        self.deployments.iter().find(|d| d.id == id)
    }

    /// SQLite の保存値を読む。未保存なら初期値を保存してから返す（以後は SQLite が正）
    pub fn load_or_seed(db_path: &Path) -> Self {
        let loaded = Db::open(db_path).and_then(|db| db.load_setting::<TradingSettings>(SETTINGS_KEY));
        match loaded {
            Ok(Some(s)) => {
                info!(pairs = ?s.pairs, deployments = s.deployments.len(), "Loaded trading settings from SQLite");
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
    use crate::ctrader::BarPeriod;

    fn settings() -> TradingSettings {
        TradingSettings { pairs: vec!["USDJPY_z".into()], bar_delay_secs: 15, guard: GuardConfig::default(), deployments: vec![] }
    }

    fn deployment(id: &str, pair: &str) -> Deployment {
        Deployment {
            id: id.into(),
            pair: pair.into(),
            strategy: StrategySpec { name: "ema_cross".into(), timeframe: BarPeriod::M5, params: Default::default() },
            enabled: true,
            dry_run: true,
        }
    }

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
        let mut s = settings();
        s.pairs.clear();
        assert!(s.validate().is_err());
        s.pairs = vec!["USDJPY".into(), "EURUSD".into()];
        assert!(s.validate().is_ok());
        s.pairs.push("US DJPY".into());
        assert!(s.validate().is_err());
        s.pairs.pop();
        s.guard.max_positions_total = 0;
        assert!(s.validate().is_err());
    }

    #[test]
    fn deployments_are_validated_and_aligned_to_pairs() {
        let mut s = settings();
        s.deployments = vec![deployment("ema-usdjpy", "usdjpy_Z")];
        s.normalize();
        assert_eq!(s.deployments[0].pair, "USDJPY_z");
        assert!(s.validate().is_ok());

        s.deployments.push(deployment("ema-usdjpy", "USDJPY_z"));
        assert!(s.validate().unwrap_err().to_string().contains("重複"));
        s.deployments[1].id = "bad id".into();
        assert!(s.validate().is_err());
        s.deployments[1] = deployment("ema-eur", "EURUSD_z");
        assert!(s.validate().unwrap_err().to_string().contains("対象ペア"));
        s.deployments[1] = deployment("ema-2", "USDJPY_z");
        s.deployments[1].strategy.params.insert("fast".into(), 80.0);
        assert!(s.validate().is_err());
    }

    #[test]
    fn llm_era_settings_still_load() {
        // 廃止した項目（判断間隔・終値判定の損切り・確信度の下限）が残った保存値も読める。戦略は空
        let old: TradingSettings = serde_json::from_str(
            r#"{"pairs":["USDJPY_z"],"barDelaySecs":15,"decisionIntervalMinutes":5,"guard":{"min_confidence":0.7,"stop_mode":"close","plan_max_hours":8.0}}"#,
        )
        .unwrap();
        assert!(old.deployments.is_empty());
        assert_eq!(old.guard.order_max_hours, 8.0);
        // 追加したときの既定はドライラン
        let d: Deployment = serde_json::from_str(r#"{"id":"x","pair":"USDJPY_z","strategy":{"name":"ema_cross","timeframe":"M5"}}"#).unwrap();
        assert!(d.dry_run && !d.enabled);
    }

    #[test]
    fn settings_json_keeps_guard_keys_snake_case() {
        let v = serde_json::to_value(settings()).unwrap();
        assert!(v.get("barDelaySecs").is_some());
        assert!(v["guard"].get("max_positions_total").is_some());
    }
}
