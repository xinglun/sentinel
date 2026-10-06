use crate::config;
use crate::features::research::application::capital_absorption::{
    build_capital_absorption_snapshot_from_coverage, classify_capital_absorption_news_observation,
    CapitalAbsorptionAutoEvent, CapitalAbsorptionAutoSnapshot,
    CapitalAbsorptionObservationCoverageState, CapitalAbsorptionSourceCoverage,
    CapitalAbsorptionSourceCoverageStatus, CapitalAbsorptionSourceHealth,
    CapitalAbsorptionSourceStatus,
};
use crate::features::research::infrastructure::capital_absorption_ipo_queue_store::persist_and_replay_ipo_queue_history;
use anyhow::{Context, Result};
use chrono::{Duration, NaiveDate};
use std::path::Path;

const MAX_NEWS_PER_SYMBOL: usize = 20;
const FINNHUB_COMPANY_NEWS_URL: &str = "https://finnhub.io/api/v1/company-news";
const FINNHUB_MARKET_NEWS_URL: &str = "https://finnhub.io/api/v1/news";
const DEFAULT_MARKET_SYMBOLS: &[&str] = &[
    "AAPL", "MSFT", "GOOG", "GOOGL", "AMZN", "META", "NVDA", "TSLA", "AVGO", "ORCL", "AMD", "PLTR",
    "IBM", "INTC",
];
const FINNHUB_PROVIDER: &str = "Finnhub company-news + market-news";

pub(crate) async fn build_automatic_capital_absorption_snapshot(
    app_config: &config::AppConfig,
    as_of_date: NaiveDate,
    lookback_days: usize,
) -> CapitalAbsorptionAutoSnapshot {
    let collection =
        fetch_finnhub_capital_absorption_events(app_config, as_of_date, lookback_days).await;
    let mut snapshot = collection.into_snapshot(as_of_date);
    if should_persist_ipo_queue_history(&snapshot) {
        if let Err(err) = persist_and_replay_ipo_queue_history(
            Path::new(&app_config.output.save_to),
            as_of_date,
            &mut snapshot,
        ) {
            snapshot.source_status.message =
                format!("IPO queue history persistence warning: {err}");
        }
    }
    snapshot
}

fn should_persist_ipo_queue_history(snapshot: &CapitalAbsorptionAutoSnapshot) -> bool {
    snapshot.observation_coverage == CapitalAbsorptionObservationCoverageState::Complete
}

fn filter_events_as_of_date(
    events: Vec<CapitalAbsorptionAutoEvent>,
    as_of_date: NaiveDate,
) -> Vec<CapitalAbsorptionAutoEvent> {
    events
        .into_iter()
        .filter(|event| event.observed_at <= as_of_date)
        .collect()
}

async fn fetch_finnhub_capital_absorption_events(
    app_config: &config::AppConfig,
    as_of_date: NaiveDate,
    lookback_days: usize,
) -> SourceCollection {
    let symbols = capital_absorption_symbols(app_config);
    let mut sources = symbols
        .iter()
        .map(|symbol| format!("company-news:{symbol}"))
        .collect::<Vec<_>>();
    sources.push("market-news:general".to_string());
    let mut collection = SourceCollection::default();

    let token = app_config
        .finnhub
        .as_ref()
        .map(|config| config.finnhub_api_key.as_str())
        .filter(|key| !key.trim().is_empty())
        .map(str::to_owned);
    let Some(token) = token else {
        for source in sources {
            collection.record_not_attempted(source, "Finnhub API key is not configured");
        }
        return collection;
    };

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
    {
        Ok(client) => client,
        Err(_) => {
            for source in sources {
                collection.record_not_attempted(source, "HTTP client could not be created");
            }
            return collection;
        }
    };
    let from = as_of_date - Duration::days(lookback_days as i64);
    for symbol in symbols {
        let source = format!("company-news:{symbol}");
        if collection.stopped_after_rate_limit {
            collection.record_not_attempted(source, "not attempted after HTTP 429 rate limit");
            continue;
        }
        let request = company_news_request(&client, &token, &symbol, from, as_of_date);
        match fetch_source_events(request, &symbol, as_of_date).await {
            Ok(events) => collection.record_success(source, events),
            Err(error) => collection.record_failure(source, error.message, error.rate_limited),
        }
    }

    if collection.stopped_after_rate_limit {
        collection.record_not_attempted(
            "market-news:general",
            "not attempted after HTTP 429 rate limit",
        );
    } else {
        let request = market_news_request(&client, &token);
        match fetch_source_events(request, "Market", as_of_date).await {
            Ok(events) => collection.record_success("market-news:general", events),
            Err(error) => {
                collection.record_failure("market-news:general", error.message, error.rate_limited)
            }
        }
    }

    collection
}

