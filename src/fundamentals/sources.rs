//! 事実データの取得元。どれも一次情報源の数値・予定・公式発表のタイトルだけを取り、
//! 見解・解説・センチメントの類は取り込まない。

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 時系列の1点
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    /// "YYYY-MM-DD"（観測日。発表日ではない）
    pub date: String,
    pub value: f64,
}

/// 経済・金利の時系列（昇順）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeriesData {
    pub id: String,
    pub title: String,
    pub units: Option<String>,
    pub frequency: Option<String>,
    pub source: String,
    pub observations: Vec<Observation>,
}

/// CFTC 建玉明細の1週
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CotWeek {
    /// 建玉の基準日（火曜）。公表は同じ週の金曜
    pub report_date: String,
    pub long: f64,
    pub short: f64,
    pub open_interest: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CotData {
    pub code: String,
    pub market: String,
    /// 集計した投資家区分（managed_money / noncommercial）
    pub trader_group: String,
    /// 新しい順
    pub weeks: Vec<CotWeek>,
}

/// 経済指標カレンダーの予定1件
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub time: DateTime<Utc>,
    /// 通貨コード（USD / JPY ... / All）
    pub currency: String,
    pub title: String,
    pub impact: Option<String>,
    pub forecast: Option<String>,
    pub previous: Option<String>,
}

/// 公式発表（RSS）1件。本文・要約は使わない
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PressItem {
    pub feed: String,
    pub title: String,
    pub link: Option<String>,
    pub published: DateTime<Utc>,
}

pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("ntrade/0.1 (local research tool)")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("reqwest client")
}

async fn get_text(http: &reqwest::Client, url: &str) -> Result<String> {
    let res = http.get(url).send().await.with_context(|| format!("GET {}", redact(url)))?;
    let status = res.status();
    let body = res.text().await.with_context(|| format!("reading {}", redact(url)))?;
    if !status.is_success() {
        return Err(anyhow!("GET {} -> {status}: {}", redact(url), body.chars().take(200).collect::<String>()));
    }
    Ok(body)
}

/// ログ・エラーに API キーを残さない
fn redact(url: &str) -> String {
    let re = regex::Regex::new(r"(api_key|apikey|\$\$app_token)=[^&]+").unwrap();
    re.replace_all(url, "$1=***").to_string()
}

fn parse_value(s: &str) -> Option<f64> {
    let t = s.trim().trim_matches('"');
    if t.is_empty() || t == "." || t == "-" || t == "N/A" {
        return None;
    }
    t.replace(',', "").parse().ok()
}

// ------------------------------------------------------------------ FRED

#[derive(Deserialize)]
struct FredSeriesRes {
    seriess: Vec<FredSeriesMeta>,
}
#[derive(Deserialize)]
struct FredSeriesMeta {
    title: String,
    units: Option<String>,
    frequency: Option<String>,
}
#[derive(Deserialize)]
struct FredObsRes {
    observations: Vec<FredObs>,
}
#[derive(Deserialize)]
struct FredObs {
    date: String,
    value: String,
}

/// FRED の系列（直近約 400 日分）
pub async fn fred_series(http: &reqwest::Client, api_key: &str, series_id: &str) -> Result<SeriesData> {
    let base = "https://api.stlouisfed.org/fred";
    let meta: FredSeriesRes = serde_json::from_str(
        &get_text(http, &format!("{base}/series?series_id={series_id}&api_key={api_key}&file_type=json")).await?,
    )
    .context("FRED series metadata")?;
    let meta = meta.seriess.into_iter().next().ok_or_else(|| anyhow!("FRED series {series_id} not found"))?;
    let start = (Utc::now() - Duration::days(400)).format("%Y-%m-%d");
    let obs: FredObsRes = serde_json::from_str(
        &get_text(
            http,
            &format!(
                "{base}/series/observations?series_id={series_id}&api_key={api_key}&file_type=json&observation_start={start}&sort_order=asc"
            ),
        )
        .await?,
    )
    .context("FRED observations")?;
    Ok(SeriesData {
        id: format!("fred:{series_id}"),
        title: meta.title,
        units: meta.units,
        frequency: meta.frequency,
        source: "FRED (Federal Reserve Bank of St. Louis)".into(),
        observations: obs
            .observations
            .into_iter()
            .filter_map(|o| Some(Observation { value: parse_value(&o.value)?, date: o.date }))
            .collect(),
    })
}

