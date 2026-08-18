use super::*;

fn observation(
    date: (i32, u32, u32),
    leader: &str,
    confidence: f64,
    breadth: f64,
    relative_strength: f64,
    rotation_stability: f64,
) -> LeaderObservation {
    LeaderObservation {
        date: NaiveDate::from_ymd_opt(date.0, date.1, date.2).unwrap(),
        leader: leader.to_string(),
        confidence: Some(confidence),
        breadth: Some(breadth),
        relative_strength: Some(relative_strength),
        rotation_stability: Some(rotation_stability),
        sector_or_index_rotation: None,
        supply_state: None,
    }
}

#[test]
fn streak_grows_for_same_leader_with_confident_reads() {
    let observations = vec![
        observation((2026, 7, 1), "GOOG", 90.0, 72.0, 71.0, 84.0),
        observation((2026, 7, 2), "GOOG", 88.0, 74.0, 72.0, 85.0),
        observation((2026, 7, 3), "GOOG", 86.0, 76.0, 74.0, 86.0),
        observation((2026, 7, 4), "GOOG", 87.0, 77.0, 75.0, 87.0),
        observation((2026, 7, 5), "GOOG", 89.0, 79.0, 77.0, 88.0),
    ];

    let result = build_leader_persistence(&observations).unwrap();
    assert_eq!(result.persistence_days, 5);
    assert_eq!(result.current_leader, "GOOG");
    assert_eq!(result.leader_state, LeaderState::Established);
    assert!(result.leadership_score > 60.0);
}

#[test]
fn streak_continues_beyond_twenty_five_persisted_days() {
    let start = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
    let observations = (0..26)
        .map(|offset| LeaderObservation {
            date: start + chrono::Duration::days(offset),
            leader: "GOOG".to_string(),
            confidence: Some(40.0),
            breadth: Some(65.0),
            relative_strength: Some(4.0),
            rotation_stability: Some(70.0),
            sector_or_index_rotation: None,
            supply_state: None,
        })
        .collect::<Vec<_>>();

    let result = build_leader_persistence(&observations).unwrap();
    assert_eq!(result.persistence_days, 26);
    assert!(result.history_coverage_complete);
    assert_eq!(result.observed_leadership_days, 26);
}

#[test]
fn feature_activation_history_is_partial_without_resetting_known_streak() {
    let start = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
    let observations = (0..5)
        .map(|offset| observation((2026, 7, 1 + offset), "SPY", 90.0, 70.0, 72.0, 85.0))
        .collect::<Vec<_>>();

    let result = build_leader_persistence(&observations).unwrap();
    assert_eq!(result.persistence_days, 5);
    assert_eq!(result.observed_leadership_days, 5);
    assert!(!result.history_coverage_complete);
    assert_eq!(result.current_leader, "SPY");
    assert_eq!(result.history_coverage, "PARTIAL");
    assert_eq!(result.first_observed_at, Some(start));
    assert_eq!(start, observations[0].date);
}

#[test]
fn complete_lookback_marks_new_leader_as_first_in_covered_history() {
    let start = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
    let observations = (0..LEADERSHIP_LOOKBACK_DAYS)
        .map(|offset| LeaderObservation {
            date: start + chrono::Duration::days(offset as i64),
            leader: if offset + 1 == LEADERSHIP_LOOKBACK_DAYS {
                "MSFT"
            } else {
                "SPY"
            }
            .to_string(),
            confidence: Some(90.0),
            breadth: Some(70.0),
            relative_strength: Some(72.0),
            rotation_stability: Some(85.0),
            sector_or_index_rotation: None,
            supply_state: None,
        })
        .collect::<Vec<_>>();
    let result = build_leader_persistence(&observations).unwrap();
    assert!(result.history_coverage_complete);
    assert_eq!(result.current_leader, "MSFT");
    assert_eq!(result.observed_leadership_days, 1);
}

#[test]
fn switch_resets_streak_on_sixth_day() {
    let observations = vec![
        observation((2026, 7, 1), "GOOG", 90.0, 72.0, 71.0, 84.0),
        observation((2026, 7, 2), "GOOG", 88.0, 74.0, 72.0, 85.0),
        observation((2026, 7, 3), "GOOG", 86.0, 76.0, 74.0, 86.0),
        observation((2026, 7, 4), "GOOG", 87.0, 77.0, 75.0, 87.0),
        observation((2026, 7, 5), "GOOG", 89.0, 79.0, 77.0, 88.0),
        observation((2026, 7, 6), "MSFT", 91.0, 81.0, 80.0, 89.0),
    ];

    let result = build_leader_persistence(&observations).unwrap();
    assert_eq!(result.persistence_days, 1);
    assert_eq!(result.current_leader, "MSFT");
    assert_eq!(result.previous_leader.as_deref(), Some("GOOG"));
    assert_eq!(result.leader_state, LeaderState::New);
    assert!(result
        .switch_history
        .iter()
        .any(|item| item.contains("GOOG -> MSFT")));
}

#[test]
fn one_day_without_a_leader_is_explicitly_absent() {
    let observations = vec![
        observation((2026, 7, 1), "GOOG", 90.0, 72.0, 71.0, 84.0),
        observation((2026, 7, 2), "", 0.0, 0.0, 0.0, 0.0),
    ];

    let result = build_leader_persistence(&observations).unwrap();

    assert_eq!(result.current_leader, "none");
    assert_eq!(result.previous_leader.as_deref(), Some("GOOG"));
    assert_eq!(result.persistence_days, 0);
    assert_eq!(result.leader_state, LeaderState::Absent);
    assert_eq!(result.leader_absence_duration, 1);
    assert!(result
        .switch_history
        .iter()
        .any(|item| item.contains("GOOG -> none")));
}

