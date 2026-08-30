#![allow(dead_code)]

use crate::features::research::application::corporate_event_provider::{
    CorporateEventSource, CorporateEventSourceKind, CorporateEventType, ExpectedCorporateEvent,
    ExpectedCorporateEventProvider, ExpectedCorporateEventProviderHealth,
    ExpectedCorporateEventProviderReadModel, FiscalPeriod,
};
use chrono::{DateTime, NaiveDate, Utc};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::NamedTempFile;

const ALPHA_VANTAGE_PROVIDER_ID: &str = "alpha_vantage";
const ALPHA_VANTAGE_URL: &str = "https://www.alphavantage.co/query";
const ALPHA_VANTAGE_HORIZON: &str = "3month";
const CACHE_SCHEMA_VERSION: u32 = 1;
const CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const MARKET_DATE_FORMAT: &str = "%Y-%m-%d";

/// Alpha Vantage HTTP 応答を provider port から分離する transport 応答。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AlphaVantageTransportResponse {
    pub status: u16,
    pub body: String,
}

/// Alpha Vantage transport の差し替え境界。fixture と失敗系テストはネットワークを使わない。
pub(crate) trait AlphaVantageEarningsCalendarTransport: Clone {
    fn fetch(&self, horizon: &str, api_key: &str) -> Result<AlphaVantageTransportResponse, String>;
}

#[derive(Clone)]
struct ReqwestAlphaVantageEarningsCalendarTransport {
    client: Client,
}

impl ReqwestAlphaVantageEarningsCalendarTransport {
    fn new() -> Result<Self, String> {
        Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map(|client| Self { client })
            .map_err(|error| format!("Alpha Vantage HTTP client could not be created: {error}"))
    }
}

impl AlphaVantageEarningsCalendarTransport for ReqwestAlphaVantageEarningsCalendarTransport {
    fn fetch(&self, horizon: &str, api_key: &str) -> Result<AlphaVantageTransportResponse, String> {
        let response = self
            .client
            .get(ALPHA_VANTAGE_URL)
            .query(&[
                ("function", "EARNINGS_CALENDAR"),
                ("horizon", horizon),
                ("datatype", "csv"),
                ("apikey", api_key),
            ])
            .send()
            .map_err(|error| format!("Alpha Vantage request failed: {error}"))?;
        let status = response.status().as_u16();
        let body = response
            .text()
            .map_err(|error| format!("Alpha Vantage response could not be read: {error}"))?;
        Ok(AlphaVantageTransportResponse { status, body })
    }
}

#[derive(Clone)]
pub(crate) struct AlphaVantageExpectedEventProvider<T> {
    api_key: String,
    cache_path: PathBuf,
    now: DateTime<Utc>,
    transport: T,
}

impl<T> AlphaVantageExpectedEventProvider<T>
where
    T: AlphaVantageEarningsCalendarTransport,
{
    pub(crate) fn with_transport(
        api_key: impl Into<String>,
        cache_path: impl Into<PathBuf>,
        now: DateTime<Utc>,
        transport: T,
    ) -> Self {
        Self {
            api_key: api_key.into(),
            cache_path: cache_path.into(),
            now,
            transport,
        }
    }
}

impl<T> ExpectedCorporateEventProvider for AlphaVantageExpectedEventProvider<T>
where
    T: AlphaVantageEarningsCalendarTransport,
{
    fn load_for_universe(&self, symbols: &[String]) -> ExpectedCorporateEventProviderReadModel {
        let source = alpha_vantage_source();
        let universe = normalized_symbols(symbols);
        if universe.is_empty() {
            return unavailable(
                source,
                "Alpha Vantage expected event provider requires a non-empty observation universe",
            );
        }

        let api_key = self.api_key.trim();
        if api_key.is_empty() {
            return unavailable(source, "Alpha Vantage API key is not configured");
        }

        match read_cache(&self.cache_path, self.now) {
            Ok(Some(cache)) if cache_is_fresh(cache.fetched_at, self.now) => {
                return healthy_from_cache(cache, self.now, &universe);
            }
            Ok(Some(_expired_cache)) => {}
            Ok(None) => {}
            Err(error) => return unavailable(source, error),
        }

        let response = match self.transport.fetch(ALPHA_VANTAGE_HORIZON, api_key) {
            Ok(response) => response,
            Err(error) => {
                return unavailable(
                    source,
                    sanitize_diagnostic(api_key, format!("Alpha Vantage request failed: {error}")),
                )
            }
        };
        if !(200..300).contains(&response.status) {
            return unavailable(
                source,
                format!("Alpha Vantage API returned HTTP {}", response.status),
            );
        }

        let events = match parse_alpha_vantage_earnings_calendar(&response.body, symbols, self.now)
        {
            Ok(events) => events,
            Err(error) => return unavailable(source, sanitize_diagnostic(api_key, error)),
        };
        let cache = AlphaVantageCache::from_events(self.now, &events);
        if let Err(error) = write_cache(&self.cache_path, &cache) {
            return unavailable(source, error);
        }

        ExpectedCorporateEventProviderReadModel {
            health: ExpectedCorporateEventProviderHealth::Healthy,
            source,
            fetched_at: Some(self.now),
            diagnostic: None,
            events: events
                .into_iter()
                .filter(|event| universe.contains(&event.symbol))
                .collect(),
        }
    }
}