// ------------------------------------------------------------------ 米財務省（名目・実質利回り）

/// 満期の表記をそろえる（"10Y" / "10 Yr" / "10 YR" → "10yr"、"3M" / "3 Mo" → "3mo"）
fn maturity_key(s: &str) -> String {
    let t = s.trim().trim_matches('"').to_lowercase().replace(' ', "");
    if let Some(n) = t.strip_suffix('y') {
        return format!("{n}yr");
    }
    if let Some(n) = t.strip_suffix('m') {
        return format!("{n}mo");
    }
    t.replace("month", "mo")
}

/// 日付 → 満期 → 利回り（日付は昇順）
pub type Curve = Vec<(NaiveDate, HashMap<String, f64>)>;

fn parse_curve_csv(text: &str, date_fmt: &str) -> Curve {
    let mut lines = text.lines().map(|l| l.trim_start_matches('\u{feff}'));
    let Some(header) = lines.by_ref().find(|l| l.trim_matches('"').starts_with("Date")) else {
        return Vec::new();
    };
    let cols: Vec<String> = header.split(',').map(maturity_key).collect();
    let mut out: Curve = lines
        .filter_map(|l| {
            let cells: Vec<&str> = l.split(',').collect();
            let date = NaiveDate::parse_from_str(cells.first()?.trim().trim_matches('"'), date_fmt).ok()?;
            let values: HashMap<String, f64> = cols
                .iter()
                .zip(cells.iter())
                .skip(1)
                .filter_map(|(c, v)| Some((c.clone(), parse_value(v)?)))
                .collect();
            (!values.is_empty()).then_some((date, values))
        })
        .collect();
    out.sort_by_key(|(d, _)| *d);
    out.dedup_by_key(|(d, _)| *d);
    out
}

/// 米財務省の日次イールドカーブ（今年と前年）。`real` なら実質利回り
pub async fn treasury_curve(http: &reqwest::Client, real: bool) -> Result<Curve> {
    let kind = if real { "daily_treasury_real_yield_curve" } else { "daily_treasury_yield_curve" };
    let year = Utc::now().format("%Y").to_string().parse::<i32>().unwrap_or(2026);
    let mut curve = Vec::new();
    for y in [year - 1, year] {
        let url = format!(
            "https://home.treasury.gov/resource-center/data-chart-center/interest-rates/daily-treasury-rates.csv/{y}/all?type={kind}&field_tdr_date_value={y}&page&_format=csv"
        );
        curve.extend(parse_curve_csv(&get_text(http, &url).await?, "%m/%d/%Y"));
    }
    curve.sort_by_key(|(d, _)| *d);
    curve.dedup_by_key(|(d, _)| *d);
    if curve.is_empty() {
        return Err(anyhow!("treasury {kind}: no rows parsed"));
    }
    Ok(curve)
}

/// 日本財務省の国債金利（全期間ファイル + 当月ファイル）。直近 400 日だけ残す
pub async fn jgb_curve(http: &reqwest::Client) -> Result<Curve> {
    let base = "https://www.mof.go.jp/english/policy/jgbs/reference/interest_rate";
    let mut curve = parse_curve_csv(&get_text(http, &format!("{base}/historical/jgbcme_all.csv")).await?, "%Y/%m/%d");
    match get_text(http, &format!("{base}/jgbcme.csv")).await {
        Ok(t) => curve.extend(parse_curve_csv(&t, "%Y/%m/%d")),
        Err(e) => tracing::warn!("JGB current-month file unavailable: {e:#}"),
    }
    curve.sort_by_key(|(d, _)| *d);
    curve.dedup_by_key(|(d, _)| *d);
    let cutoff = (Utc::now() - Duration::days(400)).date_naive();
    curve.retain(|(d, _)| *d >= cutoff);
    if curve.is_empty() {
        return Err(anyhow!("JGB: no rows parsed"));
    }
    Ok(curve)
}