fn company_news_request(
    client: &reqwest::Client,
    token: &str,
    symbol: &str,
    from: NaiveDate,
    to: NaiveDate,
) -> reqwest::RequestBuilder {
    let from = from.to_string();
    let to = to.to_string();
    client.get(FINNHUB_COMPANY_NEWS_URL).query(&[
        ("symbol", symbol),
        ("from", from.as_str()),
        ("to", to.as_str()),
        ("token", token),
    ])
}

fn market_news_request(client: &reqwest::Client, token: &str) -> reqwest::RequestBuilder {
    client
        .get(FINNHUB_MARKET_NEWS_URL)
        .query(&[("category", "general"), ("token", token)])
}

async fn fetch_source_events(
    request: reqwest::RequestBuilder,
    symbol: &str,
    as_of_date: NaiveDate,
) -> std::result::Result<Vec<CapitalAbsorptionAutoEvent>, SourceFetchFailure> {
    let response = request.send().await.map_err(|_| SourceFetchFailure {
        message: "network request failed".to_string(),
        rate_limited: false,
    })?;
    if let Some(failure) = source_failure_for_status(response.status()) {
        return Err(failure);
    }
    let raw = response.text().await.map_err(|_| SourceFetchFailure {
        message: "response body could not be read".to_string(),
        rate_limited: false,
    })?;
    extract_capital_absorption_events(symbol, &raw, as_of_date).map_err(|_| SourceFetchFailure {
        message: "news response was invalid JSON".to_string(),
        rate_limited: false,
    })
}

#[derive(Default)]
struct SourceCollection {
    events: Vec<CapitalAbsorptionAutoEvent>,
    source_coverage: Vec<CapitalAbsorptionSourceCoverage>,
    stopped_after_rate_limit: bool,
}

impl SourceCollection {
    fn record_success(
        &mut self,
        source: impl Into<String>,
        events: Vec<CapitalAbsorptionAutoEvent>,
    ) {
        self.events.extend(events);
        self.source_coverage.push(CapitalAbsorptionSourceCoverage {
            source: source.into(),
            status: CapitalAbsorptionSourceCoverageStatus::Succeeded,
            message: "response processed".to_string(),
        });
    }

    fn record_failure(
        &mut self,
        source: impl Into<String>,
        message: impl Into<String>,
        rate_limited: bool,
    ) {
        self.source_coverage.push(CapitalAbsorptionSourceCoverage {
            source: source.into(),
            status: CapitalAbsorptionSourceCoverageStatus::Failed,
            message: message.into(),
        });
        self.stopped_after_rate_limit |= rate_limited;
    }

    fn record_not_attempted(&mut self, source: impl Into<String>, message: impl Into<String>) {
        self.source_coverage.push(CapitalAbsorptionSourceCoverage {
            source: source.into(),
            status: CapitalAbsorptionSourceCoverageStatus::NotAttempted,
            message: message.into(),
        });
    }

    fn into_snapshot(mut self, as_of_date: NaiveDate) -> CapitalAbsorptionAutoSnapshot {
        let mut events = filter_events_as_of_date(std::mem::take(&mut self.events), as_of_date);
        events.sort_by(|a, b| {
            a.observed_at
                .cmp(&b.observed_at)
                .reverse()
                .then_with(|| a.subject.cmp(&b.subject))
                .then_with(|| a.description.cmp(&b.description))
        });
        let succeeded = self
            .source_coverage
            .iter()
            .filter(|source| source.status == CapitalAbsorptionSourceCoverageStatus::Succeeded)
            .count();
        let observation_coverage = if succeeded == self.source_coverage.len() && succeeded > 0 {
            CapitalAbsorptionObservationCoverageState::Complete
        } else if succeeded > 0 {
            CapitalAbsorptionObservationCoverageState::Partial
        } else {
            CapitalAbsorptionObservationCoverageState::Unavailable
        };
        let source_health = match observation_coverage {
            CapitalAbsorptionObservationCoverageState::Complete => {
                CapitalAbsorptionSourceHealth::Succeeded
            }
            CapitalAbsorptionObservationCoverageState::Partial => {
                CapitalAbsorptionSourceHealth::Partial
            }
            CapitalAbsorptionObservationCoverageState::Unavailable => {
                CapitalAbsorptionSourceHealth::Unavailable
            }
        };
        let source_status = CapitalAbsorptionSourceStatus {
            provider: FINNHUB_PROVIDER.to_string(),
            status: source_health,
            message: String::new(),
        };
        build_capital_absorption_snapshot_from_coverage(
            events,
            source_status,
            observation_coverage,
            self.source_coverage,
        )
    }
}