/// Production の Alpha Vantage expected provider を生成して observation universe を取得する。
pub(crate) fn load_alpha_vantage_expected_events(
    symbols: &[String],
    cache_path: impl AsRef<Path>,
) -> ExpectedCorporateEventProviderReadModel {
    let Some(api_key) = std::env::var("ALPHA_VANTAGE_API_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        return unavailable(
            alpha_vantage_source(),
            "Alpha Vantage API key is not configured",
        );
    };
    let transport = match ReqwestAlphaVantageEarningsCalendarTransport::new() {
        Ok(transport) => transport,
        Err(error) => return unavailable(alpha_vantage_source(), error),
    };
    AlphaVantageExpectedEventProvider::with_transport(
        api_key,
        cache_path.as_ref().to_path_buf(),
        Utc::now(),
        transport,
    )
    .load_for_universe(symbols)
}

pub(crate) fn alpha_vantage_source() -> CorporateEventSource {
    CorporateEventSource {
        provider_id: ALPHA_VANTAGE_PROVIDER_ID.to_string(),
        source_kind: CorporateEventSourceKind::EarningsCalendar,
        source_url: Some(
            "https://www.alphavantage.co/query?function=EARNINGS_CALENDAR&horizon=3month&datatype=csv"
                .to_string(),
        ),
    }
}