/// カーブから1満期の系列を切り出す
pub fn series_from_curve(curve: &Curve, id: &str, maturity: &str, title: String, source: &str) -> Option<SeriesData> {
    let key = maturity_key(maturity);
    let observations: Vec<Observation> = curve
        .iter()
        .filter_map(|(d, m)| Some(Observation { date: d.format("%Y-%m-%d").to_string(), value: *m.get(&key)? }))
        .collect();
    (!observations.is_empty()).then(|| SeriesData {
        id: id.to_string(),
        title,
        units: Some("Percent".into()),
        frequency: Some("Daily".into()),
        source: source.to_string(),
        observations,
    })
}

// ------------------------------------------------------------------ EIA

#[derive(Deserialize)]
struct EiaRes {
    response: EiaResponse,
}
#[derive(Deserialize)]
struct EiaResponse {
    data: Vec<EiaRow>,
}
#[derive(Deserialize)]
struct EiaRow {
    period: String,
    value: serde_json::Value,
    #[serde(rename = "series-description")]
    description: Option<String>,
    units: Option<String>,
}

/// EIA API v2 の週次系列（直近 60 週）
pub async fn eia_series(http: &reqwest::Client, api_key: &str, route: &str, series: &str) -> Result<SeriesData> {
    let url = format!(
        "https://api.eia.gov/v2/{route}/data/?api_key={api_key}&frequency=weekly&data[0]=value&facets[series][]={series}&sort[0][column]=period&sort[0][direction]=desc&length=60"
    );
    let res: EiaRes = serde_json::from_str(&get_text(http, &url).await?).context("EIA response")?;
    let title = res.response.data.first().and_then(|r| r.description.clone()).unwrap_or_else(|| series.to_string());
    let units = res.response.data.first().and_then(|r| r.units.clone());
    let mut observations: Vec<Observation> = res
        .response
        .data
        .into_iter()
        .filter_map(|r| {
            let v = match &r.value {
                serde_json::Value::Number(n) => n.as_f64(),
                serde_json::Value::String(s) => parse_value(s),
                _ => None,
            }?;
            Some(Observation { date: r.period, value: v })
        })
        .collect();
    observations.sort_by(|a, b| a.date.cmp(&b.date));
    Ok(SeriesData {
        id: format!("eia:{route}:{series}"),
        title,
        units,
        frequency: Some("Weekly".into()),
        source: "U.S. Energy Information Administration".into(),
        observations,
    })
}

// ------------------------------------------------------------------ CFTC COT

/// 商品は Disaggregated（Managed Money）、金融先物は Legacy（Non-commercial）で集計する
const COT_DISAGGREGATED: &str = "72hh-3qpy";
const COT_LEGACY: &str = "6dca-aqww";

