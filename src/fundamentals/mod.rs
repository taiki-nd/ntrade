//! ファンダメンタルズの事実データ。
//!
//! 公的統計・中央銀行・取引所の建玉報告・公式発表から API で事実だけを集め、判断サイクルの
//! プロンプトに渡す。相場観（上がりそう/下がりそう）は一切付けない。解釈するのは LLM で、
//! LLM 自身は Web にアクセスしない（`llm::client` を参照）。
//!
//! 取得は常駐ループ（`run_loop`）が情報源ごとの間隔で行い、判断サイクルはキャッシュを読むだけ
//! にする（推論の前に外部 API を待たない）。キャッシュは SQLite に保存し、再起動しても取り直さない。

pub mod config;
pub mod sources;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::ctrader::{BarPeriod, CTraderService};
use crate::storage::Db;
pub use config::{FundamentalsConfig, Selection, DEFAULT_FUNDAMENTALS_CONFIG_PATH};
use sources::{CalendarEvent, CotData, Observation, PressItem, SeriesData};

const CACHE_KEY: &str = "fundamentals_cache";
/// カレンダーの配信元は 5 分に 2 回までなので、設定にかかわらずこれより短い間隔では取りにいかない
const MIN_CALENDAR_INTERVAL_MIN: i64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Cached<T> {
    fetched_at: DateTime<Utc>,
    data: T,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Store {
    #[serde(default)]
    series: BTreeMap<String, Cached<SeriesData>>,
    #[serde(default)]
    cot: BTreeMap<String, Cached<CotData>>,
    #[serde(default)]
    calendar: Option<Cached<Vec<CalendarEvent>>>,
    #[serde(default)]
    press: BTreeMap<String, Cached<Vec<PressItem>>>,
    /// 情報源ごとの最後の失敗（成功したら消す）
    #[serde(default)]
    errors: BTreeMap<String, SourceError>,
    /// 情報源グループの最終取得試行（失敗しても間隔を空けるため）
    #[serde(default)]
    attempted: BTreeMap<String, DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceError {
    pub at: DateTime<Utc>,
    pub message: String,
}

/// API キー（.env）
#[derive(Debug, Clone, Default)]
struct ApiKeys {
    fred: Option<String>,
    eia: Option<String>,
    cftc: Option<String>,
}

impl ApiKeys {
    fn from_env() -> Self {
        // 起動順に依らず .env のキーを読めるようにする（既に読み込み済みなら何もしない）
        let _ = dotenvy::dotenv();
        let get = |k: &str| std::env::var(k).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        Self { fred: get("FRED_API_KEY"), eia: get("EIA_API_KEY"), cftc: get("CFTC_APP_TOKEN") }
    }
}

// ------------------------------------------------------------------ LLM に渡す形

/// 時系列の要約（最新値・前回値と、約1か月・3か月・12か月前の値）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeriesFact {
    pub id: String,
    pub title: String,
    pub units: Option<String>,
    pub frequency: Option<String>,
    pub source: String,
    pub latest: Option<Observation>,
    pub previous: Option<Observation>,
    pub about_1m_ago: Option<Observation>,
    pub about_3m_ago: Option<Observation>,
    pub about_12m_ago: Option<Observation>,
}

impl SeriesFact {
    fn from_data(d: &SeriesData) -> Self {
        let obs = &d.observations;
        let latest = obs.last().cloned();
        let ago = |days: i64| -> Option<Observation> {
            let last = chrono::NaiveDate::parse_from_str(&latest.as_ref()?.date, "%Y-%m-%d").ok()?;
            let target = (last - Duration::days(days)).format("%Y-%m-%d").to_string();
            obs.iter().rev().find(|o| o.date <= target).cloned()
        };
        Self {
            id: d.id.clone(),
            title: d.title.clone(),
            units: d.units.clone(),
            frequency: d.frequency.clone(),
            source: d.source.clone(),
            previous: obs.len().checked_sub(2).and_then(|i| obs.get(i)).cloned(),
            about_1m_ago: ago(30),
            about_3m_ago: ago(91),
            about_12m_ago: ago(365),
            latest,
        }
    }
}

/// 建玉明細の要約（直近 4 週と、52 週のネットの高値・安値）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CotFact {
    pub code: String,
    pub market: String,
    pub trader_group: String,
    /// 新しい順
    pub weeks: Vec<CotWeekFact>,
    pub net_52w_high: Option<f64>,
    pub net_52w_low: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CotWeekFact {
    pub report_date: String,
    pub long: f64,
    pub short: f64,
    pub net: f64,
    pub open_interest: Option<f64>,
}

impl CotFact {
    fn from_data(d: &CotData) -> Self {
        let net = |w: &sources::CotWeek| w.long - w.short;
        let nets: Vec<f64> = d.weeks.iter().map(net).collect();
        Self {
            code: d.code.clone(),
            market: d.market.clone(),
            trader_group: d.trader_group.clone(),
            weeks: d
                .weeks
                .iter()
                .take(4)
                .map(|w| CotWeekFact {
                    report_date: w.report_date.clone(),
                    long: w.long,
                    short: w.short,
                    net: net(w),
                    open_interest: w.open_interest,
                })
                .collect(),
            net_52w_high: nets.iter().copied().reduce(f64::max),
            net_52w_low: nets.iter().copied().reduce(f64::min),
        }
    }
}

/// 関連銘柄の日足の騰落（ブローカーの価格）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelatedMarket {
    pub symbol: String,
    pub last_daily_close: f64,
    pub last_daily_bar: String,
    pub change_1d_pct: Option<f64>,
    pub change_5d_pct: Option<f64>,
    pub change_20d_pct: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceStatus {
    pub source: String,
    pub fetched_at: Option<String>,
    pub error: Option<String>,
}

/// 判断に渡すファンダメンタルズの事実一式
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FundamentalFacts {
    pub symbol: String,
    pub as_of: String,
    pub profile: Option<String>,
    pub series: Vec<SeriesFact>,
    pub positioning_cftc: Vec<CotFact>,
    /// 経済指標の予定（直近と今後。時刻は UTC）
    pub economic_calendar: Vec<CalendarFact>,
    /// 中央銀行・統計機関の公式発表（タイトルのみ）
    pub official_releases: Vec<ReleaseFact>,
    pub related_markets: Vec<RelatedMarket>,
    /// 取れなかった情報源（古いデータを使っている、または欠けている）
    pub unavailable: Vec<SourceStatus>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalendarFact {
    pub time_utc: String,
    pub currency: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub impact: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forecast: Option<String>,
    pub previous: Option<String>,
    /// 判断時点で予定時刻を過ぎているか（結果の数値はこのフィードには無い）
    pub released: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReleaseFact {
    pub source: String,
    pub published_utc: String,
    pub title: String,
}

impl FundamentalFacts {
    pub fn is_empty(&self) -> bool {
        self.series.is_empty()
            && self.positioning_cftc.is_empty()
            && self.economic_calendar.is_empty()
            && self.official_releases.is_empty()
            && self.related_markets.is_empty()
    }

    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

// ------------------------------------------------------------------ サービス

pub struct FundamentalsService {
    config: RwLock<FundamentalsConfig>,
    http: reqwest::Client,
    keys: ApiKeys,
    db_path: PathBuf,
    store: RwLock<Store>,
    /// 発表後判断を出した（銘柄, 指標）。同じ発表で何度も判断しないため
    fired_events: std::sync::Mutex<HashSet<String>>,
}

impl FundamentalsService {
    pub fn new(db_path: PathBuf) -> Self {
        let config = FundamentalsConfig::load_or_builtin(DEFAULT_FUNDAMENTALS_CONFIG_PATH);
        let store = Db::open(&db_path)
            .and_then(|db| db.load_cache::<Store>(CACHE_KEY))
            .unwrap_or_else(|e| {
                warn!("Failed to load fundamentals cache: {e:#}");
                None
            })
            .unwrap_or_default();
        let keys = ApiKeys::from_env();
        info!(
            enabled = config.enabled,
            fred_key = keys.fred.is_some(),
            eia_key = keys.eia.is_some(),
            cached_series = store.series.len(),
            "fundamentals service initialised"
        );
        Self {
            config: RwLock::new(config),
            http: sources::http_client(),
            keys,
            db_path,
            store: RwLock::new(store),
            fired_events: std::sync::Mutex::new(HashSet::new()),
        }
    }

    pub async fn config(&self) -> FundamentalsConfig {
        self.config.read().await.clone()
    }

    /// 期限の来た情報源を取り直す
    pub async fn refresh_due(&self) {
        let cfg = self.config.read().await.clone();
        if !cfg.enabled {
            return;
        }
        let now = Utc::now();
        let due = |store: &Store, group: &str, minutes: i64| {
            store.attempted.get(group).is_none_or(|t| now - *t >= Duration::minutes(minutes.max(1)))
        };
        let (series_due, cot_due, cal_due, press_due) = {
            let s = self.store.read().await;
            (
                due(&s, "series", cfg.refresh_minutes.series),
                due(&s, "cot", cfg.refresh_minutes.cot),
                due(&s, "calendar", cfg.refresh_minutes.calendar.max(MIN_CALENDAR_INTERVAL_MIN)),
                due(&s, "press", cfg.refresh_minutes.press),
            )
        };
        if !(series_due || cot_due || cal_due || press_due) {
            return;
        }
        if series_due {
            self.refresh_series(&cfg).await;
        }
        if cot_due {
            self.refresh_cot(&cfg).await;
        }
        if cal_due {
            self.refresh_calendar(&cfg).await;
        }
        if press_due {
            self.refresh_press(&cfg).await;
        }
        self.persist().await;
    }

    async fn mark_attempt(&self, group: &str) {
        self.store.write().await.attempted.insert(group.to_string(), Utc::now());
    }

    async fn record<T>(&self, source: &str, result: anyhow::Result<T>) -> Option<T> {
        let mut store = self.store.write().await;
        match result {
            Ok(v) => {
                store.errors.remove(source);
                Some(v)
            }
            Err(e) => {
                warn!(source, "fundamentals fetch failed: {e:#}");
                store.errors.insert(source.to_string(), SourceError { at: Utc::now(), message: format!("{e:#}") });
                None
            }
        }
    }

    async fn put_series(&self, data: SeriesData) {
        let now = Utc::now();
        self.store.write().await.series.insert(data.id.clone(), Cached { fetched_at: now, data });
    }

    async fn refresh_series(&self, cfg: &FundamentalsConfig) {
        self.mark_attempt("series").await;
        let wanted = cfg.all_series();
        let has = |prefix: &str| wanted.iter().any(|s| s.starts_with(prefix));

        // 米財務省（名目・実質）と日本財務省は1回の取得で全満期が取れる
        for (real, prefix, title, enabled) in [
            (false, "ust:", "U.S. Treasury par yield (nominal)", cfg.sources.treasury),
            (true, "ust_real:", "U.S. Treasury real yield (TIPS)", cfg.sources.treasury),
        ] {
            if !enabled || !has(prefix) {
                continue;
            }
            let source = format!("treasury{}", if real { "_real" } else { "" });
            if let Some(curve) = self.record(&source, sources::treasury_curve(&self.http, real).await).await {
                for id in wanted.iter().filter(|s| s.starts_with(prefix)) {
                    let m = &id[prefix.len()..];
                    match sources::series_from_curve(&curve, id, m, format!("{title} {m}"), "U.S. Department of the Treasury") {
                        Some(d) => self.put_series(d).await,
                        None => warn!(series = %id, "maturity not found in Treasury curve"),
                    }
                }
            }
        }
        if cfg.sources.jgb && has("jgb:") {
            if let Some(curve) = self.record("jgb", sources::jgb_curve(&self.http).await).await {
                for id in wanted.iter().filter(|s| s.starts_with("jgb:")) {
                    let m = &id[4..];
                    match sources::series_from_curve(&curve, id, m, format!("Japanese Government Bond yield {m}"), "Ministry of Finance Japan") {
                        Some(d) => self.put_series(d).await,
                        None => warn!(series = %id, "maturity not found in JGB curve"),
                    }
                }
            }
        }
        if cfg.sources.fred {
            match &self.keys.fred {
                Some(key) => {
                    for id in wanted.iter().filter(|s| s.starts_with("fred:")) {
                        let res = sources::fred_series(&self.http, key, &id[5..]).await;
                        if let Some(d) = self.record(id, res).await {
                            self.put_series(d).await;
                        }
                        // FRED の制限（120 回/分）に余裕を持たせる
                        tokio::time::sleep(std::time::Duration::from_millis(600)).await;
                    }
                }
                None if has("fred:") => {
                    let _ = self.record::<()>("fred", Err(anyhow::anyhow!("FRED_API_KEY is not set"))).await;
                }
                None => {}
            }
        }
        if cfg.sources.eia {
            match &self.keys.eia {
                Some(key) => {
                    for id in wanted.iter().filter(|s| s.starts_with("eia:")) {
                        let Some((route, series)) = id[4..].rsplit_once(':') else {
                            warn!(series = %id, "EIA series id must be eia:<route>:<SERIES>");
                            continue;
                        };
                        let res = sources::eia_series(&self.http, key, route, series).await;
                        if let Some(d) = self.record(id, res).await {
                            self.put_series(d).await;
                        }
                    }
                }
                None if has("eia:") => {
                    let _ = self.record::<()>("eia", Err(anyhow::anyhow!("EIA_API_KEY is not set"))).await;
                }
                None => {}
            }
        }
    }

    async fn refresh_cot(&self, cfg: &FundamentalsConfig) {
        if !cfg.sources.cftc {
            return;
        }
        self.mark_attempt("cot").await;
        for code in cfg.all_cot() {
            let res = sources::cot(&self.http, self.keys.cftc.as_deref(), &code).await;
            if let Some(d) = self.record(&format!("cftc:{code}"), res).await {
                self.store.write().await.cot.insert(code.clone(), Cached { fetched_at: Utc::now(), data: d });
            }
        }
    }

    async fn refresh_calendar(&self, cfg: &FundamentalsConfig) {
        if !cfg.sources.calendar {
            return;
        }
        self.mark_attempt("calendar").await;
        let res = sources::calendar(&self.http, &cfg.calendar.url).await;
        if let Some(events) = self.record("calendar", res).await {
            let mut store = self.store.write().await;
            // 配信元は今週分しか出さないので、週をまたいでも直近の予定が消えないよう前回分とまとめる
            let horizon = Utc::now() - Duration::days(8);
            let mut merged: Vec<CalendarEvent> = store
                .calendar
                .take()
                .map(|c| c.data)
                .unwrap_or_default()
                .into_iter()
                .filter(|e| e.time >= horizon && !events.iter().any(|n| n.time == e.time && n.title == e.title && n.currency == e.currency))
                .collect();
            merged.extend(events);
            merged.sort_by_key(|e| e.time);
            store.calendar = Some(Cached { fetched_at: Utc::now(), data: merged });
        }
    }

    async fn refresh_press(&self, cfg: &FundamentalsConfig) {
        if !cfg.sources.press {
            return;
        }
        self.mark_attempt("press").await;
        for (feed, url) in &cfg.feeds {
            let res = sources::rss(&self.http, feed, url).await;
            if let Some(items) = self.record(&format!("press:{feed}"), res).await {
                self.store.write().await.press.insert(feed.clone(), Cached { fetched_at: Utc::now(), data: items });
            }
        }
    }

    async fn persist(&self) {
        let store = self.store.read().await.clone();
        let path = self.db_path.clone();
        let res = tokio::task::spawn_blocking(move || Db::open(path)?.save_cache(CACHE_KEY, &store)).await;
        match res {
            Ok(Ok(())) => {}
            Ok(Err(e)) => warn!("Failed to persist fundamentals cache: {e:#}"),
            Err(e) => warn!("Failed to persist fundamentals cache: {e}"),
        }
    }

    /// 銘柄の事実一式（キャッシュから組み立てる。関連銘柄の騰落だけはブローカーから引く）
    pub async fn facts_for(
        &self,
        symbol: &str,
        ctrader: Option<&CTraderService>,
        now: DateTime<Utc>,
    ) -> FundamentalFacts {
        let cfg = self.config.read().await.clone();
        if !cfg.enabled {
            return FundamentalFacts { symbol: symbol.to_string(), as_of: fmt(now), ..Default::default() };
        }
        let spec = crate::instrument::get(symbol);
        let sel = cfg.select(
            symbol,
            spec.as_ref().and_then(|s| s.base_asset.as_deref()),
            spec.as_ref().and_then(|s| s.quote_asset.as_deref()),
        );
        let mut facts = self.cached_facts(symbol, &sel, &cfg, now).await;
        if let Some(c) = ctrader {
            facts.related_markets = related_markets(c, symbol, &sel.related).await;
        }
        facts
    }

    async fn cached_facts(&self, symbol: &str, sel: &Selection, cfg: &FundamentalsConfig, now: DateTime<Utc>) -> FundamentalFacts {
        let store = self.store.read().await;
        let mut unavailable = Vec::new();
        let mut missing = |source: &str, fetched: Option<DateTime<Utc>>| {
            let error = store.errors.get(source).map(|e| e.message.clone());
            if fetched.is_none() || error.is_some() {
                unavailable.push(SourceStatus { source: source.to_string(), fetched_at: fetched.map(fmt), error });
            }
        };

        let series = sel
            .series
            .iter()
            .filter_map(|id| {
                let c = store.series.get(id);
                missing(id, c.map(|c| c.fetched_at));
                c.map(|c| SeriesFact::from_data(&c.data))
            })
            .collect();
        let positioning_cftc = sel
            .cot
            .iter()
            .filter_map(|code| {
                let c = store.cot.get(code);
                missing(&format!("cftc:{code}"), c.map(|c| c.fetched_at));
                c.map(|c| CotFact::from_data(&c.data))
            })
            .collect();

        let from = now - Duration::hours(cfg.calendar.lookback_hours);
        let to = now + Duration::hours(cfg.calendar.lookahead_hours);
        let economic_calendar = match &store.calendar {
            Some(c) if !sel.calendar.is_empty() => {
                missing("calendar", Some(c.fetched_at));
                c.data
                    .iter()
                    .filter(|e| e.time >= from && e.time <= to)
                    .filter(|e| sel.calendar.iter().any(|ccy| ccy.eq_ignore_ascii_case(&e.currency)))
                    .map(|e| CalendarFact {
                        time_utc: fmt(e.time),
                        currency: e.currency.clone(),
                        title: e.title.clone(),
                        impact: e.impact.clone().filter(|_| cfg.calendar.include_impact),
                        forecast: e.forecast.clone().filter(|_| cfg.calendar.include_forecast),
                        previous: e.previous.clone(),
                        released: e.time <= now,
                    })
                    .collect()
            }
            None if !sel.calendar.is_empty() && cfg.sources.calendar => {
                missing("calendar", None);
                Vec::new()
            }
            _ => Vec::new(),
        };

        let since = now - Duration::days(cfg.press.lookback_days);
        let mut official_releases: Vec<ReleaseFact> = sel
            .press
            .iter()
            .flat_map(|feed| {
                let c = store.press.get(feed);
                missing(&format!("press:{feed}"), c.map(|c| c.fetched_at));
                c.map(|c| c.data.clone()).unwrap_or_default()
            })
            .filter(|p| p.published >= since && p.published <= now)
            .map(|p| ReleaseFact { source: p.feed, published_utc: fmt(p.published), title: p.title })
            .collect();
        official_releases.sort_by(|a, b| b.published_utc.cmp(&a.published_utc));
        official_releases.truncate(cfg.press.max_items);

        // キー未設定などグループ単位の失敗も載せる
        for group in ["fred", "eia"] {
            if let Some(e) = store.errors.get(group) {
                if sel.series.iter().any(|s| s.starts_with(&format!("{group}:"))) {
                    unavailable.push(SourceStatus { source: group.into(), fetched_at: None, error: Some(e.message.clone()) });
                }
            }
        }

        FundamentalFacts {
            symbol: symbol.to_string(),
            as_of: fmt(now),
            profile: sel.profile.clone(),
            series,
            positioning_cftc,
            economic_calendar,
            official_releases,
            related_markets: Vec::new(),
            unavailable,
        }
    }

    /// 重要指標の発表直後か（発表時刻から 5M 足が1本確定した後、`window_minutes` 以内）。
    /// 同じ（銘柄, 指標）では一度だけ true を返す
    pub async fn take_event_trigger(&self, symbol: &str, now: DateTime<Utc>) -> Option<CalendarEvent> {
        let cfg = self.config.read().await.clone();
        if !cfg.enabled {
            return None;
        }
        let spec = crate::instrument::get(symbol);
        let sel = cfg.select(
            symbol,
            spec.as_ref().and_then(|s| s.base_asset.as_deref()),
            spec.as_ref().and_then(|s| s.quote_asset.as_deref()),
        );
        let store = self.store.read().await;
        let events = &store.calendar.as_ref()?.data;
        let window = Duration::minutes(cfg.event_trigger.window_minutes.max(5));
        let hit = events.iter().find(|e| {
            let since = now - e.time;
            since >= Duration::minutes(5)
                && since < window
                && sel.calendar.iter().any(|c| c.eq_ignore_ascii_case(&e.currency))
                && e.impact.as_deref().is_some_and(|i| cfg.event_trigger.impacts.iter().any(|x| x.eq_ignore_ascii_case(i)))
        })?;
        let key = format!("{}|{}|{}|{}", symbol.to_uppercase(), hit.currency, hit.title, hit.time.timestamp());
        let mut fired = self.fired_events.lock().unwrap_or_else(|e| e.into_inner());
        fired.retain(|k| k.rsplit('|').next().and_then(|t| t.parse::<i64>().ok()).is_some_and(|t| now.timestamp() - t < 86_400));
        fired.insert(key).then(|| hit.clone())
    }

    /// 情報源ごとの取得状況（画面表示用）
    pub async fn status(&self) -> Vec<SourceStatus> {
        let store = self.store.read().await;
        let mut out: Vec<SourceStatus> = store
            .series
            .iter()
            .map(|(k, c)| (k.clone(), Some(c.fetched_at)))
            .chain(store.cot.iter().map(|(k, c)| (format!("cftc:{k}"), Some(c.fetched_at))))
            .chain(store.calendar.iter().map(|c| ("calendar".to_string(), Some(c.fetched_at))))
            .chain(store.press.iter().map(|(k, c)| (format!("press:{k}"), Some(c.fetched_at))))
            .map(|(source, at)| SourceStatus {
                error: store.errors.get(&source).map(|e| e.message.clone()),
                source,
                fetched_at: at.map(fmt),
            })
            .collect();
        for (source, e) in &store.errors {
            if !out.iter().any(|s| &s.source == source) {
                out.push(SourceStatus { source: source.clone(), fetched_at: None, error: Some(e.message.clone()) });
            }
        }
        out.sort_by(|a, b| a.source.cmp(&b.source));
        out
    }
}

fn fmt(t: DateTime<Utc>) -> String {
    t.format("%Y-%m-%d %H:%M UTC").to_string()
}

/// 関連銘柄の名前をブローカーの表記に解決する。対象銘柄の口座サフィックス（`_z` など）を補ったものを先に試す
async fn resolve_related(ctrader: &CTraderService, symbol: &str, related: &str) -> Option<String> {
    let suffix = symbol.find(['_', '.']).map(|i| &symbol[i..]).unwrap_or("");
    if !suffix.is_empty() {
        if let Ok(name) = ctrader.canonical_symbol(&format!("{related}{suffix}")).await {
            return Some(name);
        }
    }
    ctrader.canonical_symbol(related).await.ok()
}

async fn related_markets(ctrader: &CTraderService, symbol: &str, related: &[String]) -> Vec<RelatedMarket> {
    let mut out = Vec::new();
    for r in related {
        let Some(name) = resolve_related(ctrader, symbol, r).await else { continue };
        match ctrader.get_trendbars(&name, BarPeriod::D1, 25).await {
            Ok(bars) if !bars.is_empty() => {
                let last = bars.last().expect("non-empty");
                let pct = |n: usize| {
                    bars.len()
                        .checked_sub(n + 1)
                        .and_then(|i| bars.get(i))
                        .filter(|b| b.close > 0.0)
                        .map(|b| ((last.close / b.close - 1.0) * 10_000.0).round() / 100.0)
                };
                out.push(RelatedMarket {
                    symbol: name,
                    last_daily_close: last.close,
                    last_daily_bar: last.timestamp.format("%Y-%m-%d").to_string(),
                    change_1d_pct: pct(1),
                    change_5d_pct: pct(5),
                    change_20d_pct: pct(20),
                });
            }
            Ok(_) => {}
            Err(e) => warn!(symbol = %name, "failed to fetch related market bars: {e:#}"),
        }
    }
    out
}

/// 定期取得の常駐ループ（5 分ごとに期限を確認）
pub async fn run_loop(service: Arc<FundamentalsService>) {
    loop {
        service.refresh_due().await;
        tokio::time::sleep(std::time::Duration::from_secs(300)).await;
    }
}

/// 情報源の一覧（ドキュメント・画面用）
pub fn source_catalog() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("fred", "FRED（米セントルイス連銀）: 政策金利・物価・雇用・ドル指数など。要 FRED_API_KEY"),
        ("treasury", "米財務省: 名目・実質（TIPS）利回りの日次カーブ"),
        ("jgb", "日本財務省: 国債金利の日次カーブ"),
        ("cftc", "CFTC 建玉明細（COT）: 投機筋のロング・ショート"),
        ("eia", "EIA: 原油在庫など週次統計。要 EIA_API_KEY"),
        ("calendar", "経済指標の予定（時刻・指標名・前回値）"),
        ("press", "中央銀行・統計機関の公式発表のタイトル"),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(date: &str, value: f64) -> Observation {
        Observation { date: date.into(), value }
    }

    #[test]
    fn series_fact_picks_lookbacks_on_or_before_the_target_date() {
        let d = SeriesData {
            id: "ust:10Y".into(),
            title: "t".into(),
            units: None,
            frequency: None,
            source: "s".into(),
            observations: vec![obs("2025-10-01", 4.0), obs("2026-07-01", 4.8), obs("2026-09-01", 5.0), obs("2026-09-30", 5.2), obs("2026-10-01", 5.24)],
        };
        let f = SeriesFact::from_data(&d);
        assert_eq!(f.latest, Some(obs("2026-10-01", 5.24)));
        assert_eq!(f.previous, Some(obs("2026-09-30", 5.2)));
        assert_eq!(f.about_1m_ago, Some(obs("2026-09-01", 5.0)));
        assert_eq!(f.about_3m_ago, Some(obs("2026-07-01", 4.8)));
        assert_eq!(f.about_12m_ago, Some(obs("2025-10-01", 4.0)));
    }

    #[test]
    fn cot_fact_computes_net_and_range() {
        let w = |d: &str, l: f64, s: f64| sources::CotWeek { report_date: d.into(), long: l, short: s, open_interest: None };
        let f = CotFact::from_data(&CotData {
            code: "088691".into(),
            market: "GOLD".into(),
            trader_group: "managed_money".into(),
            weeks: vec![w("2026-09-29", 131_711.0, 11_393.0), w("2026-09-22", 120_000.0, 20_000.0), w("2026-01-06", 50_000.0, 40_000.0)],
        });
        assert_eq!(f.weeks[0].net, 120_318.0);
        assert_eq!(f.net_52w_high, Some(120_318.0));
        assert_eq!(f.net_52w_low, Some(10_000.0));
    }

    #[tokio::test]
    async fn facts_filter_calendar_by_currency_and_hide_forecasts() {
        let db = std::env::temp_dir().join(format!("ntrade_fund_{}.db", std::process::id()));
        let svc = FundamentalsService::new(db.clone());
        let now = Utc::now();
        let ev = |ccy: &str, mins: i64, impact: &str| CalendarEvent {
            time: now + Duration::minutes(mins),
            currency: ccy.into(),
            title: format!("{ccy} CPI"),
            impact: Some(impact.into()),
            forecast: Some("0.3%".into()),
            previous: Some("0.2%".into()),
        };
        svc.store.write().await.calendar =
            Some(Cached { fetched_at: now, data: vec![ev("USD", -7, "High"), ev("EUR", 60, "High"), ev("JPY", 120, "Low")] });

        let facts = svc.facts_for("USDJPY", None, now).await;
        let titles: Vec<&str> = facts.economic_calendar.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(titles, ["USD CPI", "JPY CPI"]);
        assert!(facts.economic_calendar.iter().all(|e| e.forecast.is_none()), "forecast must be hidden by default");
        assert!(facts.economic_calendar[0].released && !facts.economic_calendar[1].released);

        // 発表から 5M 足 1 本確定後に一度だけ発火
        assert!(svc.take_event_trigger("USDJPY", now).await.is_some());
        assert!(svc.take_event_trigger("USDJPY", now).await.is_none());
        assert!(svc.take_event_trigger("EURGBP", now).await.is_none());
        let _ = std::fs::remove_file(db);
    }
}

/// 実データでの組み立て確認（ネットワークが要るので既定では走らせない）:
/// `cargo test --lib live_facts -- --ignored --nocapture`
#[cfg(test)]
mod live {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn live_facts() {
        let db = std::env::temp_dir().join(format!("ntrade_fund_live_{}.db", std::process::id()));
        let svc = FundamentalsService::new(db.clone());
        svc.refresh_due().await;
        for symbol in ["USDJPY_z", "XAUUSD_z", "US500_z", "XTIUSD_z"] {
            let facts = svc.facts_for(symbol, None, Utc::now()).await;
            let json = facts.to_json_pretty();
            println!(
                "{symbol}: profile={:?} series={} cot={} calendar={} releases={} unavailable={:?} json_chars={}",
                facts.profile,
                facts.series.len(),
                facts.positioning_cftc.len(),
                facts.economic_calendar.len(),
                facts.official_releases.len(),
                facts.unavailable.iter().map(|u| u.source.as_str()).collect::<Vec<_>>(),
                json.chars().count()
            );
        }
        println!("{}", svc.facts_for("XAUUSD_z", None, Utc::now()).await.to_json_pretty().chars().take(2500).collect::<String>());
        let _ = std::fs::remove_file(db);
    }
}