fn unavailable(
    source: CorporateEventSource,
    diagnostic: impl Into<String>,
) -> ExpectedCorporateEventProviderReadModel {
    ExpectedCorporateEventProviderReadModel::unavailable(source, diagnostic)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AlphaVantageCache {
    fetched_at: DateTime<Utc>,
    provider: String,
    schema_version: u32,
    records: Vec<AlphaVantageCacheRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AlphaVantageCacheRecord {
    symbol: String,
    expected_date: String,
    fiscal_period: Option<CacheFiscalPeriod>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CacheFiscalPeriod {
    quarter: u8,
    year: i32,
}

impl AlphaVantageCache {
    fn from_events(fetched_at: DateTime<Utc>, events: &[ExpectedCorporateEvent]) -> Self {
        Self {
            fetched_at,
            provider: ALPHA_VANTAGE_PROVIDER_ID.to_string(),
            schema_version: CACHE_SCHEMA_VERSION,
            records: events
                .iter()
                .map(|event| AlphaVantageCacheRecord {
                    symbol: event.symbol.clone(),
                    expected_date: event.expected_date.format(MARKET_DATE_FORMAT).to_string(),
                    fiscal_period: event.fiscal_period.map(|period| CacheFiscalPeriod {
                        quarter: period.quarter,
                        year: period.year,
                    }),
                })
                .collect(),
        }
    }
}

fn read_cache(path: &Path, now: DateTime<Utc>) -> Result<Option<AlphaVantageCache>, String> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Alpha Vantage cache could not be read: {error}")),
    };
    let cache = serde_json::from_str::<AlphaVantageCache>(&raw)
        .map_err(|error| format!("Alpha Vantage cache is malformed: {error}"))?;
    if cache.provider != ALPHA_VANTAGE_PROVIDER_ID {
        return Err("Alpha Vantage cache provider does not match".to_string());
    }
    if cache.schema_version != CACHE_SCHEMA_VERSION {
        return Err("Alpha Vantage cache schema version is unsupported".to_string());
    }
    if cache.fetched_at > now {
        return Err("Alpha Vantage cache fetched_at is in the future".to_string());
    }
    validate_cache_records(&cache.records)?;
    Ok(Some(cache))
}

fn validate_cache_records(records: &[AlphaVantageCacheRecord]) -> Result<(), String> {
    for record in records {
        if !is_valid_symbol(&record.symbol) {
            return Err("Alpha Vantage cache contains an invalid symbol".to_string());
        }
        NaiveDate::parse_from_str(&record.expected_date, MARKET_DATE_FORMAT)
            .map_err(|error| format!("Alpha Vantage cache contains an invalid date: {error}"))?;
        if let Some(period) = &record.fiscal_period {
            validate_fiscal_period(period.quarter, period.year)?;
        }
    }
    Ok(())
}

fn cache_is_fresh(fetched_at: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    now.signed_duration_since(fetched_at)
        .to_std()
        .is_ok_and(|age| age < CACHE_TTL)
}

fn healthy_from_cache(
    cache: AlphaVantageCache,
    _now: DateTime<Utc>,
    universe: &BTreeSet<String>,
) -> ExpectedCorporateEventProviderReadModel {
    let source = alpha_vantage_source();
    let fetched_at = cache.fetched_at;
    let events = cache
        .records
        .into_iter()
        .filter(|record| universe.contains(&record.symbol))
        .map(|record| ExpectedCorporateEvent {
            symbol: record.symbol,
            event_type: CorporateEventType::Earnings,
            expected_date: NaiveDate::parse_from_str(&record.expected_date, MARKET_DATE_FORMAT)
                .expect("cache records are validated before conversion"),
            fiscal_period: record.fiscal_period.map(|period| FiscalPeriod {
                quarter: period.quarter,
                year: period.year,
            }),
            source: source.clone(),
            observed_at: fetched_at,
        })
        .collect();
    ExpectedCorporateEventProviderReadModel {
        health: ExpectedCorporateEventProviderHealth::Healthy,
        source,
        fetched_at: Some(fetched_at),
        diagnostic: None,
        events,
    }
}

fn write_cache(path: &Path, cache: &AlphaVantageCache) -> Result<(), String> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .map_err(|error| format!("Alpha Vantage cache directory could not be created: {error}"))?;
    let mut temporary = NamedTempFile::new_in(parent).map_err(|error| {
        format!("Alpha Vantage cache temporary file could not be created: {error}")
    })?;
    let encoded = serde_json::to_vec_pretty(cache)
        .map_err(|error| format!("Alpha Vantage cache could not be encoded: {error}"))?;
    temporary
        .write_all(&encoded)
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|error| format!("Alpha Vantage cache could not be written: {error}"))?;
    temporary
        .persist(path)
        .map_err(|error| format!("Alpha Vantage cache could not be committed: {error}"))?;
    Ok(())
}

fn parse_alpha_vantage_earnings_calendar(
    raw: &str,
    symbols: &[String],
    observed_at: DateTime<Utc>,
) -> Result<Vec<ExpectedCorporateEvent>, String> {
    let trimmed = raw.trim_start_matches('\u{feff}').trim();
    if trimmed.starts_with('{') {
        let _ = serde_json::from_str::<serde_json::Value>(trimmed)
            .map_err(|error| format!("Alpha Vantage error payload is malformed: {error}"))?;
        return Err("Alpha Vantage error payload received".to_string());
    }
    let rows = parse_csv_rows(trimmed)?;
    let headers = header_indexes(
        rows.first()
            .ok_or_else(|| "Alpha Vantage earnings calendar CSV is empty".to_string())?,
    )?;
    let symbol_index = required_column(&headers, "symbol")?;
    let report_date_index = required_column(&headers, "reportdate")?;
    let fiscal_date_index = headers.get("fiscaldateending").copied();
    let universe = normalized_symbols(symbols);
    if universe.is_empty() {
        return Err(
            "Alpha Vantage expected event provider requires a non-empty observation universe"
                .to_string(),
        );
    }

    let source = alpha_vantage_source();
    let mut events = BTreeMap::new();
    for row in rows.into_iter().skip(1) {
        if row.len() != headers.len() {
            return Err(
                "Alpha Vantage earnings calendar CSV row has an unexpected column count"
                    .to_string(),
            );
        }
        let raw_symbol = row
            .get(symbol_index)
            .map(String::as_str)
            .unwrap_or_default()
            .trim();
        if raw_symbol.is_empty() {
            return Err("Alpha Vantage earnings calendar row has an empty symbol".to_string());
        }
        let symbol = raw_symbol.to_ascii_uppercase();
        if !universe.contains(&symbol) {
            continue;
        }
        if !is_valid_symbol(&symbol) {
            return Err("Alpha Vantage earnings calendar row has an invalid symbol".to_string());
        }
        let report_date = parse_row_date(&row, report_date_index, "report date")?;
        if let Some(index) = fiscal_date_index {
            if !row
                .get(index)
                .map(String::as_str)
                .unwrap_or_default()
                .trim()
                .is_empty()
            {
                let _ = parse_row_date(&row, index, "fiscal date")?;
            }
        }
        let event = ExpectedCorporateEvent {
            symbol: symbol.clone(),
            event_type: CorporateEventType::Earnings,
            expected_date: report_date,
            fiscal_period: None,
            source: source.clone(),
            observed_at,
        };
        events.entry((symbol, report_date)).or_insert(event);
    }
    Ok(events.into_values().collect())
}