pub async fn cot(http: &reqwest::Client, app_token: Option<&str>, code: &str) -> Result<CotData> {
    let token = app_token.map(|t| format!("&$$app_token={t}")).unwrap_or_default();
    let fetch = |dataset: &'static str, long: &'static str, short: &'static str| {
        let url = format!(
            "https://publicreporting.cftc.gov/resource/{dataset}.json?$select=market_and_exchange_names,report_date_as_yyyy_mm_dd,{long},{short},open_interest_all&$where=cftc_contract_market_code=%27{code}%27&$order=report_date_as_yyyy_mm_dd%20DESC&$limit=52{token}"
        );
        async move {
            let rows: Vec<serde_json::Map<String, serde_json::Value>> =
                serde_json::from_str(&get_text(http, &url).await?).context("CFTC response")?;
            anyhow::Ok(rows)
        }
    };
    let (rows, group, long, short) = {
        let rows = fetch(COT_DISAGGREGATED, "m_money_positions_long_all", "m_money_positions_short_all").await?;
        if rows.is_empty() {
            let rows = fetch(COT_LEGACY, "noncomm_positions_long_all", "noncomm_positions_short_all").await?;
            (rows, "noncommercial", "noncomm_positions_long_all", "noncomm_positions_short_all")
        } else {
            (rows, "managed_money", "m_money_positions_long_all", "m_money_positions_short_all")
        }
    };
    if rows.is_empty() {
        return Err(anyhow!("CFTC: no reports for contract code {code}"));
    }
    let num = |r: &serde_json::Map<String, serde_json::Value>, k: &str| r.get(k).and_then(|v| v.as_str()).and_then(parse_value);
    let market = rows[0].get("market_and_exchange_names").and_then(|v| v.as_str()).unwrap_or(code).to_string();
    let weeks = rows
        .iter()
        .filter_map(|r| {
            Some(CotWeek {
                report_date: r.get("report_date_as_yyyy_mm_dd")?.as_str()?.chars().take(10).collect(),
                long: num(r, long)?,
                short: num(r, short)?,
                open_interest: num(r, "open_interest_all"),
            })
        })
        .collect();
    Ok(CotData { code: code.to_string(), market, trader_group: group.to_string(), weeks })
}

// ------------------------------------------------------------------ 経済指標カレンダー

#[derive(Deserialize)]
struct FfEvent {
    title: String,
    country: String,
    date: String,
    impact: Option<String>,
    forecast: Option<String>,
    previous: Option<String>,
}

pub async fn calendar(http: &reqwest::Client, url: &str) -> Result<Vec<CalendarEvent>> {
    let text = get_text(http, url).await?;
    parse_calendar(&text)
}

fn parse_calendar(text: &str) -> Result<Vec<CalendarEvent>> {
    let events: Vec<FfEvent> = serde_json::from_str(text).context("calendar JSON (rate limited?)")?;
    let non_empty = |s: Option<String>| s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
    let mut out: Vec<CalendarEvent> = events
        .into_iter()
        .filter_map(|e| {
            Some(CalendarEvent {
                time: DateTime::parse_from_rfc3339(&e.date).ok()?.with_timezone(&Utc),
                currency: e.country,
                title: e.title,
                impact: non_empty(e.impact),
                forecast: non_empty(e.forecast),
                previous: non_empty(e.previous),
            })
        })
        .collect();
    out.sort_by_key(|e| e.time);
    Ok(out)
}

// ------------------------------------------------------------------ 公式発表 RSS

pub async fn rss(http: &reqwest::Client, feed: &str, url: &str) -> Result<Vec<PressItem>> {
    Ok(parse_rss(feed, &get_text(http, url).await?))
}

