use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

pub const OBSERVATION_TIMELINE_DAYS: usize = 7;
pub const SUMMARY_NO_STRUCTURAL_CHANGE: &str = "NO_STRUCTURAL_CHANGE";
pub const SUMMARY_STRUCTURAL_CHANGE: &str = "STRUCTURAL_CHANGE";
pub const SUMMARY_LIMITED_COVERAGE_NO_STRUCTURAL_CHANGE: &str =
    "LIMITED_COVERAGE_NO_STRUCTURAL_CHANGE";
pub const SUMMARY_LIMITED_COVERAGE_STRUCTURAL_CHANGE: &str = "LIMITED_COVERAGE_STRUCTURAL_CHANGE";

const VALID_SUPPLY_PHASES: [&str; 5] = [
    "IDLE",
    "ACCUMULATING",
    "ABSORBING",
    "STRESSED",
    "OVERWHELMED",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HistoryCoverage {
    Complete,
    Partial,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryProgression {
    Appended,
    SameDayRerun,
    HistoryRegression,
    InvalidGap,
}

pub fn validate_history_progression(
    previous_count: usize,
    current_count: usize,
    same_market_date: bool,
) -> HistoryProgression {
    if current_count < previous_count {
        HistoryProgression::HistoryRegression
    } else if same_market_date && current_count == previous_count {
        HistoryProgression::SameDayRerun
    } else if !same_market_date && current_count == previous_count + 1 {
        HistoryProgression::Appended
    } else {
        HistoryProgression::InvalidGap
    }
}

/// 日次観測の履歴件数と基線信頼性を公開前に検証する。
pub fn daily_observation_consistency_gate(
    previous_count: usize,
    current_count: usize,
    is_new_market_date: bool,
    baseline_available: bool,
    leadership_confidence: &str,
) -> Result<(), &'static str> {
    if current_count < previous_count {
        return Err("HISTORY_REGRESSION");
    }
    if is_new_market_date && current_count != previous_count + 1 {
        return Err("INVALID_HISTORY_APPEND");
    }
    if !is_new_market_date && current_count != previous_count {
        return Err("INVALID_SAME_DAY_RERUN");
    }
    if !baseline_available && leadership_confidence == "HIGH" {
        return Err("CONFIDENCE_CONTRADICTION");
    }
    Ok(())
}

/// 永続化前に、基線日付と Narrative の事実参照を検証する。
pub fn daily_fact_consistency_gate(
    current_market_date: NaiveDate,
    previous_market_date: Option<NaiveDate>,
    baseline_available: bool,
    narrative_values: &[String],
    allowed_leaders: &[String],
) -> Result<(), &'static str> {
    if baseline_available
        && previous_market_date.is_none_or(|previous| previous >= current_market_date)
    {
        return Err("BASELINE_MISMATCH");
    }
    if narrative_values.iter().any(|line| {
        line.split(|character: char| !character.is_ascii_alphanumeric())
            .filter(|token| !token.is_empty())
            .filter(|token| {
                token.len() <= 5
                    && token
                        .chars()
                        .all(|character| character.is_ascii_uppercase())
            })
            .any(|token| !allowed_leaders.iter().any(|leader| leader == token))
    }) {
        return Err("NARRATIVE_FACT_CONFLICT");
    }
    Ok(())
}

