use crate::features::research::interface::corporate_event_provider::CorporateEventObservation;
use crate::features::research::interface::corporate_event_provider::CorporateEventReleaseWindow;
use chrono::NaiveDate;
use serde::Deserialize;
use std::collections::HashSet;

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

const FINNHUB_SOURCE: &str = "Finnhub Earnings Calendar";
const MARKET_TIMEZONE: &str = "America/New_York";

fn parse_finnhub_earnings_calendar(
    raw: &str,
    market_date: NaiveDate,
    symbols: &[String],
) -> Result<Vec<CorporateEventObservation>, String> {
    let response = serde_json::from_str::<FinnhubEarningsCalendarResponse>(raw)
        .map_err(|error| format!("Finnhub earnings response could not be parsed: {error}"))?;
    let symbols = symbols
        .iter()
        .map(|symbol| symbol.trim().to_ascii_uppercase())
        .filter(|symbol| !symbol.is_empty())
        .collect::<HashSet<_>>();
    let source_url = format!(
        "https://finnhub.io/api/v1/calendar/earnings?from={market_date}&to={market_date}"
    );
    let mut events = Vec::new();

    for record in response.earnings_calendar {
        let symbol = record.symbol.trim().to_ascii_uppercase();
        if symbol.is_empty() {
            return Err("Finnhub earnings response has an empty symbol".to_string());
        }
        let event_date = NaiveDate::parse_from_str(record.date.trim(), "%Y-%m-%d")
            .map_err(|error| format!("Finnhub earnings response has invalid date: {error}"))?;
        let release_window = parse_release_window(record.hour.trim())?;
        if !symbols.is_empty() && !symbols.contains(&symbol) {
            continue;
        }
        if event_date != market_date {
            continue;
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
            observed_at: format!("{market_date}T00:00:00Z"),
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

fn parse_release_window(value: &str) -> Result<CorporateEventReleaseWindow, String> {
    match value.to_ascii_lowercase().as_str() {
        "bmo" => Ok(CorporateEventReleaseWindow::BeforeMarketOpen),
        "amc" => Ok(CorporateEventReleaseWindow::AfterMarketClose),
        "dmh" => Ok(CorporateEventReleaseWindow::DuringMarketHours),
        other => Err(format!("unsupported Finnhub earnings release window: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_finnhub_earnings_calendar;
    use crate::features::research::interface::corporate_event_provider::CorporateEventReleaseWindow;
    use chrono::NaiveDate;

    #[test]
    fn parses_nvidia_earnings_fixture() {
        let raw = include_str!("../../../../tests/fixtures/corporate_events/finnhub-2026-08-27-nvidia-earnings.json");
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
    }

    #[test]
    fn rejects_malformed_finnhub_payloads() {
        let date = NaiveDate::from_ymd_opt(2026, 8, 27).unwrap();
        assert!(parse_finnhub_earnings_calendar("{", date, &[]).is_err());
        assert!(parse_finnhub_earnings_calendar("{}", date, &[]).is_err());
        assert!(parse_finnhub_earnings_calendar(
            r#"{"earningsCalendar":[{"date":"not-a-date","symbol":"NVDA","hour":"amc","quarter":2,"year":2027}]}"#,
            date,
            &[],
        )
        .is_err());
        assert!(parse_finnhub_earnings_calendar(
            r#"{"earningsCalendar":[{"date":"2026-08-27","symbol":"NVDA","hour":"unknown","quarter":2,"year":2027}]}"#,
            date,
            &[],
        )
        .is_err());
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
}