#[test]
fn six_consecutive_days_without_a_leader_report_absence_duration() {
    let observations = (0..6)
        .map(|index| observation((2026, 7, 1 + index), "", 0.0, 0.0, 0.0, 0.0))
        .collect::<Vec<_>>();

    let result = build_leader_persistence(&observations).unwrap();

    assert_eq!(result.current_leader, "none");
    assert_eq!(result.leader_state, LeaderState::Absent);
    assert_eq!(result.leader_absence_duration, 6);
    assert_eq!(result.persistence_days, 0);
}

#[test]
fn low_breadth_keeps_score_bounded() {
    let observations = vec![
        observation((2026, 7, 1), "GOOG", 92.0, 20.0, 85.0, 92.0),
        observation((2026, 7, 2), "GOOG", 93.0, 18.0, 87.0, 93.0),
    ];

    let result = build_leader_persistence(&observations).unwrap();
    assert!(result.leadership_score < 45.0);
}

#[test]
fn score_decline_triggers_decaying_without_name_change() {
    let observations = vec![
        observation((2026, 7, 1), "GOOG", 94.0, 82.0, 80.0, 92.0),
        observation((2026, 7, 2), "GOOG", 88.0, 78.0, 76.0, 86.0),
        observation((2026, 7, 3), "GOOG", 58.0, 24.0, 28.0, 30.0),
    ];

    let result = build_leader_persistence(&observations).unwrap();
    assert_eq!(result.current_leader, "GOOG");
    assert_eq!(result.leader_state, LeaderState::Decaying);
    assert!(result.leadership_score <= result.previous_score);
}

#[test]
fn low_confidence_does_not_break_persistence() {
    let observations = vec![
        observation((2026, 7, 1), "GOOG", 90.0, 72.0, 71.0, 84.0),
        observation((2026, 7, 2), "GOOG", 59.0, 70.0, 70.0, 83.0),
    ];

    let result = build_leader_persistence(&observations).unwrap();
    assert_eq!(result.persistence_days, 2);
    assert!(result.leadership_score < 60.0);
}

#[test]
fn frequent_rotation_marks_rotating() {
    let observations = vec![
        observation((2026, 7, 1), "GOOG", 88.0, 74.0, 72.0, 84.0),
        observation((2026, 7, 2), "MSFT", 89.0, 75.0, 73.0, 85.0),
        observation((2026, 7, 3), "GOOG", 87.0, 74.0, 71.0, 82.0),
        observation((2026, 7, 4), "NVDA", 90.0, 76.0, 74.0, 86.0),
        observation((2026, 7, 5), "GOOG", 91.0, 78.0, 75.0, 87.0),
    ];

    let result = build_leader_persistence(&observations).unwrap();
    assert_eq!(result.leader_state, LeaderState::Rotating);
    assert!(result.switch_history.len() >= 3);
}

#[test]
fn eight_day_leader_with_breadth_and_relative_strength_is_dominant() {
    let start = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
    let observations = (0..8)
        .map(|offset| LeaderObservation {
            date: start + chrono::Duration::days(offset),
            leader: "SPY".to_string(),
            confidence: Some(70.0),
            breadth: Some(60.0),
            relative_strength: Some(60.0),
            rotation_stability: Some(70.0),
            sector_or_index_rotation: None,
            supply_state: None,
        })
        .collect::<Vec<_>>();

    let result = build_leader_persistence(&observations).unwrap();

    assert_eq!(result.leader_state, LeaderState::Dominant);
}

#[test]
fn three_consecutive_meaningful_declines_mark_leader_as_fading() {
    let observations = vec![
        observation((2026, 7, 1), "SPY", 90.0, 80.0, 80.0, 90.0),
        observation((2026, 7, 2), "SPY", 90.0, 78.0, 78.0, 88.0),
        observation((2026, 7, 3), "SPY", 90.0, 76.0, 76.0, 86.0),
        observation((2026, 7, 4), "SPY", 90.0, 74.0, 74.0, 84.0),
    ];

    let result = build_leader_persistence(&observations).unwrap();

    assert_eq!(result.leader_state, LeaderState::Fading);
}

#[test]
fn empty_or_single_observation_is_unavailable() {
    assert_eq!(
        build_leader_persistence(&[]).unwrap().history_coverage,
        "UNAVAILABLE"
    );
    let single = observation((2026, 7, 1), "SPY", 80.0, 70.0, 70.0, 70.0);
    let result = build_leader_persistence(&[single]).unwrap();
    assert_eq!(result.history_coverage, "UNAVAILABLE");
    assert_eq!(result.leader_state, LeaderState::Unavailable);
    assert_eq!(result.first_observed_at, None);
}

#[test]
fn leaderless_history_separates_snapshot_leader_last_confirmed_leader_and_absence_start() {
    let observations = std::iter::once(observation((2026, 8, 4), "TSLA", 80.0, 70.0, 70.0, 70.0))
        .chain((5..=13).map(|day| observation((2026, 8, day), "", 0.0, 0.0, 0.0, 0.0)))
        .collect::<Vec<_>>();

    let result = build_leader_persistence(&observations).unwrap();

    assert_eq!(result.current_leader, "none");
    assert_eq!(result.previous_snapshot_leader.as_deref(), Some("none"));
    assert_eq!(result.last_confirmed_leader.as_deref(), Some("TSLA"));
    assert_eq!(result.leader_absence_since, Some(observations[1].date));
    assert_eq!(result.leader_absence_duration, 9);
    assert_eq!(
        result.tactical_leadership_structure,
        "LEADERLESS / FRAGMENTED"
    );
}