struct SourceFetchFailure {
    message: String,
    rate_limited: bool,
}

fn source_failure_for_status(status: reqwest::StatusCode) -> Option<SourceFetchFailure> {
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        Some(SourceFetchFailure {
            message: "HTTP 429 rate limited".to_string(),
            rate_limited: true,
        })
    } else if !status.is_success() {
        Some(SourceFetchFailure {
            message: format!("HTTP {status}"),
            rate_limited: false,
        })
    } else {
        None
    }
}

fn capital_absorption_symbols(_app_config: &config::AppConfig) -> Vec<String> {
    let mut symbols = DEFAULT_MARKET_SYMBOLS
        .iter()
        .map(|symbol| (*symbol).to_string())
        .collect::<Vec<_>>();
    symbols.sort();
    symbols.dedup();
    symbols
}

fn extract_capital_absorption_events(
    symbol: &str,
    raw_json: &str,
    _fallback_date: NaiveDate,
) -> Result<Vec<CapitalAbsorptionAutoEvent>> {
    let items: Vec<serde_json::Value> =
        serde_json::from_str(raw_json).context("Failed to parse Finnhub news JSON")?;
    Ok(items
        .iter()
        .take(MAX_NEWS_PER_SYMBOL)
        .filter_map(|item| event_from_news_item(symbol, item, _fallback_date))
        .collect())
}