/// RSS 2.0 / RSS 1.0（RDF）/ Atom の item から、タイトル・リンク・日時だけを取り出す
fn parse_rss(feed: &str, xml: &str) -> Vec<PressItem> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(xml.trim_start_matches('\u{feff}'));
    reader.config_mut().trim_text(true);
    let mut items = Vec::new();
    let mut in_item = false;
    let mut field: Option<String> = None;
    let (mut title, mut link, mut date) = (String::new(), None::<String>, None::<String>);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_lowercase();
                if name == "item" || name == "entry" {
                    in_item = true;
                    (title, link, date) = (String::new(), None, None);
                } else if in_item {
                    if name == "link" {
                        // Atom の <link href="..."/> は Empty で来ることが多いが、Start の場合もある
                        if let Some(href) = e.try_get_attribute("href").ok().flatten() {
                            link = Some(String::from_utf8_lossy(&href.value).to_string());
                        }
                    }
                    field = Some(name);
                }
            }
            Ok(Event::Empty(e)) if in_item => {
                if e.local_name().as_ref().eq_ignore_ascii_case(b"link") {
                    if let Some(href) = e.try_get_attribute("href").ok().flatten() {
                        link = Some(String::from_utf8_lossy(&href.value).to_string());
                    }
                }
            }
            Ok(Event::Text(t)) if in_item => {
                let text = t.unescape().map(|c| c.to_string()).unwrap_or_default();
                apply_field(field.as_deref(), &text, &mut title, &mut link, &mut date);
            }
            Ok(Event::CData(t)) if in_item => {
                let text = String::from_utf8_lossy(&t.into_inner()).to_string();
                apply_field(field.as_deref(), &text, &mut title, &mut link, &mut date);
            }
            Ok(Event::End(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_lowercase();
                if (name == "item" || name == "entry") && in_item {
                    in_item = false;
                    if let Some(published) = date.as_deref().and_then(parse_feed_date) {
                        if !title.trim().is_empty() {
                            items.push(PressItem {
                                feed: feed.to_string(),
                                title: title.trim().to_string(),
                                link: link.clone().map(|l| l.trim().to_string()),
                                published,
                            });
                        }
                    }
                }
                field = None;
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                tracing::warn!(feed, "RSS parse error: {e}");
                break;
            }
            _ => {}
        }
    }
    items.sort_by_key(|i| std::cmp::Reverse(i.published));
    items
}

fn apply_field(field: Option<&str>, text: &str, title: &mut String, link: &mut Option<String>, date: &mut Option<String>) {
    match field {
        Some("title") => title.push_str(text),
        Some("link") if link.is_none() => *link = Some(text.to_string()),
        Some("pubdate") | Some("date") | Some("published") | Some("updated") if date.is_none() => {
            *date = Some(text.to_string())
        }
        _ => {}
    }
}

