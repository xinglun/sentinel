use crate::features::research::acl::macro_event_calendar_file_reader::read_macro_event_calendar_text;
use crate::features::research::interface::macro_event_observation::{
    FutureCalendarObservation, MacroEventSourceHealth,
};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MacroEventCalendarReadModel {
    pub as_of_date: NaiveDate,
    pub source_health: MacroEventSourceHealth,
    pub source: String,
    pub source_url: Option<String>,
    pub source_attempts: usize,
    pub source_successes: usize,
    pub source_failures: usize,
    pub diagnostic: Option<String>,
    pub source_diagnostics: Vec<MacroEventSourceDiagnostic>,
    pub observations: Vec<FutureCalendarObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct MacroEventSourceDiagnostic {
    pub family: String,
    pub label: String,
    pub url: String,
    pub fetch_health: MacroEventSourceHealth,
    pub observation_count: usize,
    pub note: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum MacroEventDocument {
    Single(Box<FutureCalendarObservation>),
    Multiple(Vec<FutureCalendarObservation>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MacroEventCalendarSourceStrategy {
    Official,
    Json { path: Option<PathBuf> },
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

pub(crate) fn load_macro_event_calendar_with_strategy(
    strategy: MacroEventCalendarSourceStrategy,
    as_of_date: NaiveDate,
) -> MacroEventCalendarReadModel {
    match strategy {
        MacroEventCalendarSourceStrategy::Official => {
            crate::features::research::interface::macro_event_official_calendar_adapter::load_official_future_calendar(as_of_date)
        }
        MacroEventCalendarSourceStrategy::Json { path } => path
            .as_deref()
            .map(|path| load_macro_event_calendar_from_json(path, as_of_date))
            .unwrap_or_else(|| {
                MacroEventCalendarReadModel::failed(
                    as_of_date,
                    "env://SENTINEL_MACRO_EVENT_CALENDAR_JSON_PATH".to_string(),
                    "json calendar path is not configured".to_string(),
                )
            }),
    }
}

pub(crate) fn load_macro_event_calendar_from_env(
    as_of_date: NaiveDate,
) -> MacroEventCalendarReadModel {
    load_macro_event_calendar_with_strategy(
        resolve_macro_event_calendar_source_strategy(),
        as_of_date,
    )
}

pub(crate) fn resolve_macro_event_calendar_source_strategy() -> MacroEventCalendarSourceStrategy {
    let source =
        env::var("SENTINEL_MACRO_EVENT_CALENDAR_SOURCE").unwrap_or_else(|_| "official".to_string());
    let normalized = source.trim();
    if normalized.eq_ignore_ascii_case("official") {
        MacroEventCalendarSourceStrategy::Official
    } else if let Some(path) = normalized.strip_prefix("json:") {
        MacroEventCalendarSourceStrategy::Json {
            path: Some(PathBuf::from(path.trim())),
        }
    } else if normalized.eq_ignore_ascii_case("json") {
        MacroEventCalendarSourceStrategy::Json {
            path: env::var("SENTINEL_MACRO_EVENT_CALENDAR_JSON_PATH")
                .ok()
                .map(PathBuf::from),
        }
    } else {
        MacroEventCalendarSourceStrategy::Official
    }
}

pub(crate) fn load_macro_event_calendar_from_json(
    path: &Path,
    as_of_date: NaiveDate,
) -> MacroEventCalendarReadModel {
    let source = path.display().to_string();
    let Some(raw) = read_macro_event_calendar_text(path) else {
        return MacroEventCalendarReadModel::failed(
            as_of_date,
            source,
            "json calendar file could not be read".to_string(),
        );
    };
    let Ok(document) = serde_json::from_str::<MacroEventDocument>(&raw) else {
        return MacroEventCalendarReadModel::failed(
            as_of_date,
            source,
            "json calendar document could not be parsed".to_string(),
        );
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
        return MacroEventCalendarReadModel::failed(
            as_of_date,
            source,
            "json calendar document could not be parsed".to_string(),
        );
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
        Self::with_source_stats(as_of_date, source, None, 0, 0, 0, None)
    }

    pub(crate) fn failed(as_of_date: NaiveDate, source: String, diagnostic: String) -> Self {
        Self::with_source_stats(as_of_date, source, None, 1, 0, 1, Some(diagnostic))
    }

    pub(crate) fn from_observations<T>(
        as_of_date: NaiveDate,
        source: String,
        observations: Vec<T>,
    ) -> Self
    where
        T: Into<FutureCalendarObservation>,
    {
        Self::from_observations_with_stats(as_of_date, source, observations, 1, 1, 0, None)
    }

    pub(crate) fn from_observations_with_stats<T>(
        as_of_date: NaiveDate,
        source: String,
        observations: Vec<T>,
        source_attempts: usize,
        source_successes: usize,
        source_failures: usize,
        diagnostic: Option<String>,
    ) -> Self
    where
        T: Into<FutureCalendarObservation>,
    {
        Self::from_observations_with_stats_and_diagnostics(
            as_of_date,
            source,
            observations,
            source_attempts,
            source_successes,
            source_failures,
            diagnostic,
            Vec::new(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_observations_with_stats_and_diagnostics<T>(
        as_of_date: NaiveDate,
        source: String,
        observations: Vec<T>,
        source_attempts: usize,
        source_successes: usize,
        source_failures: usize,
        diagnostic: Option<String>,
        source_diagnostics: Vec<MacroEventSourceDiagnostic>,
    ) -> Self
    where
        T: Into<FutureCalendarObservation>,
    {
        let observations = observations.into_iter().map(Into::into).collect::<Vec<_>>();
        let source_health = derive_source_health(
            source_attempts,
            source_successes,
            source_failures,
            observations.is_empty(),
        );
        let source_url = observations
            .first()
            .map(|observation| observation.source_url.clone());
        Self {
            as_of_date,
            source_health,
            source,
            source_url,
            source_attempts,
            source_successes,
            source_failures,
            diagnostic,
            source_diagnostics,
            observations,
        }
    }

    fn with_source_stats(
        as_of_date: NaiveDate,
        source: String,
        source_url: Option<String>,
        source_attempts: usize,
        source_successes: usize,
        source_failures: usize,
        diagnostic: Option<String>,
    ) -> Self {
        Self {
            as_of_date,
            source_health: derive_source_health(
                source_attempts,
                source_successes,
                source_failures,
                false,
            ),
            source,
            source_url,
            source_attempts,
            source_successes,
            source_failures,
            diagnostic,
            source_diagnostics: Vec::new(),
            observations: Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn unavailable_with_diagnostic(
        as_of_date: NaiveDate,
        source: String,
        diagnostic: String,
    ) -> Self {
        Self::failed(as_of_date, source, diagnostic)
    }

    #[allow(dead_code)]
    pub(crate) fn with_diagnostic(
        as_of_date: NaiveDate,
        source: String,
        diagnostic: Option<String>,
    ) -> Self {
        Self {
            as_of_date,
            source_health: MacroEventSourceHealth::Unavailable,
            source,
            source_url: None,
            source_attempts: 0,
            source_successes: 0,
            source_failures: 0,
            diagnostic,
            source_diagnostics: Vec::new(),
            observations: Vec::new(),
        }
    }
}

fn derive_source_health(
    source_attempts: usize,
    source_successes: usize,
    source_failures: usize,
    has_observations: bool,
) -> MacroEventSourceHealth {
    if source_attempts == 0 && !has_observations {
        return MacroEventSourceHealth::Unavailable;
    }

    if source_failures > 0 && (source_successes > 0 || has_observations) {
        return MacroEventSourceHealth::Partial;
    }

    if source_successes > 0 || has_observations {
        MacroEventSourceHealth::Succeeded
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
    use tempfile::NamedTempFile;

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

    #[test]
    fn partial_source_health_is_preserved_with_diagnostics() {
        let observation = build_observation(MacroEventSourceHealth::Succeeded);
        let read_model = MacroEventCalendarReadModel::from_observations_with_stats(
            NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            "official-calendar-connector".to_string(),
            vec![observation],
            3,
            2,
            1,
            Some("one official source failed to fetch".to_string()),
        );

        assert_eq!(read_model.source_health, MacroEventSourceHealth::Partial);
        assert_eq!(read_model.source_attempts, 3);
        assert_eq!(read_model.source_successes, 2);
        assert_eq!(read_model.source_failures, 1);
        assert_eq!(
            read_model.diagnostic.as_deref(),
            Some("one official source failed to fetch")
        );
    }

    #[test]
    fn json_strategy_without_path_returns_diagnostic_failure() {
        let read_model = load_macro_event_calendar_with_strategy(
            MacroEventCalendarSourceStrategy::Json { path: None },
            NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
        );

        assert_eq!(
            read_model.source_health,
            MacroEventSourceHealth::Unavailable
        );
        assert_eq!(read_model.source_attempts, 1);
        assert_eq!(read_model.source_failures, 1);
        assert!(read_model
            .diagnostic
            .as_deref()
            .is_some_and(|value| value.contains("path is not configured")));
    }

    #[test]
    fn json_strategy_loads_replay_fixture() {
        let mut file = NamedTempFile::new().unwrap();
        let observation = build_observation(MacroEventSourceHealth::Succeeded);
        let raw = serde_json::to_string(&vec![observation]).unwrap();
        use std::io::Write;
        file.as_file_mut().write_all(raw.as_bytes()).unwrap();

        let read_model = load_macro_event_calendar_with_strategy(
            MacroEventCalendarSourceStrategy::Json {
                path: Some(file.path().to_path_buf()),
            },
            NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
        );

        assert_eq!(read_model.source_health, MacroEventSourceHealth::Succeeded);
        assert_eq!(read_model.observations.len(), 1);
    }

    #[test]
    fn json_strategy_without_path_returns_unavailable_read_model() {
        let read_model = load_macro_event_calendar_with_strategy(
            MacroEventCalendarSourceStrategy::Json { path: None },
            NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
        );

        assert_eq!(
            read_model.source_health,
            MacroEventSourceHealth::Unavailable
        );
        assert!(read_model.observations.is_empty());
    }

    #[test]
    fn json_prefix_strategy_embeds_path_from_source_selection() {
        let strategy = {
            let source = "json:/tmp/macro-calendar.json";
            if let Some(path) = source.strip_prefix("json:") {
                MacroEventCalendarSourceStrategy::Json {
                    path: Some(PathBuf::from(path)),
                }
            } else {
                MacroEventCalendarSourceStrategy::Official
            }
        };

        assert_eq!(
            strategy,
            MacroEventCalendarSourceStrategy::Json {
                path: Some(PathBuf::from("/tmp/macro-calendar.json"))
            }
        );
    }
}
