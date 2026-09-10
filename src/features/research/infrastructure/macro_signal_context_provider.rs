use crate::config::AppConfig;
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

/// Research ACL に渡す前の provider 内部 read model。Radar の型を参照しない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MacroSignalContextProviderReadModel {
    pub market_date: NaiveDate,
    pub rates_credit: MacroSignalContextProviderSource,
    pub commodity: MacroSignalContextProviderSource,
    pub geopolitical: MacroSignalContextProviderSource,
    pub observed_market_reactions: Vec<ProviderMarketReaction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct MacroSignalContextProviderSource {
    pub status: MacroSignalContextProviderSourceStatus,
    pub events: Vec<MacroSignalContextProviderEvent>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum MacroSignalContextProviderSourceStatus {
    Healthy,
    Partial,
    Degraded,
    #[default]
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum MacroSignalContextProviderInformationLevel {
    High,
    Medium,
    Low,
    #[default]
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum MacroSignalContextProviderLifecycle {
    ActiveRepricing,
    Aftermath,
    #[default]
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct MacroSignalContextProviderEvent {
    pub event_id: String,
    pub accepted_at: String,
    pub title: String,
    pub information_content: MacroSignalContextProviderInformationLevel,
    pub market_relevance: MacroSignalContextProviderInformationLevel,
    pub evidence_quality: MacroSignalContextProviderInformationLevel,
    pub lifecycle: MacroSignalContextProviderLifecycle,
    pub event_fact: String,
    pub observed_at: String,
    pub source_published_at: String,
    pub market_date: String,
    pub evidence: Vec<ProviderEvidenceRecord>,
    pub expected_value: Option<String>,
    pub actual_value: Option<String>,
    pub surprise: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ProviderEvidenceRecord {
    pub source: String,
    pub source_url: String,
    pub timestamp: String,
    pub source_published_at: String,
    pub event_type: String,
    pub subject: String,
    pub importance: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ProviderMarketReaction {
    pub observation_id: String,
    pub observed_at: String,
    pub session: String,
    pub venue: String,
    pub instrument: String,
    pub source_published_at: String,
    pub market_date: String,
    pub subject: String,
    pub observation: String,
    pub evidence: Vec<ProviderEvidenceRecord>,
}

const FRED_API_BASE: &str = "https://api.stlouisfed.org/fred/series/observations";
const FINNHUB_NEWS_URL: &str = "https://finnhub.io/api/v1/news";
const FRED_SERIES_RATES: &[(&str, &str)] = &[
    ("DGS10", "US 10Y Treasury yield"),
    ("DGS2", "US 2Y Treasury yield"),
    ("BAMLH0A0HYM2", "US high-yield option-adjusted spread"),
];
const FRED_SERIES_COMMODITY: &[(&str, &str)] = &[
    ("DCOILBRENTEU", "Brent crude oil"),
    ("DCOILWTICO", "WTI crude oil"),
];
const GEOPOLITICAL_KEYWORDS: &[&str] = &[
    "attack",
    "conflict",
    "escalat",
    "energy facility",
    "houthi",
    "iran",
    "israel",
    "military",
    "missile",
    "refinery",
    "saudi",
    "strike",
    "war",
];

fn stable_id(namespace: &str, parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(namespace.as_bytes());
    for part in parts {
        hasher.update([0]);
        hasher.update(part.as_bytes());
    }
    format!("{namespace}:{:x}", hasher.finalize())
}

#[derive(Debug, Clone, PartialEq)]
struct ParsedFredSeries {
    latest_date: NaiveDate,
    latest_value: f64,
    previous_date: NaiveDate,
    previous_value: f64,
}

#[derive(Debug, Deserialize)]
struct FredResponse {
    observations: Vec<FredObservation>,
}

#[derive(Debug, Deserialize)]
struct FredObservation {
    date: NaiveDate,
    value: String,
}

#[derive(Debug, Clone)]
struct FredSeriesResult {
    series_id: &'static str,
    label: &'static str,
    observation: ParsedFredSeries,
}

pub(crate) async fn load_macro_signal_context(
    app_config: &AppConfig,
    market_date: NaiveDate,
    report_run_at: DateTime<Utc>,
) -> MacroSignalContextProviderReadModel {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
    {
        Ok(client) => client,
        Err(err) => {
            return MacroSignalContextProviderReadModel {
                market_date,
                rates_credit: unavailable_source(format!("HTTP client unavailable: {err}")),
                commodity: unavailable_source("HTTP client unavailable".to_string()),
                geopolitical: unavailable_source("HTTP client unavailable".to_string()),
                observed_market_reactions: Vec::new(),
            }
        }
    };
    let rates = load_fred_source(
        &client,
        app_config
            .fred
            .as_ref()
            .map(|config| config.fred_api_key.as_str()),
        market_date,
        FRED_SERIES_RATES,
        "RATES_CREDIT",
        rate_information_level,
        report_run_at.to_rfc3339().as_str(),
    )
    .await;
    let commodity = load_fred_source(
        &client,
        app_config
            .fred
            .as_ref()
            .map(|config| config.fred_api_key.as_str()),
        market_date,
        FRED_SERIES_COMMODITY,
        "COMMODITY_OIL",
        commodity_information_level,
        report_run_at.to_rfc3339().as_str(),
    )
    .await;
    let geopolitical = load_finnhub_geopolitical_source(
        &client,
        app_config
            .finnhub
            .as_ref()
            .map(|config| config.finnhub_api_key.as_str()),
        market_date,
        report_run_at.to_rfc3339().as_str(),
    )
    .await;
    let observed_market_reactions = rates.1.iter().chain(commodity.1.iter()).cloned().collect();
    MacroSignalContextProviderReadModel {
        market_date,
        rates_credit: rates.0,
        commodity: commodity.0,
        geopolitical: geopolitical.0,
        observed_market_reactions,
    }
}

async fn load_fred_source(
    client: &reqwest::Client,
    api_key: Option<&str>,
    market_date: NaiveDate,
    series: &[(&'static str, &'static str)],
    event_type: &'static str,
    information_level: fn(&[FredSeriesResult]) -> MacroSignalContextProviderInformationLevel,
    accepted_at: &str,
) -> (
    MacroSignalContextProviderSource,
    Vec<ProviderMarketReaction>,
) {
    let Some(api_key) = api_key.filter(|key| !key.trim().is_empty()) else {
        return (
            unavailable_source("FRED API key is not configured".to_string()),
            Vec::new(),
        );
    };

    let mut results = Vec::new();
    let mut diagnostics = Vec::new();
    for &(series_id, label) in series {
        match fetch_fred_series(client, api_key, market_date, series_id).await {
            Ok(observation) => results.push(FredSeriesResult {
                series_id,
                label,
                observation,
            }),
            Err(err) => diagnostics.push(format!("FRED {series_id}: {err}")),
        }
    }
    if results.is_empty() {
        return (
            MacroSignalContextProviderSource {
                status: MacroSignalContextProviderSourceStatus::Degraded,
                events: Vec::new(),
                diagnostics,
            },
            Vec::new(),
        );
    }
    let status = if diagnostics.is_empty() {
        MacroSignalContextProviderSourceStatus::Healthy
    } else {
        MacroSignalContextProviderSourceStatus::Partial
    };
    let level = information_level(&results);
    let event = build_fred_event(market_date, event_type, level, &results, accepted_at);
    let reactions = build_fred_reactions(market_date, event_type, &results);
    (
        MacroSignalContextProviderSource {
            status,
            events: vec![event],
            diagnostics,
        },
        reactions,
    )
}

async fn fetch_fred_series(
    client: &reqwest::Client,
    api_key: &str,
    market_date: NaiveDate,
    series_id: &str,
) -> Result<ParsedFredSeries> {
    let start_date = market_date - Duration::days(7);
    let response = client
        .get(FRED_API_BASE)
        .query(&[
            ("series_id", series_id),
            ("api_key", api_key),
            ("file_type", "json"),
            ("observation_start", start_date.to_string().as_str()),
            ("observation_end", market_date.to_string().as_str()),
        ])
        .send()
        .await
        .map_err(|_| anyhow!("FRED request failed for {series_id}"))?;
    if !response.status().is_success() {
        return Err(anyhow!("FRED returned {}", response.status()));
    }
    let raw = response
        .text()
        .await
        .context("FRED response body unavailable")?;
    parse_fred_series_observations(&raw, market_date)
}

fn parse_fred_series_observations(raw: &str, market_date: NaiveDate) -> Result<ParsedFredSeries> {
    let response: FredResponse = serde_json::from_str(raw).context("invalid FRED JSON")?;
    let mut observations = response
        .observations
        .into_iter()
        .filter_map(|observation| {
            if observation.date > market_date || observation.value.trim() == "." {
                return None;
            }
            let value = observation.value.parse::<f64>().ok()?;
            value.is_finite().then_some((observation.date, value))
        })
        .collect::<Vec<_>>();
    observations.sort_by_key(|left| std::cmp::Reverse(left.0));
    let [(latest_date, latest_value), (previous_date, previous_value), ..] =
        observations.as_slice()
    else {
        return Err(anyhow!("FRED series has fewer than two valid observations"));
    };
    Ok(ParsedFredSeries {
        latest_date: *latest_date,
        latest_value: *latest_value,
        previous_date: *previous_date,
        previous_value: *previous_value,
    })
}

fn build_fred_event(
    market_date: NaiveDate,
    event_type: &str,
    level: MacroSignalContextProviderInformationLevel,
    results: &[FredSeriesResult],
    accepted_at: &str,
) -> MacroSignalContextProviderEvent {
    let category = if event_type == "RATES_CREDIT" {
        "US RATES / CREDIT"
    } else {
        "BRENT / OIL"
    };
    let fact = results
        .iter()
        .map(|result| {
            format!(
                "{} latest {:.2} (daily {:+.2})",
                result.label,
                result.observation.latest_value,
                result.observation.latest_value - result.observation.previous_value
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    let published_at = format!("{}T00:00:00Z", results[0].observation.latest_date);
    let evidence = results
        .iter()
        .map(|result| fred_evidence(market_date, event_type, level, result))
        .collect::<Vec<_>>();
    MacroSignalContextProviderEvent {
        event_id: stable_id(
            "fred-event",
            &[
                event_type,
                &market_date.to_string(),
                &published_at,
                category,
            ],
        ),
        accepted_at: accepted_at.to_string(),
        title: category.to_string(),
        information_content: level,
        market_relevance: level,
        evidence_quality: MacroSignalContextProviderInformationLevel::High,
        lifecycle: MacroSignalContextProviderLifecycle::ActiveRepricing,
        event_fact: fact,
        observed_at: published_at.clone(),
        source_published_at: published_at,
        market_date: market_date.to_string(),
        evidence,
        expected_value: None,
        actual_value: None,
        surprise: None,
        reason: Some(
            "Observed repricing is recorded for context; causal attribution is not established."
                .to_string(),
        ),
    }
}

fn fred_evidence(
    market_date: NaiveDate,
    event_type: &str,
    level: MacroSignalContextProviderInformationLevel,
    result: &FredSeriesResult,
) -> ProviderEvidenceRecord {
    let published_at = format!("{}T00:00:00Z", result.observation.latest_date);
    ProviderEvidenceRecord {
        source: "FRED".to_string(),
        source_url: format!("https://fred.stlouisfed.org/series/{}", result.series_id),
        timestamp: format!("{}T00:00:00Z", market_date),
        source_published_at: published_at,
        event_type: event_type.to_string(),
        subject: result.label.to_string(),
        importance: information_level_name(level).to_string(),
    }
}

fn build_fred_reactions(
    market_date: NaiveDate,
    event_type: &str,
    results: &[FredSeriesResult],
) -> Vec<ProviderMarketReaction> {
    results
        .iter()
        .map(|result| {
            let level = if event_type == "RATES_CREDIT" {
                rate_information_level(std::slice::from_ref(result))
            } else {
                commodity_information_level(std::slice::from_ref(result))
            };
            let evidence = fred_evidence(market_date, event_type, level, result);
            ProviderMarketReaction {
                observation_id: stable_id(
                    "fred-observation",
                    &[
                        result.series_id,
                        &result.observation.latest_date.to_string(),
                    ],
                ),
                observed_at: evidence.timestamp.clone(),
                session: "DAILY".to_string(),
                venue: "FRED".to_string(),
                instrument: result.series_id.to_string(),
                source_published_at: evidence.source_published_at.clone(),
                market_date: market_date.to_string(),
                subject: result.label.to_string(),
                observation: format!(
                    "latest {:.2}; daily change {:+.2}",
                    result.observation.latest_value,
                    result.observation.latest_value - result.observation.previous_value
                ),
                evidence: vec![evidence],
            }
        })
        .collect()
}

fn rate_information_level(
    results: &[FredSeriesResult],
) -> MacroSignalContextProviderInformationLevel {
    let high = results.iter().any(|result| {
        let delta = (result.observation.latest_value - result.observation.previous_value).abs();
        if result.series_id == "BAMLH0A0HYM2" {
            delta >= 0.10
        } else {
            delta >= 0.08
        }
    });
    if high {
        MacroSignalContextProviderInformationLevel::High
    } else {
        MacroSignalContextProviderInformationLevel::Low
    }
}

fn commodity_information_level(
    results: &[FredSeriesResult],
) -> MacroSignalContextProviderInformationLevel {
    let high = results.iter().any(|result| {
        let delta = result.observation.latest_value - result.observation.previous_value;
        delta.abs() >= 2.0
            || (result.observation.previous_value != 0.0
                && (delta / result.observation.previous_value).abs() >= 0.02)
    });
    if high {
        MacroSignalContextProviderInformationLevel::High
    } else {
        MacroSignalContextProviderInformationLevel::Low
    }
}

async fn load_finnhub_geopolitical_source(
    client: &reqwest::Client,
    api_key: Option<&str>,
    market_date: NaiveDate,
    accepted_at: &str,
) -> (
    MacroSignalContextProviderSource,
    Vec<ProviderMarketReaction>,
) {
    let Some(api_key) = api_key.filter(|key| !key.trim().is_empty()) else {
        return (
            unavailable_source("Finnhub API key is not configured".to_string()),
            Vec::new(),
        );
    };
    let response = match client
        .get(FINNHUB_NEWS_URL)
        .query(&[("category", "general"), ("token", api_key)])
        .send()
        .await
    {
        Ok(response) => response,
        Err(_err) => {
            return (
                degraded_source("Finnhub request failed".to_string()),
                Vec::new(),
            )
        }
    };
    if !response.status().is_success() {
        return (
            degraded_source(format!("Finnhub returned {}", response.status())),
            Vec::new(),
        );
    }
    let raw = match response.text().await {
        Ok(raw) => raw,
        Err(err) => {
            return (
                degraded_source(format!("Finnhub response body unavailable: {err}")),
                Vec::new(),
            )
        }
    };
    match parse_finnhub_geopolitical_items(&raw, market_date, accepted_at) {
        Ok((events, malformed_count)) => {
            let diagnostics = (malformed_count > 0).then(|| {
                format!("Finnhub skipped {malformed_count} malformed escalation candidate(s)")
            });
            (
                MacroSignalContextProviderSource {
                    status: if malformed_count == 0 {
                        MacroSignalContextProviderSourceStatus::Healthy
                    } else {
                        MacroSignalContextProviderSourceStatus::Partial
                    },
                    events,
                    diagnostics: diagnostics.into_iter().collect(),
                },
                Vec::new(),
            )
        }
        Err(err) => (
            degraded_source(format!("Finnhub JSON invalid: {err}")),
            Vec::new(),
        ),
    }
}

fn parse_finnhub_geopolitical_items(
    raw: &str,
    market_date: NaiveDate,
    accepted_at: &str,
) -> Result<(Vec<MacroSignalContextProviderEvent>, usize)> {
    let items: Vec<Value> = serde_json::from_str(raw).context("invalid Finnhub JSON")?;
    let mut malformed_count = 0;
    let events = items
        .into_iter()
        .filter_map(|item| {
            let headline = item.get("headline").and_then(Value::as_str)?.trim();
            let summary = item.get("summary").and_then(Value::as_str).unwrap_or("");
            let text = format!("{headline} {summary}").to_lowercase();
            if !GEOPOLITICAL_KEYWORDS
                .iter()
                .any(|keyword| text.contains(keyword))
            {
                return None;
            }
            let datetime = item
                .get("datetime")
                .and_then(Value::as_i64)
                .and_then(|timestamp| DateTime::from_timestamp(timestamp, 0));
            let source = item
                .get("source")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty());
            let source_url = item
                .get("url")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty());
            let Some(datetime) = datetime else {
                malformed_count += 1;
                return None;
            };
            if datetime.date_naive() > market_date {
                return None;
            }
            let (Some(source), Some(source_url)) = (source, source_url) else {
                malformed_count += 1;
                return None;
            };
            let level = if [
                "attack", "strike", "missile", "war", "houthi", "escalat",
            ]
            .iter()
            .any(|keyword| text.contains(keyword))
            {
                MacroSignalContextProviderInformationLevel::High
            } else {
                MacroSignalContextProviderInformationLevel::Medium
            };
            let published_at = datetime.to_rfc3339();
            let evidence = ProviderEvidenceRecord {
                source: source.to_string(),
                source_url: source_url.to_string(),
                timestamp: published_at.clone(),
                source_published_at: published_at.clone(),
                event_type: "GEOPOLITICAL_ESCALATION".to_string(),
                subject: headline.to_string(),
                importance: information_level_name(level).to_string(),
            };
            Some(MacroSignalContextProviderEvent {
                event_id: stable_id("finnhub-event", &[source_url, &published_at, headline]),
                accepted_at: accepted_at.to_string(),
                title: "GEOPOLITICAL ESCALATION".to_string(),
                information_content: level,
                market_relevance: level,
                evidence_quality: MacroSignalContextProviderInformationLevel::High,
                lifecycle: if datetime.date_naive() == market_date {
                    MacroSignalContextProviderLifecycle::ActiveRepricing
                } else {
                    MacroSignalContextProviderLifecycle::Aftermath
                },
                event_fact: if summary.trim().is_empty() {
                    headline.to_string()
                } else {
                    format!("{headline}; {summary}")
                },
                observed_at: published_at.clone(),
                source_published_at: published_at,
                market_date: market_date.to_string(),
                evidence: vec![evidence],
                expected_value: None,
                actual_value: None,
                surprise: None,
                reason: Some(
                    "Structured escalation headline is recorded for context; causal attribution is not established."
                        .to_string(),
                ),
            })
        })
        .collect();
    Ok((events, malformed_count))
}

fn information_level_name(level: MacroSignalContextProviderInformationLevel) -> &'static str {
    match level {
        MacroSignalContextProviderInformationLevel::High => "HIGH",
        MacroSignalContextProviderInformationLevel::Medium => "MEDIUM",
        MacroSignalContextProviderInformationLevel::Low => "LOW",
        MacroSignalContextProviderInformationLevel::Unavailable => "UNAVAILABLE",
    }
}

fn unavailable_source(diagnostic: String) -> MacroSignalContextProviderSource {
    MacroSignalContextProviderSource {
        status: MacroSignalContextProviderSourceStatus::Unavailable,
        events: Vec::new(),
        diagnostics: vec![diagnostic],
    }
}

fn degraded_source(diagnostic: String) -> MacroSignalContextProviderSource {
    MacroSignalContextProviderSource {
        status: MacroSignalContextProviderSourceStatus::Degraded,
        events: Vec::new(),
        diagnostics: vec![diagnostic],
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_fred_event, build_fred_reactions, commodity_information_level,
        information_level_name, load_finnhub_geopolitical_source, load_fred_source,
        parse_finnhub_geopolitical_items, parse_fred_series_observations, rate_information_level,
        FredSeriesResult, MacroSignalContextProviderInformationLevel,
        MacroSignalContextProviderLifecycle, ParsedFredSeries, FRED_SERIES_RATES,
    };
    use chrono::NaiveDate;

    #[test]
    fn fred_observations_are_bounded_to_market_date_and_keep_previous_value() {
        let observations = r#"{
            "observations": [
                {"date": "2026-09-08", "value": "4.80"},
                {"date": "2026-09-07", "value": "4.70"},
                {"date": "2026-09-09", "value": "4.95"},
                {"date": "2026-09-06", "value": "."}
            ]
        }"#;

        let parsed = parse_fred_series_observations(
            observations,
            NaiveDate::from_ymd_opt(2026, 9, 8).expect("valid date"),
        )
        .expect("valid FRED response");

        assert_eq!(parsed.latest_date.to_string(), "2026-09-08");
        assert_eq!(parsed.latest_value, 4.80);
        assert_eq!(parsed.previous_date.to_string(), "2026-09-07");
        assert_eq!(parsed.previous_value, 4.70);
    }

    #[test]
    fn finnhub_escalation_parser_filters_future_news_and_requires_evidence() {
        let news = r#"[
            {"datetime": 1788825600, "headline": "Missile strike raises energy risk", "summary": "Oil facilities affected", "source": "Structured News", "url": "https://example.test/1"},
            {"datetime": 1788912000, "headline": "Future attack report", "summary": "not yet in market date", "source": "Structured News", "url": "https://example.test/2"},
            {"datetime": 1788825600, "headline": "War headline without url", "summary": "", "source": "Structured News"}
        ]"#;
        let (events, malformed) = parse_finnhub_geopolitical_items(
            news,
            NaiveDate::from_ymd_opt(2026, 9, 8).expect("valid date"),
            "2026-09-08T23:00:00Z",
        )
        .expect("valid Finnhub response");

        assert_eq!(events.len(), 1);
        assert_eq!(malformed, 1);
        assert_eq!(events[0].market_date, "2026-09-08");
        assert_eq!(
            events[0].information_content,
            super::MacroSignalContextProviderInformationLevel::High
        );
        assert!(!events[0].evidence[0].source_url.is_empty());
    }

    #[tokio::test]
    async fn missing_credentials_fail_closed_without_network() {
        let client = reqwest::Client::new();
        let market_date = NaiveDate::from_ymd_opt(2026, 9, 8).expect("valid date");
        let (rates, rates_reactions) = load_fred_source(
            &client,
            None,
            market_date,
            FRED_SERIES_RATES,
            "RATES_CREDIT",
            super::rate_information_level,
            "2026-09-08T23:00:00Z",
        )
        .await;
        let (geopolitical, geopolitical_reactions) =
            load_finnhub_geopolitical_source(&client, None, market_date, "2026-09-08T23:00:00Z")
                .await;

        assert_eq!(
            rates.status,
            super::MacroSignalContextProviderSourceStatus::Unavailable
        );
        assert_eq!(
            geopolitical.status,
            super::MacroSignalContextProviderSourceStatus::Unavailable
        );
        assert!(rates_reactions.is_empty());
        assert!(geopolitical_reactions.is_empty());
    }

    #[test]
    fn pure_provider_helpers_classify_and_build_fred_context() {
        let rates_result = FredSeriesResult {
            series_id: "DGS10",
            label: "US 10Y Treasury yield",
            observation: ParsedFredSeries {
                latest_date: NaiveDate::from_ymd_opt(2026, 9, 8).expect("valid date"),
                latest_value: 4.80,
                previous_date: NaiveDate::from_ymd_opt(2026, 9, 7).expect("valid date"),
                previous_value: 4.70,
            },
        };
        let credit_result = FredSeriesResult {
            series_id: "BAMLH0A0HYM2",
            label: "US high-yield option-adjusted spread",
            observation: ParsedFredSeries {
                latest_date: rates_result.observation.latest_date,
                latest_value: 3.20,
                previous_date: rates_result.observation.previous_date,
                previous_value: 3.09,
            },
        };
        let market_date = rates_result.observation.latest_date;

        assert_eq!(
            rate_information_level(std::slice::from_ref(&rates_result)),
            MacroSignalContextProviderInformationLevel::High
        );
        assert_eq!(
            rate_information_level(std::slice::from_ref(&credit_result)),
            MacroSignalContextProviderInformationLevel::High
        );
        assert_eq!(
            rate_information_level(&[FredSeriesResult {
                observation: ParsedFredSeries {
                    previous_value: 4.70,
                    latest_value: 4.71,
                    ..rates_result.observation.clone()
                },
                ..rates_result.clone()
            }]),
            MacroSignalContextProviderInformationLevel::Low
        );
        assert_eq!(
            commodity_information_level(std::slice::from_ref(&rates_result)),
            MacroSignalContextProviderInformationLevel::High
        );
        assert_eq!(
            commodity_information_level(&[FredSeriesResult {
                observation: ParsedFredSeries {
                    previous_value: 0.0,
                    latest_value: 0.1,
                    ..rates_result.observation.clone()
                },
                ..rates_result.clone()
            }]),
            MacroSignalContextProviderInformationLevel::Low
        );
        assert_eq!(
            information_level_name(MacroSignalContextProviderInformationLevel::High),
            "HIGH"
        );
        assert_eq!(
            information_level_name(MacroSignalContextProviderInformationLevel::Medium),
            "MEDIUM"
        );
        assert_eq!(
            information_level_name(MacroSignalContextProviderInformationLevel::Low),
            "LOW"
        );
        assert_eq!(
            information_level_name(MacroSignalContextProviderInformationLevel::Unavailable),
            "UNAVAILABLE"
        );

        let event = build_fred_event(
            market_date,
            "RATES_CREDIT",
            MacroSignalContextProviderInformationLevel::High,
            &[rates_result.clone(), credit_result.clone()],
            "2026-09-08T23:00:00Z",
        );
        assert_eq!(
            event.lifecycle,
            MacroSignalContextProviderLifecycle::ActiveRepricing
        );
        assert_eq!(event.evidence.len(), 2);
        assert!(event.event_fact.contains("US 10Y Treasury yield"));
        let reactions = build_fred_reactions(market_date, "RATES_CREDIT", &[rates_result]);
        assert_eq!(reactions.len(), 1);
        assert_eq!(reactions[0].evidence.len(), 1);
    }

    #[test]
    fn parsers_fail_closed_and_keep_medium_aftermath_events() {
        assert!(parse_fred_series_observations(
            "not json",
            NaiveDate::from_ymd_opt(2026, 9, 8).expect("valid date")
        )
        .is_err());
        assert!(parse_fred_series_observations(
            r#"{"observations":[{"date":"2026-09-08","value":"."}]}"#,
            NaiveDate::from_ymd_opt(2026, 9, 8).expect("valid date")
        )
        .is_err());

        let news = r#"[
            {"datetime": 1788739200, "headline": "Iran talks affect energy market", "summary": "", "source": "Structured News", "url": "https://example.test/old"},
            {"headline": "Conflict without timestamp", "summary": "energy facility"}
        ]"#;
        let (events, malformed) = parse_finnhub_geopolitical_items(
            news,
            NaiveDate::from_ymd_opt(2026, 9, 8).expect("valid date"),
            "2026-09-08T23:00:00Z",
        )
        .expect("valid Finnhub response");

        assert_eq!(events.len(), 1);
        assert_eq!(malformed, 1);
        assert_eq!(
            events[0].information_content,
            MacroSignalContextProviderInformationLevel::Medium
        );
        assert_eq!(
            events[0].lifecycle,
            MacroSignalContextProviderLifecycle::Aftermath
        );
        assert_eq!(events[0].event_fact, "Iran talks affect energy market");
    }
}