fn event_from_news_item(
    symbol: &str,
    item: &serde_json::Value,
    _fallback_date: NaiveDate,
) -> Option<CapitalAbsorptionAutoEvent> {
    let headline = item
        .get("headline")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let summary = item
        .get("summary")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let observed_at = item
        .get("datetime")
        .and_then(|value| value.as_i64())
        .and_then(|timestamp| chrono::DateTime::from_timestamp(timestamp, 0))
        .map(|datetime| datetime.date_naive())?;
    let source_url = item
        .get("url")
        .and_then(|value| value.as_str())
        .filter(|url| !url.trim().is_empty())
        .map(|url| url.to_string());
    classify_capital_absorption_news_observation(symbol, headline, summary, observed_at, source_url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::research::application::capital_absorption::{
        CapitalAbsorptionAutoConfidence, CapitalAbsorptionAutoEventCategory,
        CapitalAbsorptionObservationEventType, CapitalAbsorptionSupplyKind,
    };

    fn sample_event(observed_at: NaiveDate) -> CapitalAbsorptionAutoEvent {
        CapitalAbsorptionAutoEvent {
            category: CapitalAbsorptionAutoEventCategory::IpoSupply,
            supply_kind: CapitalAbsorptionSupplyKind::Potential,
            event_type: CapitalAbsorptionObservationEventType::Reported,
            subject: "SpaceX".to_string(),
            description: "IPO discussion".to_string(),
            amount_usd_b: None,
            ai_capex_related: false,
            source_url: None,
            observed_at,
            source_count: 1,
            confidence: CapitalAbsorptionAutoConfidence::Low,
        }
    }

    #[test]
    fn source_collection_preserves_successful_events_after_rate_limit() {
        let observed_at = NaiveDate::from_ymd_opt(2026, 6, 3).unwrap();
        let mut collection = SourceCollection::default();
        collection.record_success("company-news:GOOG", vec![sample_event(observed_at)]);
        collection.record_failure("company-news:MSFT", "HTTP 429", true);
        assert!(collection.stopped_after_rate_limit);
        collection.record_not_attempted("company-news:NVDA", "rate limit reached");
        collection.record_not_attempted("market-news:general", "rate limit reached");

        let snapshot = collection.into_snapshot(observed_at);

        assert_eq!(snapshot.observed_events.len(), 1);
        assert_eq!(
            snapshot.observation_coverage,
            CapitalAbsorptionObservationCoverageState::Partial
        );
        assert_eq!(snapshot.status, crate::features::research::domain::capital_absorption::CapitalAbsorptionAutoStatus::Watch);
        assert_eq!(
            snapshot.source_status.status,
            CapitalAbsorptionSourceHealth::Partial
        );
        assert_eq!(
            snapshot.source_coverage[0].status,
            CapitalAbsorptionSourceCoverageStatus::Succeeded
        );
        assert_eq!(
            snapshot.source_coverage[1].status,
            CapitalAbsorptionSourceCoverageStatus::Failed
        );
        assert!(snapshot.source_coverage[1].message.contains("429"));
        assert_eq!(
            snapshot.source_coverage[2].status,
            CapitalAbsorptionSourceCoverageStatus::NotAttempted
        );
        assert_eq!(
            snapshot.source_coverage[3].status,
            CapitalAbsorptionSourceCoverageStatus::NotAttempted
        );
        assert_eq!(snapshot.capital_demand.rolling_12m_usd_b, None);
    }

    #[test]
    fn only_complete_observations_may_persist_ipo_queue_history() {
        let observed_at = NaiveDate::from_ymd_opt(2026, 6, 3).unwrap();

        let mut complete = SourceCollection::default();
        complete.record_success("company-news:GOOG", Vec::new());
        let complete = complete.into_snapshot(observed_at);
        assert_eq!(
            complete.observation_coverage,
            CapitalAbsorptionObservationCoverageState::Complete
        );
        assert!(should_persist_ipo_queue_history(&complete));

        let mut partial = SourceCollection::default();
        partial.record_success("company-news:GOOG", vec![sample_event(observed_at)]);
        partial.record_failure("company-news:MSFT", "network request failed", false);
        let partial = partial.into_snapshot(observed_at);
        assert_eq!(
            partial.observation_coverage,
            CapitalAbsorptionObservationCoverageState::Partial
        );
        assert_eq!(partial.observed_events.len(), 1);
        assert!(!should_persist_ipo_queue_history(&partial));

        let mut unavailable = SourceCollection::default();
        unavailable.record_failure("company-news:GOOG", "HTTP 429", true);
        let unavailable = unavailable.into_snapshot(observed_at);
        assert_eq!(
            unavailable.observation_coverage,
            CapitalAbsorptionObservationCoverageState::Unavailable
        );
        assert!(!should_persist_ipo_queue_history(&unavailable));
    }

    #[test]
    fn http_429_is_rate_limit_health_failure_not_an_empty_success() {
        let failure = source_failure_for_status(reqwest::StatusCode::TOO_MANY_REQUESTS)
            .expect("429 must be rejected as a failed source response");

        assert!(failure.rate_limited);
        assert!(failure.message.contains("429"));
        assert!(source_failure_for_status(reqwest::StatusCode::OK).is_none());
        assert!(
            !source_failure_for_status(reqwest::StatusCode::SERVICE_UNAVAILABLE)
                .expect("5xx should be a source failure")
                .rate_limited
        );
    }

    #[test]
    fn empty_successful_feed_is_complete_but_unavailable_feed_is_not_empty_evidence() {
        let observed_at = NaiveDate::from_ymd_opt(2026, 6, 3).unwrap();
        let mut complete = SourceCollection::default();
        complete.record_success("company-news:GOOG", Vec::new());
        let complete = complete.into_snapshot(observed_at);

        let mut unavailable = SourceCollection::default();
        unavailable.record_failure("company-news:GOOG", "HTTP 429 rate limited", true);
        unavailable.record_not_attempted("market-news:general", "rate limit reached");
        let unavailable = unavailable.into_snapshot(observed_at);

        assert_eq!(
            complete.observation_coverage,
            CapitalAbsorptionObservationCoverageState::Complete
        );
        assert!(complete.observed_events.is_empty());
        assert_eq!(complete.status, crate::features::research::domain::capital_absorption::CapitalAbsorptionAutoStatus::Normal);
        assert_eq!(
            unavailable.observation_coverage,
            CapitalAbsorptionObservationCoverageState::Unavailable
        );
        assert_eq!(
            unavailable.source_status.status,
            CapitalAbsorptionSourceHealth::Unavailable
        );
        assert!(unavailable.observed_events.is_empty());
        assert_eq!(unavailable.status, crate::features::research::domain::capital_absorption::CapitalAbsorptionAutoStatus::Normal);
        assert_eq!(unavailable.capital_demand.rolling_12m_usd_b, None);
    }

    #[test]
    fn source_collection_state_is_fresh_for_each_snapshot() {
        let observed_at = NaiveDate::from_ymd_opt(2026, 6, 3).unwrap();
        let mut first_run = SourceCollection::default();
        first_run.record_success("company-news:GOOG", vec![sample_event(observed_at)]);
        let first = first_run.into_snapshot(observed_at);

        let mut recovered_run = SourceCollection::default();
        recovered_run.record_failure("company-news:GOOG", "network request failed", false);
        let recovered = recovered_run.into_snapshot(observed_at);

        assert_eq!(first.observed_events.len(), 1);
        assert!(recovered.observed_events.is_empty());
        assert_ne!(
            first.collection_snapshot_id,
            recovered.collection_snapshot_id
        );
        assert_eq!(
            recovered.observation_coverage,
            CapitalAbsorptionObservationCoverageState::Unavailable
        );
    }

    #[test]
    fn historical_snapshot_excludes_future_news_but_keeps_same_day_news() {
        let as_of_date = NaiveDate::from_ymd_opt(2026, 6, 10).unwrap();
        let events = filter_events_as_of_date(
            vec![
                sample_event(NaiveDate::from_ymd_opt(2026, 6, 10).unwrap()),
                sample_event(NaiveDate::from_ymd_opt(2026, 6, 11).unwrap()),
            ],
            as_of_date,
        );

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].observed_at, as_of_date);
    }

    #[test]
    fn extracts_offering_event_from_finnhub_news() {
        let raw = r#"[
          {
            "headline": "Alphabet announces $80 billion secondary offering for AI data center capex",
            "summary": "The company plans to raise capital for AI infrastructure.",
            "url": "https://example.com/alphabet-offering",
            "datetime": 1780444800
          }
        ]"#;

        let events = extract_capital_absorption_events(
            "GOOG",
            raw,
            NaiveDate::from_ymd_opt(2026, 6, 3).unwrap(),
        )
        .unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].category,
            CapitalAbsorptionAutoEventCategory::MegaCapFinancing
        );
        assert_eq!(events[0].amount_usd_b, Some(80.0));
        assert!(events[0].ai_capex_related);
        assert_eq!(events[0].supply_kind, CapitalAbsorptionSupplyKind::Actual);
        assert_eq!(
            events[0].event_type,
            CapitalAbsorptionObservationEventType::Confirmed
        );
    }

    #[test]
    fn ignores_ordinary_news_without_capital_absorption_terms() {
        let raw = r#"[{"headline":"Alphabet launches a new product","summary":"Product availability expanded globally.","datetime":1780444800}]"#;

        let events = extract_capital_absorption_events(
            "GOOG",
            raw,
            NaiveDate::from_ymd_opt(2026, 6, 3).unwrap(),
        )
        .unwrap();

        assert!(events.is_empty());
    }

    #[test]
    fn excludes_news_without_datetime_instead_of_fabricating_as_of_date() {
        let raw = r#"[
          {
            "headline": "SpaceX IPO discussion grows",
            "summary": "Investors discuss a possible listing.",
            "url": "https://example.com/undated-ipo"
          }
        ]"#;

        let events = extract_capital_absorption_events(
            "Market",
            raw,
            NaiveDate::from_ymd_opt(2026, 6, 3).unwrap(),
        )
        .unwrap();

        assert!(events.is_empty());
    }

    #[test]
    fn detects_ai_ipo_candidate_subject_from_market_news() {
        let raw = r#"[
          {
            "headline": "SpaceX IPO expected as investor discussion increases",
            "summary": "Several reports say the company is preparing for a possible listing.",
            "url": "https://example.com/spacex-ipo",
            "datetime": 1780444800
          }
        ]"#;

        let events = extract_capital_absorption_events(
            "Market",
            raw,
            NaiveDate::from_ymd_opt(2026, 6, 3).unwrap(),
        )
        .unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].subject, "SpaceX");
        assert_eq!(
            events[0].category,
            CapitalAbsorptionAutoEventCategory::IpoSupply
        );
        assert_eq!(
            events[0].supply_kind,
            CapitalAbsorptionSupplyKind::Potential
        );
        assert_eq!(events[0].amount_usd_b, None);
    }

    #[test]
    fn keeps_anthropic_ipo_discussion_in_potential_queue_without_amount() {
        let raw = r#"[
          {
            "headline": "Anthropic IPO discussion grows after private valuation reaches $60 billion",
            "summary": "Investors are considering the company ahead of a possible IPO.",
            "url": "https://example.com/anthropic-ipo-discussion",
            "datetime": 1780444800
          }
        ]"#;

        let events = extract_capital_absorption_events(
            "Market",
            raw,
            NaiveDate::from_ymd_opt(2026, 6, 3).unwrap(),
        )
        .unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].subject, "Anthropic");
        assert_eq!(
            events[0].supply_kind,
            CapitalAbsorptionSupplyKind::Potential
        );
        assert_eq!(
            events[0].event_type,
            CapitalAbsorptionObservationEventType::Reported
        );
        assert_eq!(events[0].amount_usd_b, None);
    }

    #[test]
    fn ignores_weak_ipo_related_stock_recommendations() {
        let raw = r#"[
          {
            "headline": "3 stocks to buy before the Anthropic IPO",
            "summary": "A Wall Street analyst research call mentions related tickers.",
            "url": "https://example.com/stocks-before-ipo",
            "datetime": 1780444800
          }
        ]"#;

        let events = extract_capital_absorption_events(
            "Market",
            raw,
            NaiveDate::from_ymd_opt(2026, 6, 3).unwrap(),
        )
        .unwrap();

        assert!(events.is_empty());
    }

    #[test]
    fn confirmed_ipo_uses_only_confirmed_financing_amount() {
        let raw = r#"[
          {
            "headline": "Figure filed for IPO to raise $750 million",
            "summary": "The S-1 confirms expected gross proceeds from the offering.",
            "url": "https://example.com/figure-ipo-filed",
            "datetime": 1780444800
          }
        ]"#;

        let events = extract_capital_absorption_events(
            "Market",
            raw,
            NaiveDate::from_ymd_opt(2026, 6, 3).unwrap(),
        )
        .unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].subject, "Figure");
        assert_eq!(events[0].supply_kind, CapitalAbsorptionSupplyKind::Actual);
        assert_eq!(events[0].amount_usd_b, Some(0.75));
    }

    #[test]
    fn ignores_projected_ipo_valuation_amount_for_actual_supply() {
        let raw = r#"[
          {
            "headline": "Stripe IPO expected at $90 billion valuation",
            "summary": "The company remains an IPO candidate, with no offering amount confirmed.",
            "url": "https://example.com/stripe-ipo-valuation",
            "datetime": 1780444800
          }
        ]"#;

        let events = extract_capital_absorption_events(
            "Market",
            raw,
            NaiveDate::from_ymd_opt(2026, 6, 3).unwrap(),
        )
        .unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].subject, "Stripe");
        assert_eq!(
            events[0].supply_kind,
            CapitalAbsorptionSupplyKind::Potential
        );
        assert_eq!(events[0].amount_usd_b, None);
    }

    #[test]
    fn company_news_request_encodes_symbol_dates_and_token_as_query_parameters() {
        let from = NaiveDate::from_ymd_opt(2026, 5, 27).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 6, 3).unwrap();
        let token = "fake token+/=&?";
        let client = reqwest::Client::new();

        let request = company_news_request(&client, token, "BRK/B & Co", from, to)
            .build()
            .expect("company-news request should build offline");
        let url = request.url();
        let query = url
            .query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect::<Vec<_>>();

        assert_eq!(url.host_str(), Some("finnhub.io"));
        assert_eq!(url.path(), "/api/v1/company-news");
        assert!(
            query
                == vec![
                    ("symbol".to_string(), "BRK/B & Co".to_string()),
                    ("from".to_string(), "2026-05-27".to_string()),
                    ("to".to_string(), "2026-06-03".to_string()),
                    ("token".to_string(), token.to_string()),
                ],
            "company-news request should preserve all parameter values"
        );
    }

    #[test]
    fn market_news_request_keeps_general_category_and_encodes_token() {
        let token = "fake token+/=&?";
        let client = reqwest::Client::new();

        let request = market_news_request(&client, token)
            .build()
            .expect("market-news request should build offline");
        let url = request.url();
        let query = url
            .query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect::<Vec<_>>();

        assert_eq!(url.host_str(), Some("finnhub.io"));
        assert_eq!(url.path(), "/api/v1/news");
        assert!(
            query
                == vec![
                    ("category".to_string(), "general".to_string()),
                    ("token".to_string(), token.to_string()),
                ],
            "market-news request should preserve category and token values"
        );
    }
}
