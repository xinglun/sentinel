use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

pub const OBSERVATION_TIMELINE_DAYS: usize = 7;
pub const SUMMARY_NO_STRUCTURAL_CHANGE: &str = "NO_STRUCTURAL_CHANGE";
pub const SUMMARY_STRUCTURAL_CHANGE: &str = "STRUCTURAL_CHANGE";
pub const SUMMARY_LIMITED_COVERAGE_NO_STRUCTURAL_CHANGE: &str =
    "LIMITED_COVERAGE_NO_STRUCTURAL_CHANGE";
pub const SUMMARY_LIMITED_COVERAGE_STRUCTURAL_CHANGE: &str = "LIMITED_COVERAGE_STRUCTURAL_CHANGE";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HistoryCoverage {
    Complete,
    Partial,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ObservationTimelineEntry {
    pub date: NaiveDate,
    pub primary_leader: String,
    pub secondary_leaders: Vec<String>,
    pub breadth_score: f64,
    pub concentration_score: f64,
    pub rotation_score: f64,
    pub confidence_index: f64,
    pub market_state: String,
    pub supply_phase: String,
    pub risk_state: String,
    pub day_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ObservationTimeline {
    pub history_coverage: HistoryCoverage,
    pub entries: Vec<ObservationTimelineEntry>,
    pub summary: String,
}

impl ObservationTimeline {
    pub fn has_structural_change(&self) -> bool {
        self.entries.windows(2).any(|pair| {
            let previous = &pair[0];
            let current = &pair[1];
            previous.primary_leader != current.primary_leader
                || previous.secondary_leaders != current.secondary_leaders
                || (previous.breadth_score - current.breadth_score).abs() > f64::EPSILON
                || (previous.concentration_score - current.concentration_score).abs() > f64::EPSILON
                || (previous.rotation_score - current.rotation_score).abs() > f64::EPSILON
                || (previous.confidence_index - current.confidence_index).abs() > f64::EPSILON
                || previous.market_state != current.market_state
                || previous.supply_phase != current.supply_phase
                || previous.risk_state != current.risk_state
                || previous.day_type != current.day_type
        })
    }
}

pub fn build_observation_timeline(
    observations: &[ObservationTimelineEntry],
    expected_trading_dates: &[NaiveDate],
) -> ObservationTimeline {
    let mut entries = observations
        .iter()
        .filter(|entry| expected_trading_dates.contains(&entry.date))
        .cloned()
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.date);
    entries.dedup_by_key(|entry| entry.date);
    let coverage = if entries.len() < 3 {
        HistoryCoverage::Unavailable
    } else if entries.len() == OBSERVATION_TIMELINE_DAYS
        && expected_trading_dates
            .iter()
            .all(|date| entries.iter().any(|entry| entry.date == *date))
    {
        HistoryCoverage::Complete
    } else {
        HistoryCoverage::Partial
    };
    let timeline = ObservationTimeline {
        history_coverage: coverage,
        entries,
        summary: String::new(),
    };
    let summary = match timeline.history_coverage {
        HistoryCoverage::Unavailable => String::new(),
        HistoryCoverage::Partial if timeline.entries.len() < 5 => String::new(),
        HistoryCoverage::Partial if timeline.has_structural_change() => {
            SUMMARY_LIMITED_COVERAGE_STRUCTURAL_CHANGE.to_string()
        }
        HistoryCoverage::Partial => SUMMARY_LIMITED_COVERAGE_NO_STRUCTURAL_CHANGE.to_string(),
        HistoryCoverage::Complete if timeline.has_structural_change() => {
            SUMMARY_STRUCTURAL_CHANGE.to_string()
        }
        HistoryCoverage::Complete => SUMMARY_NO_STRUCTURAL_CHANGE.to_string(),
    };
    ObservationTimeline {
        summary,
        ..timeline
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, NaiveDate};

    fn entry(date: (i32, u32, u32), leader: &str) -> ObservationTimelineEntry {
        ObservationTimelineEntry {
            date: NaiveDate::from_ymd_opt(date.0, date.1, date.2).unwrap(),
            primary_leader: leader.to_string(),
            secondary_leaders: vec!["MSFT".to_string()],
            breadth_score: 40.0,
            concentration_score: 80.0,
            rotation_score: 20.0,
            confidence_index: 50.0,
            market_state: "RANGE".to_string(),
            supply_phase: "WATCH".to_string(),
            risk_state: "NORMAL".to_string(),
            day_type: "NORMAL".to_string(),
        }
    }

    #[test]
    fn seven_trading_day_window_marks_missing_snapshot_as_partial() {
        let expected = [
            (2026, 7, 1),
            (2026, 7, 2),
            (2026, 7, 3),
            (2026, 7, 6),
            (2026, 7, 7),
            (2026, 7, 8),
            (2026, 7, 9),
        ]
        .into_iter()
        .map(|date| NaiveDate::from_ymd_opt(date.0, date.1, date.2).unwrap())
        .collect::<Vec<_>>();
        let observations = expected
            .iter()
            .filter(|date| **date != NaiveDate::from_ymd_opt(2026, 7, 6).unwrap())
            .map(|date| entry((date.year(), date.month(), date.day()), "SPY"))
            .collect::<Vec<_>>();

        let timeline = build_observation_timeline(&observations, &expected);

        assert_eq!(timeline.history_coverage, HistoryCoverage::Partial);
        assert_eq!(timeline.entries.len(), 6);
    }

    #[test]
    fn empty_window_is_unavailable_and_complete_window_is_complete() {
        let expected = (1..=7)
            .map(|day| NaiveDate::from_ymd_opt(2026, 7, day).unwrap())
            .collect::<Vec<_>>();
        let observations = expected
            .iter()
            .map(|date| entry((date.year(), date.month(), date.day()), "SPY"))
            .collect::<Vec<_>>();

        assert_eq!(
            build_observation_timeline(&[], &expected).history_coverage,
            HistoryCoverage::Unavailable
        );
        assert_eq!(
            build_observation_timeline(&observations, &expected).history_coverage,
            HistoryCoverage::Complete
        );
    }

    #[test]
    fn one_or_two_observations_are_unavailable_without_trend_summary() {
        let expected = (1..=7)
            .map(|day| NaiveDate::from_ymd_opt(2026, 7, day).unwrap())
            .collect::<Vec<_>>();
        let observations = expected[..2]
            .iter()
            .map(|date| entry((date.year(), date.month(), date.day()), "SPY"))
            .collect::<Vec<_>>();

        let timeline = build_observation_timeline(&observations, &expected);

        assert_eq!(timeline.history_coverage, HistoryCoverage::Unavailable);
        assert_eq!(timeline.entries.len(), 2);
        assert!(timeline.summary.is_empty());
    }

    #[test]
    fn summary_detects_non_leader_structural_dimension_changes() {
        let expected = (1..=5)
            .map(|day| NaiveDate::from_ymd_opt(2026, 7, day).unwrap())
            .collect::<Vec<_>>();
        let mut observations = expected
            .iter()
            .map(|date| entry((date.year(), date.month(), date.day()), "SPY"))
            .collect::<Vec<_>>();
        observations[1].confidence_index = 65.0;
        observations[1].supply_phase = "DISTRIBUTION".to_string();
        observations[1].risk_state = "DEFENSIVE".to_string();
        observations[1].day_type = "STRESS".to_string();

        let timeline = build_observation_timeline(&observations, &expected);

        assert_eq!(timeline.summary, SUMMARY_LIMITED_COVERAGE_STRUCTURAL_CHANGE);
    }

    #[test]
    fn partial_coverage_uses_point_count_to_gate_trend_conclusions() {
        let expected = (1..=7)
            .map(|day| NaiveDate::from_ymd_opt(2026, 7, day).unwrap())
            .collect::<Vec<_>>();

        for count in [3, 4] {
            let observations = expected[..count]
                .iter()
                .map(|date| entry((date.year(), date.month(), date.day()), "SPY"))
                .collect::<Vec<_>>();
            let timeline = build_observation_timeline(&observations, &expected);

            assert_eq!(timeline.history_coverage, HistoryCoverage::Partial);
            assert!(timeline.summary.is_empty(), "count={count}");
        }

        for count in [5, 6] {
            let observations = expected[..count]
                .iter()
                .map(|date| entry((date.year(), date.month(), date.day()), "SPY"))
                .collect::<Vec<_>>();
            let timeline = build_observation_timeline(&observations, &expected);

            assert_eq!(timeline.history_coverage, HistoryCoverage::Partial);
            assert_eq!(timeline.summary, "LIMITED_COVERAGE_NO_STRUCTURAL_CHANGE");
        }
    }
}