fn parse_row_date(row: &[String], index: usize, label: &str) -> Result<NaiveDate, String> {
    let value = row
        .get(index)
        .map(String::as_str)
        .unwrap_or_default()
        .trim();
    NaiveDate::parse_from_str(value, MARKET_DATE_FORMAT)
        .map_err(|error| format!("Alpha Vantage earnings calendar has invalid {label}: {error}"))
}

fn parse_csv_rows(raw: &str) -> Result<Vec<Vec<String>>, String> {
    let mut rows = Vec::new();
    for line in raw.lines() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }
        rows.push(parse_csv_line(line)?);
    }
    Ok(rows)
}

fn parse_csv_line(line: &str) -> Result<Vec<String>, String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '"' if quoted && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => {
                fields.push(field.trim().to_string());
                field.clear();
            }
            _ => field.push(character),
        }
    }
    if quoted {
        return Err("Alpha Vantage earnings calendar CSV has an unclosed quote".to_string());
    }
    fields.push(field.trim().to_string());
    Ok(fields)
}

fn header_indexes(row: &[String]) -> Result<BTreeMap<String, usize>, String> {
    let mut headers = BTreeMap::new();
    for (index, header) in row.iter().enumerate() {
        let normalized = header.trim().to_ascii_lowercase();
        if normalized.is_empty() || headers.insert(normalized, index).is_some() {
            return Err("Alpha Vantage earnings calendar CSV has invalid headers".to_string());
        }
    }
    Ok(headers)
}

fn required_column(headers: &BTreeMap<String, usize>, name: &str) -> Result<usize, String> {
    headers
        .get(name)
        .copied()
        .ok_or_else(|| format!("Alpha Vantage earnings calendar CSV is missing {name} column"))
}

fn normalized_symbols(symbols: &[String]) -> BTreeSet<String> {
    symbols
        .iter()
        .map(|symbol| symbol.trim().to_ascii_uppercase())
        .filter(|symbol| !symbol.is_empty())
        .collect()
}

fn is_valid_symbol(symbol: &str) -> bool {
    !symbol.is_empty()
        && symbol.len() <= 32
        && symbol
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || ".-_".contains(character))
}

fn validate_fiscal_period(quarter: u8, year: i32) -> Result<(), String> {
    if !(1..=4).contains(&quarter) || !(1900..=9999).contains(&year) {
        return Err("Alpha Vantage cache contains an invalid fiscal period".to_string());
    }
    Ok(())
}

fn sanitize_diagnostic(api_key: &str, diagnostic: String) -> String {
    diagnostic.replace(api_key, "[REDACTED]")
}

#[cfg(test)]
mod tests {
    use super::{
        parse_alpha_vantage_earnings_calendar, AlphaVantageEarningsCalendarTransport,
        AlphaVantageExpectedEventProvider, AlphaVantageTransportResponse,
        ReqwestAlphaVantageEarningsCalendarTransport,
    };
    use crate::features::research::application::corporate_event_provider::{
        CorporateEventSourceKind, CorporateEventType, ExpectedCorporateEventProvider,
        ExpectedCorporateEventProviderHealth,
    };
    use chrono::{DateTime, Utc};
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    const FIXTURE: &str =
        include_str!("../../../../tests/fixtures/alpha_vantage/earnings_calendar.csv");

