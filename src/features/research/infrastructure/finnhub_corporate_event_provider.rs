use crate::config::AppConfig;
use crate::features::research::application::corporate_event_provider::{
    CorporateEventObservation, CorporateEventProvider, CorporateEventProviderHealth,
    CorporateEventProviderReadModel, CorporateEventReleaseWindow,
};
use chrono::{NaiveDate, SecondsFormat, Utc};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::HashSet;
use std::time::Duration;

const FINNHUB_SOURCE: &str = "Finnhub Earnings Calendar";
const FINNHUB_CALENDAR_URL: &str = "https://finnhub.io/api/v1/calendar/earnings";
const MARKET_TIMEZONE: &str = "America/New_York";

/// Finnhub HTTP 応答を provider port から分離するための transport 応答。
#[derive(Debug, Clone, PartialEq, Eq)]
struct FinnhubTransportResponse {
    status: u16,
    body: String,
}

/// Finnhub transport の差し替え境界。fixture と失敗系テストはネットワークを使わない。
trait FinnhubEarningsCalendarTransport: Clone {
    fn fetch(
        &self,
        from: NaiveDate,
        to: NaiveDate,
        api_key: &str,
    ) -> Result<FinnhubTransportResponse, String>;
}

#[derive(Clone)]
struct ReqwestFinnhubEarningsCalendarTransport {
    client: Client,
}

impl ReqwestFinnhubEarningsCalendarTransport {
    fn new() -> Result<Self, String> {
        Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map(|client| Self { client })
            .map_err(|error| format!("Finnhub HTTP client could not be created: {error}"))
    }
}

impl FinnhubEarningsCalendarTransport for ReqwestFinnhubEarningsCalendarTransport {
    fn fetch(
        &self,
        from: NaiveDate,
        to: NaiveDate,
        api_key: &str,
    ) -> Result<FinnhubTransportResponse, String> {
        let url = format!("{FINNHUB_CALENDAR_URL}?from={from}&to={to}");
        let response = self
            .client
            .get(url)
            .header("X-Finnhub-Token", api_key)
            .send()
            .map_err(|error| format!("Finnhub earnings request failed: {error}"))?;
        let status = response.status().as_u16();
        let body = response
            .text()
            .map_err(|error| format!("Finnhub earnings response could not be read: {error}"))?;
        Ok(FinnhubTransportResponse { status, body })
    }
}

#[derive(Clone)]
struct FinnhubCorporateEventProvider<T> {
    api_key: String,
    transport: T,
}

impl<T> FinnhubCorporateEventProvider<T>
where
    T: FinnhubEarningsCalendarTransport,
{
    fn with_transport(api_key: impl Into<String>, transport: T) -> Self {
        Self {
            api_key: api_key.into(),
            transport,
        }
    }
}

impl<T> CorporateEventProvider for FinnhubCorporateEventProvider<T>
where
    T: FinnhubEarningsCalendarTransport,
{
    fn load_for_market_date(
        &self,
        market_date: NaiveDate,
        symbols: &[String],
    ) -> CorporateEventProviderReadModel {
        let token = self.api_key.trim();
        if token.is_empty() {
            return CorporateEventProviderReadModel::unavailable(
                "Finnhub API key is not configured",
            );
        }
        if normalized_symbols(symbols).is_empty() {
            return CorporateEventProviderReadModel::unavailable(
                "Finnhub corporate event provider requires a non-empty observation universe",
            );
        }

        let response = match self.transport.fetch(market_date, market_date, token) {
            Ok(response) => response,
            Err(error) => {
                return unavailable_with_sanitized_diagnostic(token, error);
            }
        };
        if !(200..300).contains(&response.status) {
            return unavailable_with_sanitized_diagnostic(
                token,
                format!("Finnhub earnings API returned HTTP {}", response.status),
            );
        }
        let retrieved_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
        let mut events = match parse_finnhub_earnings_calendar(&response.body, market_date, symbols)
        {
            Ok(events) => events,
            Err(error) => return unavailable_with_sanitized_diagnostic(token, error),
        };
        for event in &mut events {
            event.observed_at = retrieved_at.clone();
        }
        CorporateEventProviderReadModel {
            health: CorporateEventProviderHealth::Healthy,
            source: FINNHUB_SOURCE.to_string(),
            source_url: calendar_url(market_date),
            retrieved_at,
            diagnostic: None,
            events,
        }
    }
}