fn parse_feed_date(s: &str) -> Option<DateTime<Utc>> {
    let s = s.trim();
    DateTime::parse_from_rfc2822(s)
        .or_else(|_| DateTime::parse_from_rfc3339(s))
        .map(|d| d.with_timezone(&Utc))
        .ok()
        .or_else(|| {
            NaiveDate::parse_from_str(s.get(..10)?, "%Y-%m-%d").ok().and_then(|d| d.and_hms_opt(0, 0, 0)).map(|d| d.and_utc())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn treasury_and_jgb_csv_are_parsed_by_maturity() {
        let ust = "Date,\"1 Mo\",\"1.5 Month\",\"3 Mo\",\"2 Yr\",\"10 Yr\"\n10/02/2026,4.04,4.09,4.19,4.83,5.28\n10/01/2026,4.06,4.10,4.17,4.78,5.24\n";
        let curve = parse_curve_csv(ust, "%m/%d/%Y");
        assert_eq!(curve.len(), 2);
        let s = series_from_curve(&curve, "ust:10Y", "10Y", "t".into(), "UST").unwrap();
        assert_eq!(s.observations.last().unwrap(), &Observation { date: "2026-10-02".into(), value: 5.28 });
        assert_eq!(series_from_curve(&curve, "ust:3M", "3M", "t".into(), "UST").unwrap().observations[0].value, 4.17);

        let jgb = "Interest Rate (October 2026),,,(Unit : %)\nDate,1Y,2Y,10Y\n2026/10/1,1.668,1.939,3.092\n,,,\n\"note\",,,\n";
        let curve = parse_curve_csv(jgb, "%Y/%m/%d");
        assert_eq!(curve.len(), 1);
        assert_eq!(series_from_curve(&curve, "jgb:2Y", "2Y", "t".into(), "MOF").unwrap().observations[0].value, 1.939);
    }

    #[test]
    fn calendar_keeps_only_scheduled_facts() {
        let json = r#"[{"title":"Non-Farm Employment Change","country":"USD","date":"2026-10-02T08:30:00-04:00","impact":"High","forecast":"120K","previous":"95K"},
                       {"title":"bad","country":"USD","date":"not a date","impact":"Low","forecast":"","previous":""}]"#;
        let ev = parse_calendar(json).unwrap();
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0].time.to_rfc3339(), "2026-10-02T12:30:00+00:00");
        assert_eq!(ev[0].previous.as_deref(), Some("95K"));
    }

    #[test]
    fn rss_items_keep_title_link_and_date_only() {
        let xml = r#"<?xml version="1.0"?><rss version="2.0"><channel><title>FRB</title>
            <item><title>Federal Reserve issues FOMC statement</title>
            <link><![CDATA[https://www.federalreserve.gov/x.htm]]></link>
            <description><![CDATA[commentary that must not leak]]></description>
            <pubDate><![CDATA[Wed, 16 Sep 2026 18:00:00 GMT]]></pubDate></item></channel></rss>"#;
        let items = parse_rss("fed", xml);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Federal Reserve issues FOMC statement");
        assert_eq!(items[0].link.as_deref(), Some("https://www.federalreserve.gov/x.htm"));
        assert_eq!(items[0].published.to_rfc3339(), "2026-09-16T18:00:00+00:00");

        let rdf = r#"<rdf:RDF xmlns:rdf="r" xmlns="http://purl.org/rss/1.0/" xmlns:dc="http://purl.org/dc/elements/1.1/">
            <item rdf:about="u"><title>Statistics release</title><link>https://boj/x</link><dc:date>2026-10-01T08:50:00+09:00</dc:date></item></rdf:RDF>"#;
        let items = parse_rss("boj", rdf);
        assert_eq!(items[0].published.to_rfc3339(), "2026-09-30T23:50:00+00:00");
    }

    #[test]
    fn api_keys_are_redacted() {
        assert_eq!(redact("https://x/?series_id=A&api_key=SECRET&file_type=json"), "https://x/?series_id=A&api_key=***&file_type=json");
    }
}

/// 実際の情報源への疎通確認（ネットワークが要るので既定では走らせない）:
/// `cargo test --lib live_sources -- --ignored --nocapture`
#[cfg(test)]
mod live {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn live_sources() {
        let http = http_client();
        let ust = treasury_curve(&http, false).await.expect("treasury nominal");
        println!("UST 10Y: {:?}", series_from_curve(&ust, "ust:10Y", "10Y", "t".into(), "s").and_then(|s| s.observations.last().cloned()));
        let real = treasury_curve(&http, true).await.expect("treasury real");
        println!("UST real 10Y: {:?}", series_from_curve(&real, "ust_real:10Y", "10Y", "t".into(), "s").and_then(|s| s.observations.last().cloned()));
        let jgb = jgb_curve(&http).await.expect("jgb");
        println!("JGB 10Y: {:?}", series_from_curve(&jgb, "jgb:10Y", "10Y", "t".into(), "s").and_then(|s| s.observations.last().cloned()));
        for code in ["088691", "13874A", "097741"] {
            let c = cot(&http, None, code).await.expect("cftc");
            println!("COT {code} {} {}: {:?}", c.market, c.trader_group, c.weeks.first());
        }
        let cal = calendar(&http, "https://nfs.faireconomy.media/ff_calendar_thisweek.json").await.expect("calendar");
        println!("calendar: {} events, first {:?}", cal.len(), cal.first());
        let cfg = crate::fundamentals::FundamentalsConfig::builtin();
        for (feed, url) in &cfg.feeds {
            match rss(&http, feed, url).await {
                Ok(items) => println!("RSS {feed}: {} items, latest {:?}", items.len(), items.first().map(|i| (&i.title, i.published))),
                Err(e) => println!("RSS {feed}: ERROR {e:#}"),
            }
        }
    }
}