    #[derive(Clone)]
    struct StubTransport {
        response: Result<AlphaVantageTransportResponse, String>,
        calls: Arc<Mutex<Vec<(String, String)>>>,
    }

    impl StubTransport {
        fn success(body: &str, calls: Arc<Mutex<Vec<(String, String)>>>) -> Self {
            Self {
                response: Ok(AlphaVantageTransportResponse {
                    status: 200,
                    body: body.to_string(),
                }),
                calls,
            }
        }

        fn status(status: u16, body: &str, calls: Arc<Mutex<Vec<(String, String)>>>) -> Self {
            Self {
                response: Ok(AlphaVantageTransportResponse {
                    status,
                    body: body.to_string(),
                }),
                calls,
            }
        }

        fn error(message: &str, calls: Arc<Mutex<Vec<(String, String)>>>) -> Self {
            Self {
                response: Err(message.to_string()),
                calls,
            }
        }
    }

    impl AlphaVantageEarningsCalendarTransport for StubTransport {
        fn fetch(
            &self,
            horizon: &str,
            api_key: &str,
        ) -> Result<AlphaVantageTransportResponse, String> {
            self.calls
                .lock()
                .expect("call log lock must not be poisoned")
                .push((horizon.to_string(), api_key.to_string()));
            self.response.clone()
        }
    }

