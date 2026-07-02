use crate::features::research::interface::macro_event_observation::{
    FutureCalendarObservation, MacroEventSourceHealth,
};
use chrono::NaiveDate;
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MacroEventCalendarReadModel {
    pub as_of_date: NaiveDate,
    pub source_health: MacroEventSourceHealth,
    pub source: String,
    pub source_url: Option<String>,
    pub observations: Vec<FutureCalendarObservation>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum MacroEventDocument {
    Single(Box<FutureCalendarObservation>),
    Multiple(Vec<FutureCalendarObservation>),
}

#[allow(dead_code)]
pub(crate) trait MacroEventCalendarAdapter {
    fn load_from_json(&self, path: &Path, as_of_date: NaiveDate) -> MacroEventCalendarReadModel;
}

#[allow(dead_code)]
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct FileMacroEventCalendarAdapter;

impl MacroEventCalendarAdapter for FileMacroEventCalendarAdapter {
    fn load_from_json(&self, path: &Path, as_of_date: NaiveDate) -> MacroEventCalendarReadModel {
        load_macro_event_calendar_from_json(path, as_of_date)
    }
}

pub(crate) fn load_macro_event_calendar_from_json(
    path: &Path,
    as_of_date: NaiveDate,
) -> MacroEventCalendarReadModel {
    let source = path.display().to_string();
    let Ok(raw) = fs::read_to_string(path) else {
        return MacroEventCalendarReadModel::unavailable(as_of_date, source);
    };
    let Ok(document) = serde_json::from_str::<MacroEventDocument>(&raw) else {
        return MacroEventCalendarReadModel::unavailable(as_of_date, source);
    };
    match document {
        MacroEventDocument::Single(observation) => {
            MacroEventCalendarReadModel::from_observations(as_of_date, source, vec![*observation])
        }
        MacroEventDocument::Multiple(observations) => {
            MacroEventCalendarReadModel::from_observations(as_of_date, source, observations)
        }
    }
}

#[cfg(test)]
pub(crate) fn load_macro_event_calendar_from_json_str(
    raw: &str,
    as_of_date: NaiveDate,
    source: impl Into<String>,
) -> MacroEventCalendarReadModel {
    let source = source.into();
    let Ok(document) = serde_json::from_str::<MacroEventDocument>(raw) else {
        return MacroEventCalendarReadModel::unavailable(as_of_date, source);
    };
    match document {
        MacroEventDocument::Single(observation) => {
            MacroEventCalendarReadModel::from_observations(as_of_date, source, vec![*observation])
        }
        MacroEventDocument::Multiple(observations) => {
            MacroEventCalendarReadModel::from_observations(as_of_date, source, observations)
        }
    }
}

impl MacroEventCalendarReadModel {
    pub(crate) fn unavailable(as_of_date: NaiveDate, source: String) -> Self {
        Self {
            as_of_date,
            source_health: MacroEventSourceHealth::Unavailable,
            source,
            source_url: None,
            observations: Vec::new(),
        }
    }

    pub(crate) fn from_observations<T>(
        as_of_date: NaiveDate,
        source: String,
        observations: Vec<T>,
    ) -> Self
    where
        T: Into<FutureCalendarObservation>,
    {
        let observations = observations.into_iter().map(Into::into).collect::<Vec<_>>();
        let source_health = derive_source_health(&observations);
        let source_url = observations
            .first()
            .map(|observation| observation.source_url.clone());
        Self {
            as_of_date,
            source_health,
            source,
            source_url,
            observations,
        }
    }
}

fn derive_source_health(observations: &[FutureCalendarObservation]) -> MacroEventSourceHealth {
    if observations.is_empty() {
        return MacroEventSourceHealth::Unavailable;
    }

    if observations
        .iter()
        .any(|observation| observation.source_health == MacroEventSourceHealth::Partial)
    {
        return MacroEventSourceHealth::Partial;
    }

    if observations
        .iter()
        .all(|observation| observation.source_health == MacroEventSourceHealth::Succeeded)
    {
        MacroEventSourceHealth::Succeeded
    } else if observations
        .iter()
        .any(|observation| observation.source_health == MacroEventSourceHealth::Succeeded)
    {
        MacroEventSourceHealth::Partial
    } else {
        MacroEventSourceHealth::Unavailable
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::research::interface::macro_event_observation::{
        MacroEventImportance, MacroEventInformationContent, MacroEventLifecycle,
        MacroEventObservation, MacroEventSourceHealth, MacroEventType,
    };

    fn build_observation(source_health: MacroEventSourceHealth) -> MacroEventObservation {
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
            source_url: "https://www.federalreserve.gov/monetarypolicy/fomccalendars.htm"
                .to_string(),
            importance: MacroEventImportance::Critical,
            lifecycle: MacroEventLifecycle::Upcoming,
            expected_value: Some("5.25%".to_string()),
            actual_value: None,
            previous_value: Some("5.50%".to_string()),
            unit: Some("%".to_string()),
            surprise_state: crate::features::research::interface::macro_event_observation::MacroEventSurpriseState::NotAvailable,
            information_content: MacroEventInformationContent::High,
            source_health,
            observed_at: NaiveDate::from_ymd_opt(2026, 6, 17).unwrap(),
        }
    }

    #[test]
    fn loads_single_observation_from_json_str() {
        let read_model = load_macro_event_calendar_from_json_str(
            &serde_json::to_string(&build_observation(MacroEventSourceHealth::Succeeded)).unwrap(),
            NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            "inline",
        );

        assert_eq!(read_model.source_health, MacroEventSourceHealth::Succeeded);
        assert_eq!(read_model.observations.len(), 1);
    }

    #[test]
    fn parser_failure_returns_unavailable_read_model() {
        let read_model = load_macro_event_calendar_from_json_str(
            "{broken",
            NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            "broken",
        );

        assert_eq!(
            read_model.source_health,
            MacroEventSourceHealth::Unavailable
        );
        assert!(read_model.observations.is_empty());
    }
}