pub(crate) fn load_finnhub_corporate_events(
    app_config: &AppConfig,
    market_date: NaiveDate,
    symbols: &[String],
) -> CorporateEventProviderReadModel {
    let Some(api_key) = app_config
        .finnhub
        .as_ref()
        .map(|config| config.finnhub_api_key.trim())
        .filter(|key| !key.is_empty())
    else {
        return CorporateEventProviderReadModel::unavailable("Finnhub API key is not configured");
    };
    let transport = match ReqwestFinnhubEarningsCalendarTransport::new() {
        Ok(transport) => transport,
        Err(error) => return CorporateEventProviderReadModel::unavailable(error),
    };
    FinnhubCorporateEventProvider::with_transport(api_key, transport)
        .load_for_market_date(market_date, symbols)
}

fn calendar_url(market_date: NaiveDate) -> String {
    format!("{FINNHUB_CALENDAR_URL}?from={market_date}&to={market_date}")
}

fn unavailable_with_sanitized_diagnostic(
    api_key: &str,
    diagnostic: impl Into<String>,
) -> CorporateEventProviderReadModel {
    CorporateEventProviderReadModel::unavailable(diagnostic.into().replace(api_key, "[REDACTED]"))
}

#[derive(Debug, Deserialize)]
struct FinnhubEarningsCalendarResponse {
    #[serde(rename = "earningsCalendar")]
    earnings_calendar: Vec<FinnhubEarningsCalendarRecord>,
}

#[derive(Debug, Deserialize)]
struct FinnhubEarningsCalendarRecord {
    date: String,
    symbol: String,
    hour: String,
    quarter: u8,
    year: i32,
    #[serde(rename = "epsActual", default)]
    eps_actual: Option<f64>,
    #[serde(rename = "epsEstimate", default)]
    eps_estimate: Option<f64>,
    #[serde(rename = "revenueActual", default)]
    revenue_actual: Option<f64>,
    #[serde(rename = "revenueEstimate", default)]
    revenue_estimate: Option<f64>,
}

fn parse_finnhub_earnings_calendar(
    raw: &str,
    market_date: NaiveDate,
    symbols: &[String],
) -> Result<Vec<CorporateEventObservation>, String> {
    let response = serde_json::from_str::<FinnhubEarningsCalendarResponse>(raw)
        .map_err(|error| format!("Finnhub earnings response could not be parsed: {error}"))?;
    let symbols = normalized_symbols(symbols);
    if symbols.is_empty() {
        return Err(
            "Finnhub corporate event provider requires a non-empty observation universe"
                .to_string(),
        );
    }
    let source_url =
        format!("https://finnhub.io/api/v1/calendar/earnings?from={market_date}&to={market_date}");
    let mut events = Vec::new();

    for record in response.earnings_calendar {
        let symbol = record.symbol.trim().to_ascii_uppercase();
        if symbol.is_empty() {
            return Err("Finnhub earnings response has an empty symbol".to_string());
        }
        let event_date = NaiveDate::parse_from_str(record.date.trim(), "%Y-%m-%d")
            .map_err(|error| format!("Finnhub earnings response has invalid date: {error}"))?;
        let release_window = parse_release_window(record.hour.trim())?;
        if !symbols.contains(&symbol) {
            continue;
        }
        if event_date != market_date {
            continue;
        }
        if !(1..=4).contains(&record.quarter) || !(1900..=9999).contains(&record.year) {
            return Err(format!(
                "Finnhub earnings response has invalid fiscal period: FY{} Q{}",
                record.year, record.quarter
            ));
        }
        events.push(CorporateEventObservation {
            symbol,
            market_date: event_date,
            market_timezone: MARKET_TIMEZONE.to_string(),
            release_window,
            fiscal_quarter: record.quarter,
            fiscal_year: record.year,
            eps_actual: record.eps_actual,
            eps_estimate: record.eps_estimate,
            revenue_actual: record.revenue_actual,
            revenue_estimate: record.revenue_estimate,
            source: FINNHUB_SOURCE.to_string(),
            source_url: source_url.clone(),
            observed_at: String::new(),
        });
    }

    events.sort_by(|left, right| {
        left.symbol
            .cmp(&right.symbol)
            .then_with(|| left.fiscal_year.cmp(&right.fiscal_year))
            .then_with(|| left.fiscal_quarter.cmp(&right.fiscal_quarter))
    });
    Ok(events)
}

fn normalized_symbols(symbols: &[String]) -> HashSet<String> {
    symbols
        .iter()
        .map(|symbol| symbol.trim().to_ascii_uppercase())
        .filter(|symbol| !symbol.is_empty())
        .collect()
}