    fn timestamp() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-08-31T00:00:00Z")
            .expect("fixture timestamp must be valid")
            .with_timezone(&Utc)
    }

    fn symbols() -> Vec<String> {
        vec!["NVDA".to_string(), "MSFT".to_string()]
    }

    fn provider(
        api_key: &str,
        cache_path: &Path,
        body: &str,
        calls: Arc<Mutex<Vec<(String, String)>>>,
    ) -> AlphaVantageExpectedEventProvider<StubTransport> {
        AlphaVantageExpectedEventProvider::with_transport(
            api_key,
            cache_path.to_path_buf(),
            timestamp(),
            StubTransport::success(body, calls),
        )
    }

    #[test]
    fn reqwest_transport_can_be_constructed_with_finite_timeout() {
        ReqwestAlphaVantageEarningsCalendarTransport::new()
            .expect("production transport must be constructible");
    }

    #[test]
    fn parses_calendar_and_filters_to_local_observation_universe() {
        let events = parse_alpha_vantage_earnings_calendar(FIXTURE, &symbols(), timestamp())
            .expect("fixture must parse");

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].symbol, "MSFT");
        assert_eq!(events[1].symbol, "NVDA");
        assert!(events
            .iter()
            .all(|event| event.event_type == CorporateEventType::Earnings));
        assert!(events.iter().all(|event| event.fiscal_period.is_none()));
        assert!(events.iter().all(|event| {
            event.source.provider_id == "alpha_vantage"
                && event.source.source_kind == CorporateEventSourceKind::EarningsCalendar
                && event
                    .source
                    .source_url
                    .as_deref()
                    .is_some_and(|url| !url.contains("apikey"))
        }));
    }

    #[test]
    fn parser_rejects_invalid_date_for_requested_symbol() {
        let raw = "symbol,name,reportDate,fiscalDateEnding,estimate,currency\nNVDA,NVIDIA,not-a-date,2026-07-31,,USD\n";

        let error = parse_alpha_vantage_earnings_calendar(raw, &["NVDA".to_string()], timestamp())
            .expect_err("invalid report date must fail closed");
        assert!(error.contains("report date"));
    }

    #[test]
    fn parser_rejects_invalid_fiscal_date_without_inventing_a_period() {
        let raw = "symbol,name,reportDate,fiscalDateEnding,estimate,currency\nNVDA,NVIDIA,2026-08-27,not-a-date,,USD\n";

        let error = parse_alpha_vantage_earnings_calendar(raw, &["NVDA".to_string()], timestamp())
            .expect_err("invalid fiscal date must fail closed");
        assert!(error.contains("fiscal date"));
    }

    #[test]
    fn unknown_symbols_are_skipped_and_missing_matching_events_remain_healthy() {
        let raw = "symbol,name,reportDate,fiscalDateEnding,estimate,currency\nUNKNOWN,Unknown,2026-08-27,2026-07-31,,USD\n";

        let events = parse_alpha_vantage_earnings_calendar(raw, &["NVDA".to_string()], timestamp())
            .expect("unknown symbols should be skipped");
        assert!(events.is_empty());
    }

    #[test]
    fn provider_makes_one_three_month_request_and_returns_healthy_expected_events() {
        let dir = tempfile::tempdir().expect("temporary directory must be created");
        let calls = Arc::new(Mutex::new(Vec::new()));
        let provider = provider(
            "test-key",
            &dir.path().join("calendar.json"),
            FIXTURE,
            calls.clone(),
        );

        let read_model = provider.load_for_universe(&symbols());

        assert_eq!(
            read_model.health,
            ExpectedCorporateEventProviderHealth::Healthy
        );
        assert_eq!(read_model.events.len(), 2);
        assert_eq!(read_model.fetched_at, Some(timestamp()));
        assert_eq!(
            calls.lock().unwrap().as_slice(),
            [("3month".to_string(), "test-key".to_string())]
        );
    }

    #[test]
    fn missing_key_fails_before_transport_and_does_not_become_no_event() {
        let dir = tempfile::tempdir().expect("temporary directory must be created");
        let calls = Arc::new(Mutex::new(Vec::new()));
        let provider = provider(
            "  ",
            &dir.path().join("calendar.json"),
            FIXTURE,
            calls.clone(),
        );

        let read_model = provider.load_for_universe(&symbols());

        assert_eq!(
            read_model.health,
            ExpectedCorporateEventProviderHealth::Unavailable
        );
        assert!(read_model.events.is_empty());
        assert_eq!(
            read_model.diagnostic.as_deref(),
            Some("Alpha Vantage API key is not configured")
        );
        assert!(calls.lock().unwrap().is_empty());
    }

    #[test]
    fn quota_error_json_is_unavailable() {
        let dir = tempfile::tempdir().expect("temporary directory must be created");
        let calls = Arc::new(Mutex::new(Vec::new()));
        let provider = provider(
            "test-key",
            &dir.path().join("calendar.json"),
            r#"{"Note":"Thank you for using Alpha Vantage! Our standard API call frequency is 25 requests per day."}"#,
            calls,
        );

        let read_model = provider.load_for_universe(&symbols());

        assert_eq!(
            read_model.health,
            ExpectedCorporateEventProviderHealth::Unavailable
        );
        assert!(read_model.events.is_empty());
        assert!(read_model
            .diagnostic
            .as_deref()
            .unwrap()
            .contains("error payload"));
    }

    #[test]
    fn non_success_http_response_is_unavailable() {
        let dir = tempfile::tempdir().expect("temporary directory must be created");
        let calls = Arc::new(Mutex::new(Vec::new()));
        let transport = StubTransport::status(429, "rate limited", calls);
        let provider = AlphaVantageExpectedEventProvider::with_transport(
            "test-key",
            dir.path().join("calendar.json"),
            timestamp(),
            transport,
        );

        let read_model = provider.load_for_universe(&symbols());

        assert_eq!(
            read_model.health,
            ExpectedCorporateEventProviderHealth::Unavailable
        );
        assert!(read_model.events.is_empty());
        assert!(read_model
            .diagnostic
            .as_deref()
            .unwrap()
            .contains("HTTP 429"));
    }

    #[test]
    fn malformed_csv_is_unavailable() {
        let dir = tempfile::tempdir().expect("temporary directory must be created");
        let calls = Arc::new(Mutex::new(Vec::new()));
        let provider = provider(
            "test-key",
            &dir.path().join("calendar.json"),
            "not,csv\n",
            calls,
        );

        let read_model = provider.load_for_universe(&symbols());

        assert_eq!(
            read_model.health,
            ExpectedCorporateEventProviderHealth::Unavailable
        );
        assert!(read_model.events.is_empty());
    }

    #[test]
    fn fresh_cache_hit_does_not_repeat_the_request_and_preserves_cache_metadata() {
        let dir = tempfile::tempdir().expect("temporary directory must be created");
        let cache_path = dir.path().join("calendar.json");
        let first_calls = Arc::new(Mutex::new(Vec::new()));
        let first = provider("test-key", &cache_path, FIXTURE, first_calls.clone());
        let first_model = first.load_for_universe(&symbols());
        assert_eq!(first_model.events.len(), 2);

        let second_calls = Arc::new(Mutex::new(Vec::new()));
        let second = provider("test-key", &cache_path, "malformed", second_calls.clone());
        let second_model = second.load_for_universe(&symbols());

        assert_eq!(
            second_model.health,
            ExpectedCorporateEventProviderHealth::Healthy
        );
        assert_eq!(second_model.events, first_model.events);
        assert!(second_calls.lock().unwrap().is_empty());
        let cache = std::fs::read_to_string(cache_path).expect("cache must be readable");
        assert!(cache.contains("\"fetched_at\""));
        assert!(cache.contains("\"provider\": \"alpha_vantage\""));
        assert!(cache.contains("\"schema_version\": 1"));
        assert!(cache.contains("\"records\""));
    }

    #[test]
    fn expired_cache_refreshes_but_never_returns_stale_events_on_failure() {
        let dir = tempfile::tempdir().expect("temporary directory must be created");
        let cache_path = dir.path().join("calendar.json");
        let initial_calls = Arc::new(Mutex::new(Vec::new()));
        let initial = provider("test-key", &cache_path, FIXTURE, initial_calls);
        assert_eq!(initial.load_for_universe(&symbols()).events.len(), 2);

        let calls = Arc::new(Mutex::new(Vec::new()));
        let failing = AlphaVantageExpectedEventProvider::with_transport(
            "test-key",
            cache_path,
            timestamp() + chrono::Duration::hours(25),
            StubTransport::error("timeout", calls.clone()),
        );
        let read_model = failing.load_for_universe(&symbols());

        assert_eq!(
            read_model.health,
            ExpectedCorporateEventProviderHealth::Unavailable
        );
        assert!(read_model.events.is_empty());
        assert_eq!(
            read_model.diagnostic.as_deref(),
            Some("Alpha Vantage request failed: timeout")
        );
        assert_eq!(calls.lock().unwrap().len(), 1);
    }

    #[test]
    fn malformed_cache_is_unavailable_without_network_fallback() {
        let dir = tempfile::tempdir().expect("temporary directory must be created");
        let cache_path = dir.path().join("calendar.json");
        std::fs::write(&cache_path, "not-json").expect("malformed cache must be written");
        let calls = Arc::new(Mutex::new(Vec::new()));
        let provider = provider("test-key", &cache_path, FIXTURE, calls.clone());

        let read_model = provider.load_for_universe(&symbols());

        assert_eq!(
            read_model.health,
            ExpectedCorporateEventProviderHealth::Unavailable
        );
        assert!(read_model.events.is_empty());
        assert!(read_model.diagnostic.as_deref().unwrap().contains("cache"));
        assert!(calls.lock().unwrap().is_empty());
    }

    #[test]
    fn cache_write_failure_is_unavailable() {
        let dir = tempfile::tempdir().expect("temporary directory must be created");
        let parent_file = dir.path().join("parent-file");
        std::fs::write(&parent_file, "not-a-directory").expect("parent file must be written");
        let calls = Arc::new(Mutex::new(Vec::new()));
        let provider = provider(
            "test-key",
            &parent_file.join("calendar.json"),
            FIXTURE,
            calls,
        );

        let read_model = provider.load_for_universe(&symbols());

        assert_eq!(
            read_model.health,
            ExpectedCorporateEventProviderHealth::Unavailable
        );
        assert!(read_model.events.is_empty());
        assert!(read_model.diagnostic.as_deref().unwrap().contains("cache"));
    }

    #[test]
    fn empty_universe_is_unavailable_before_request() {
        let dir = tempfile::tempdir().expect("temporary directory must be created");
        let calls = Arc::new(Mutex::new(Vec::new()));
        let provider = provider(
            "test-key",
            &dir.path().join("calendar.json"),
            FIXTURE,
            calls.clone(),
        );

        let read_model = provider.load_for_universe(&[]);

        assert_eq!(
            read_model.health,
            ExpectedCorporateEventProviderHealth::Unavailable
        );
        assert!(read_model.events.is_empty());
        assert!(calls.lock().unwrap().is_empty());
    }
}