/// 日次の事実、供給フェーズ、Read Model の値が同じ観測結果を参照することを検証する。
pub fn cross_layer_consistency_gate(
    current_market_date: NaiveDate,
    previous_market_date: Option<NaiveDate>,
    baseline_available: bool,
    current_supply_phase: &str,
    change_log_supply_phase: &str,
    narrative_values: &[String],
    allowed_leaders: &[String],
) -> Result<(), &'static str> {
    if baseline_available
        && previous_market_date.is_none_or(|previous| previous >= current_market_date)
    {
        return Err("BASELINE_MISMATCH");
    }
    if current_supply_phase != change_log_supply_phase {
        return Err("BASELINE_MISMATCH");
    }
    daily_fact_consistency_gate(
        current_market_date,
        previous_market_date,
        baseline_available,
        narrative_values,
        allowed_leaders,
    )
    .map_err(|reason| {
        if reason == "NARRATIVE_FACT_CONFLICT" {
            "READ_MODEL_CONFLICT"
        } else {
            reason
        }
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ObservationTimelineEntry {
    pub date: NaiveDate,
    pub primary_leader: String,
    pub secondary_leaders: Vec<String>,
    pub breadth_score: f64,
    #[serde(default)]
    pub breadth_raw_percent: f64,
    #[serde(default)]
    pub breadth_up_count: usize,
    #[serde(default)]
    pub breadth_flat_count: usize,
    #[serde(default)]
    pub breadth_down_count: usize,
    #[serde(default)]
    pub breadth_total_count: usize,
    #[serde(default)]
    pub breadth_universe_integrity: f64,
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

fn normalize_supply_phase(value: &str) -> String {
    if VALID_SUPPLY_PHASES.contains(&value) {
        value.to_string()
    } else {
        "UNAVAILABLE".to_string()
    }
}

fn supply_phase_changed(previous: &str, current: &str) -> bool {
    previous != "UNAVAILABLE" && current != "UNAVAILABLE" && previous != current
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
                || supply_phase_changed(&previous.supply_phase, &current.supply_phase)
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
        .map(|mut entry| {
            entry.supply_phase = normalize_supply_phase(&entry.supply_phase);
            entry
        })
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
            supply_phase: "IDLE".to_string(),
            risk_state: "NORMAL".to_string(),
            day_type: "NORMAL".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn cross_layer_gate_rejects_supply_and_read_model_conflicts() {
        let current = NaiveDate::from_ymd_opt(2026, 7, 27).unwrap();
        let previous = NaiveDate::from_ymd_opt(2026, 7, 24).unwrap();
        assert_eq!(
            cross_layer_consistency_gate(
                current,
                Some(previous),
                true,
                "STRESSED",
                "IDLE",
                &[],
                &[],
            ),
            Err("BASELINE_MISMATCH")
        );
        assert_eq!(
            cross_layer_consistency_gate(
                current,
                Some(previous),
                true,
                "STRESSED",
                "STRESSED",
                &["GOOG remains the primary leader.".to_string()],
                &["TSLA".to_string()],
            ),
            Err("READ_MODEL_CONFLICT")
        );
    }

    #[test]
    fn breadth_observation_fields_round_trip_separately_from_classification_score() {
        let mut value = entry((2026, 8, 13), "SPY");
        value.breadth_score = 35.0;
        value.breadth_raw_percent = 50.0;
        value.breadth_up_count = 5;
        value.breadth_flat_count = 1;
        value.breadth_down_count = 4;
        value.breadth_total_count = 10;
        value.breadth_universe_integrity = 1.0;
        let encoded = serde_json::to_value(&value).unwrap();
        let restored: ObservationTimelineEntry = serde_json::from_value(encoded).unwrap();
        assert_eq!(restored.breadth_score, 35.0);
        assert_eq!(restored.breadth_raw_percent, 50.0);
        assert_eq!(restored.breadth_total_count, 10);
        assert_eq!(restored.breadth_universe_integrity, 1.0);
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
        observations[1].supply_phase = "STRESSED".to_string();
        observations[1].risk_state = "DEFENSIVE".to_string();
        observations[1].day_type = "STRESS".to_string();

        let timeline = build_observation_timeline(&observations, &expected);

        assert_eq!(timeline.summary, SUMMARY_LIMITED_COVERAGE_STRUCTURAL_CHANGE);
    }

    #[test]
    fn legacy_supply_values_are_unavailable_and_do_not_create_a_phase_change() {
        let expected = (1..=5)
            .map(|day| NaiveDate::from_ymd_opt(2026, 7, day).unwrap())
            .collect::<Vec<_>>();
        let mut observations = expected
            .iter()
            .map(|date| entry((date.year(), date.month(), date.day()), "SPY"))
            .collect::<Vec<_>>();
        observations[0].supply_phase = "HIGH".to_string();

        let timeline = build_observation_timeline(&observations, &expected);

        assert_eq!(timeline.entries[0].supply_phase, "UNAVAILABLE");
        assert_eq!(timeline.entries[1].supply_phase, "IDLE");
        assert!(!timeline.has_structural_change());
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

    #[test]
    fn history_progression_rejects_regression_and_accepts_append_or_rerun() {
        assert_eq!(
            validate_history_progression(2, 1, false),
            HistoryProgression::HistoryRegression
        );
        assert_eq!(
            validate_history_progression(2, 3, false),
            HistoryProgression::Appended
        );
        assert_eq!(
            validate_history_progression(2, 2, true),
            HistoryProgression::SameDayRerun
        );
    }

    #[test]
    fn daily_consistency_gate_rejects_regression_and_invalid_confidence() {
        assert_eq!(
            daily_observation_consistency_gate(3, 2, true, true, "LOW"),
            Err("HISTORY_REGRESSION")
        );
        assert_eq!(
            daily_observation_consistency_gate(3, 5, true, true, "LOW"),
            Err("INVALID_HISTORY_APPEND")
        );
        assert_eq!(
            daily_observation_consistency_gate(3, 3, false, false, "HIGH"),
            Err("CONFIDENCE_CONTRADICTION")
        );
        assert_eq!(
            daily_observation_consistency_gate(3, 3, false, false, "LOW"),
            Ok(())
        );
        assert_eq!(
            daily_observation_consistency_gate(3, 4, true, true, "HIGH"),
            Ok(())
        );
        assert_eq!(
            daily_observation_consistency_gate(3, 3, false, true, "HIGH"),
            Ok(())
        );
    }

    #[test]
    fn daily_fact_gate_rejects_baseline_and_narrative_conflicts() {
        assert_eq!(
            daily_fact_consistency_gate(
                NaiveDate::from_ymd_opt(2026, 7, 27).unwrap(),
                Some(NaiveDate::from_ymd_opt(2026, 7, 27).unwrap()),
                true,
                &[],
                &[]
            ),
            Err("BASELINE_MISMATCH")
        );
        assert_eq!(
            daily_fact_consistency_gate(
                NaiveDate::from_ymd_opt(2026, 7, 27).unwrap(),
                Some(NaiveDate::from_ymd_opt(2026, 7, 24).unwrap()),
                true,
                &["市场由 SPY 与 GOOG 主导".to_string()],
                &["SPY".to_string()]
            ),
            Err("NARRATIVE_FACT_CONFLICT")
        );
    }
}