fn parse_release_window(value: &str) -> Result<CorporateEventReleaseWindow, String> {
    match value.to_ascii_lowercase().as_str() {
        "bmo" => Ok(CorporateEventReleaseWindow::BeforeMarketOpen),
        "amc" => Ok(CorporateEventReleaseWindow::AfterMarketClose),
        "dmh" => Ok(CorporateEventReleaseWindow::DuringMarketHours),
        other => Err(format!(
            "unsupported Finnhub earnings release window: {other}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        parse_finnhub_earnings_calendar, FinnhubCorporateEventProvider,
        FinnhubEarningsCalendarTransport, FinnhubTransportResponse,
    };
    use crate::features::research::application::corporate_event_provider::{
        CorporateEventProvider, CorporateEventProviderHealth, CorporateEventReleaseWindow,
    };
    use chrono::{DateTime, NaiveDate};
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct StubTransport {
        response: Result<FinnhubTransportResponse, String>,
    }

    impl StubTransport {
        fn response(status: u16, body: &str) -> Self {
            Self {
                response: Ok(FinnhubTransportResponse {
                    status,
                    body: body.to_string(),
                }),
            }
        }

        fn error(message: &str) -> Self {
            Self {
                response: Err(message.to_string()),
            }
        }
    }

    impl FinnhubEarningsCalendarTransport for StubTransport {
        fn fetch(
            &self,
            _from: NaiveDate,
            _to: NaiveDate,
            _api_key: &str,
        ) -> Result<FinnhubTransportResponse, String> {
            self.response.clone()
        }
    }

    #[derive(Clone)]
    struct CapturingTransport {
        response: FinnhubTransportResponse,
        calls: Arc<Mutex<Vec<(NaiveDate, NaiveDate, String)>>>,
    }

    impl FinnhubEarningsCalendarTransport for CapturingTransport {
        fn fetch(
            &self,
            from: NaiveDate,
            to: NaiveDate,
            api_key: &str,
        ) -> Result<FinnhubTransportResponse, String> {
            self.calls
                .lock()
                .expect("call log lock must not be poisoned")
                .push((from, to, api_key.to_string()));
            Ok(self.response.clone())
        }
    }

    #[test]
    fn parses_nvidia_earnings_fixture() {
        let raw = include_str!(
            "../../../../tests/fixtures/corporate_events/finnhub-2026-08-27-nvidia-earnings.json"
        );
        let events = parse_finnhub_earnings_calendar(
            raw,
            NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
            &["NVDA".to_string()],
        )
        .expect("fixture must parse");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].symbol, "NVDA");
        assert_eq!(events[0].market_date.to_string(), "2026-08-27");
        assert_eq!(events[0].market_timezone, "America/New_York");
        assert_eq!(
            events[0].release_window,
            CorporateEventReleaseWindow::AfterMarketClose
        );
        assert_eq!(events[0].fiscal_quarter, 2);
        assert_eq!(events[0].fiscal_year, 2027);
        assert_eq!(events[0].revenue_actual, Some(96_200_000_000.0));
        assert!(events[0].observed_at.is_empty());
    }

    #[test]
    fn rejects_malformed_finnhub_payloads() {
        let date = NaiveDate::from_ymd_opt(2026, 8, 27).unwrap();
        let symbols = ["NVDA".to_string()];
        assert!(parse_finnhub_earnings_calendar("{", date, &symbols).is_err());
        assert!(parse_finnhub_earnings_calendar("{}", date, &symbols).is_err());
        assert!(parse_finnhub_earnings_calendar(
            r#"{"earningsCalendar":[{"date":"not-a-date","symbol":"NVDA","hour":"amc","quarter":2,"year":2027}]}"#,
            date,
            &symbols,
        )
        .is_err());
        assert!(parse_finnhub_earnings_calendar(
            r#"{"earningsCalendar":[{"date":"2026-08-27","symbol":"NVDA","hour":"unknown","quarter":2,"year":2027}]}"#,
            date,
            &symbols,
        )
        .is_err());
    }

    #[test]
    fn rejects_invalid_fiscal_periods() {
        let date = NaiveDate::from_ymd_opt(2026, 8, 27).unwrap();
        for quarter in [0, 5] {
            let payload = format!(
                r#"{{"earningsCalendar":[{{"date":"{date}","symbol":"NVDA","hour":"amc","quarter":{quarter},"year":2027}}]}}"#
            );
            assert!(
                parse_finnhub_earnings_calendar(&payload, date, &["NVDA".to_string()]).is_err()
            );
        }
        assert!(parse_finnhub_earnings_calendar(
            r#"{"earningsCalendar":[{"date":"2026-08-27","symbol":"NVDA","hour":"amc","quarter":2,"year":0}]}"#,
            date,
            &["NVDA".to_string()],
        )
        .is_err());
    }

    #[test]
    fn provider_rejects_an_empty_observation_universe_before_transport() {
        let provider = FinnhubCorporateEventProvider::with_transport(
            "test-token",
            StubTransport::error("transport must not be called"),
        );
        let read_model = provider.load_for_market_date(
            NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
            &["  ".to_string()],
        );

        assert_eq!(read_model.health, CorporateEventProviderHealth::Unavailable);
        assert_eq!(
            read_model.diagnostic.as_deref(),
            Some("Finnhub corporate event provider requires a non-empty observation universe")
        );
    }

    #[test]
    fn provider_passes_date_range_and_token_only_to_transport_header_boundary() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let raw = r#"{"earningsCalendar":[]}"#;
        let provider = FinnhubCorporateEventProvider::with_transport(
            "test-token",
            CapturingTransport {
                response: FinnhubTransportResponse {
                    status: 200,
                    body: raw.to_string(),
                },
                calls: Arc::clone(&calls),
            },
        );
        let date = NaiveDate::from_ymd_opt(2026, 8, 27).unwrap();

        let read_model = provider.load_for_market_date(date, &["NVDA".to_string()]);

        assert_eq!(read_model.health, CorporateEventProviderHealth::Healthy);
        assert_eq!(
            calls.lock().unwrap().as_slice(),
            &[(date, date, "test-token".to_string())]
        );
        assert!(DateTime::parse_from_rfc3339(&read_model.retrieved_at).is_ok());
        assert!(!read_model.source_url.contains("test-token"));
    }

    #[test]
    fn ignores_valid_events_outside_the_requested_market_date() {
        let raw = r#"{"earningsCalendar":[{"date":"2026-08-26","symbol":"NVDA","hour":"bmo","quarter":2,"year":2027}]}"#;
        let events = parse_finnhub_earnings_calendar(
            raw,
            NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
            &["NVDA".to_string()],
        )
        .expect("valid no-event response must remain healthy");

        assert!(events.is_empty());
    }

    #[test]
    fn provider_returns_healthy_read_model_for_valid_response() {
        let raw = include_str!(
            "../../../../tests/fixtures/corporate_events/finnhub-2026-08-27-nvidia-earnings.json"
        );
        let provider = FinnhubCorporateEventProvider::with_transport(
            "test-token",
            StubTransport::response(200, raw),
        );
        let read_model = provider.load_for_market_date(
            NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
            &["NVDA".to_string()],
        );

        assert_eq!(read_model.health, CorporateEventProviderHealth::Healthy);
        assert_eq!(read_model.events.len(), 1);
        assert!(read_model.diagnostic.is_none());
        assert!(DateTime::parse_from_rfc3339(&read_model.retrieved_at).is_ok());
        assert!(DateTime::parse_from_rfc3339(&read_model.events[0].observed_at).is_ok());
        assert!(!read_model.source_url.contains("test-token"));
    }

    #[test]
    fn provider_fails_closed_when_credential_is_missing() {
        let provider = FinnhubCorporateEventProvider::with_transport(
            "  ",
            StubTransport::error("transport must not be called"),
        );
        let read_model = provider.load_for_market_date(
            NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
            &["NVDA".to_string()],
        );

        assert_eq!(read_model.health, CorporateEventProviderHealth::Unavailable);
        assert!(read_model.events.is_empty());
        assert_eq!(
            read_model.diagnostic.as_deref(),
            Some("Finnhub API key is not configured")
        );
    }

    #[test]
    fn provider_fails_closed_for_http_and_transport_errors_without_token_leak() {
        let date = NaiveDate::from_ymd_opt(2026, 8, 27).unwrap();
        for status in [401, 403, 429, 500] {
            let provider = FinnhubCorporateEventProvider::with_transport(
                "test-token",
                StubTransport::response(status, "provider error"),
            );
            let read_model = provider.load_for_market_date(date, &["NVDA".to_string()]);
            assert_eq!(read_model.health, CorporateEventProviderHealth::Unavailable);
            assert!(read_model.events.is_empty());
            assert!(!read_model
                .diagnostic
                .as_deref()
                .unwrap_or_default()
                .contains("test-token"));
        }

        let provider = FinnhubCorporateEventProvider::with_transport(
            "test-token",
            StubTransport::error("transport failed token=test-token"),
        );
        let read_model = provider.load_for_market_date(date, &["NVDA".to_string()]);
        assert_eq!(read_model.health, CorporateEventProviderHealth::Unavailable);
        assert!(!read_model
            .diagnostic
            .as_deref()
            .unwrap_or_default()
            .contains("test-token"));
    }

    #[test]
    fn provider_fails_closed_for_malformed_success_payload() {
        let provider = FinnhubCorporateEventProvider::with_transport(
            "test-token",
            StubTransport::response(200, "not-json"),
        );
        let read_model = provider.load_for_market_date(
            NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
            &["NVDA".to_string()],
        );

        assert_eq!(read_model.health, CorporateEventProviderHealth::Unavailable);
        assert!(read_model.events.is_empty());
    }
}
