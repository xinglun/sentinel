use crate::features::research::interface::cli_command_handler::build_official_calendar_smoke_payload;
use crate::features::research::interface::macro_event_calendar_adapter::{
    MacroEventCalendarReadModel, MacroEventSourceDiagnostic,
};
use crate::features::research::interface::macro_event_observation::{
    MacroEventImportance, MacroEventInformationContent, MacroEventLifecycle, MacroEventObservation,
    MacroEventSourceHealth, MacroEventSurpriseState, MacroEventType,
};
use crate::features::research::interface::macro_event_official_calendar_adapter::{
    official_calendar_smoke_summary, OfficialCalendarSmokeSummary,
};
use chrono::NaiveDate;

fn sample_observation() -> MacroEventObservation {
    MacroEventObservation {
        event_id: "fomc-2026-06-18".to_string(),
        as_of_date: NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
        event_date: NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
        event_time: Some("14:00".to_string()),
        timezone: "America/New_York".to_string(),
        country: "US".to_string(),
        event_type: MacroEventType::FomcRateDecision,
        event_name: "FOMC Rate Decision".to_string(),
        source: "Federal Reserve".to_string(),
        source_url: "https://www.federalreserve.gov/monetarypolicy/fomccalendars.htm".to_string(),
        importance: MacroEventImportance::Critical,
        lifecycle: MacroEventLifecycle::Upcoming,
        expected_value: Some("5.25%".to_string()),
        actual_value: None,
        previous_value: Some("5.50%".to_string()),
        unit: Some("%".to_string()),
        surprise_state: MacroEventSurpriseState::NotAvailable,
        information_content: MacroEventInformationContent::High,
        source_health: MacroEventSourceHealth::Succeeded,
        observed_at: NaiveDate::from_ymd_opt(2026, 6, 17).unwrap(),
    }
}

#[test]
fn official_calendar_smoke_summary_keeps_source_diagnostics() {
    let read_model = MacroEventCalendarReadModel::from_observations_with_stats_and_diagnostics(
        NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
        "official-calendar-connector".to_string(),
        vec![sample_observation()],
        4,
        3,
        1,
        Some("one official source failed to fetch".to_string()),
        vec![MacroEventSourceDiagnostic {
            family: "Federal Reserve Board".to_string(),
            label: "Fed calendar".to_string(),
            url: "https://www.federalreserve.gov/newsevents/calendar.htm".to_string(),
            fetch_health: MacroEventSourceHealth::Succeeded,
            observation_count: 1,
            note: "1 release(s)".to_string(),
        }],
    );

    let summary = official_calendar_smoke_summary(&read_model);

    assert_eq!(summary.source_diagnostics.len(), 1);
    assert_eq!(summary.source_diagnostics[0].label, "Fed calendar");
    assert_eq!(summary.source_diagnostics[0].observation_count, 1);
}

#[test]
fn official_calendar_smoke_payload_keeps_source_diagnostics() {
    let summary = OfficialCalendarSmokeSummary {
        as_of_date: NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
        source_health: MacroEventSourceHealth::Partial,
        source_attempts: 4,
        source_successes: 3,
        source_failures: 1,
        observation_count: 2,
        source_diagnostics: vec![MacroEventSourceDiagnostic {
            family: "Bureau of Labor Statistics".to_string(),
            label: "BLS CPI release".to_string(),
            url: "https://www.bls.gov/schedule/news_release/cpi.htm".to_string(),
            fetch_health: MacroEventSourceHealth::Succeeded,
            observation_count: 1,
            note: "1 release(s)".to_string(),
        }],
        diagnostic: Some("official source failed on one endpoint".to_string()),
    };

    let payload = build_official_calendar_smoke_payload(&summary);

    assert_eq!(payload["smoke"], "official-calendar");
    assert_eq!(payload["summary"]["source_health"], "PARTIAL");
    assert_eq!(
        payload["summary"]["source_diagnostics"][0]["label"],
        "BLS CPI release"
    );
    assert_eq!(
        payload["summary"]["diagnostic"],
        "official source failed on one endpoint"
    );
}
