use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

const RECENT_SWITCH_WINDOW: usize = 5;
const SCORE_SCALE: f64 = 16.0;
pub const LEADERSHIP_LOOKBACK_DAYS: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LeaderState {
    #[default]
    New,
    Early,
    Established,
    Decaying,
    Rotating,
}

impl LeaderState {
    pub fn as_str(self) -> &'static str {
        match self {
            LeaderState::New => "NEW",
            LeaderState::Early => "EARLY",
            LeaderState::Established => "ESTABLISHED",
            LeaderState::Decaying => "DECAYING",
            LeaderState::Rotating => "ROTATING",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderObservation {
    pub date: NaiveDate,
    pub leader: String,
    pub confidence: Option<f64>,
    pub breadth: Option<f64>,
    pub relative_strength: Option<f64>,
    pub rotation_stability: Option<f64>,
    pub sector_or_index_rotation: Option<String>,
    pub supply_state: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LeaderPersistenceResult {
    pub current_leader: String,
    pub previous_leader: Option<String>,
    pub persistence_days: usize,
    pub observed_leadership_days: usize,
    pub history_coverage_complete: bool,
    pub leadership_score: f64,
    pub previous_score: f64,
    pub current_breadth: Option<f64>,
    pub previous_breadth: Option<f64>,
    pub current_relative_strength: Option<f64>,
    pub previous_relative_strength: Option<f64>,
    pub current_rotation_stability: Option<f64>,
    pub previous_rotation_stability: Option<f64>,
    pub current_confidence: Option<f64>,
    pub previous_confidence: Option<f64>,
    pub leader_state: LeaderState,
    pub switch_history: Vec<String>,
    pub confidence_floor_met: bool,
    pub same_leader_as_previous: bool,
}

pub fn build_leader_persistence(
    observations: &[LeaderObservation],
) -> Option<LeaderPersistenceResult> {
    let mut by_date = std::collections::BTreeMap::new();
    for observation in observations
        .iter()
        .filter(|observation| !observation.leader.trim().is_empty())
    {
        by_date.insert(observation.date, observation);
    }
    let observations = by_date.into_values().collect::<Vec<_>>();
    let current = observations.last().copied()?;
    let previous = observations.iter().rev().nth(1).copied();

    let current_index = observations.len() - 1;
    let current_persistence_days = qualified_streak(observations.as_slice(), current_index);
    let observed_leadership_days = observations
        .iter()
        .filter(|observation| observation.leader == current.leader)
        .count();
    let history_coverage_complete = observations.len() >= LEADERSHIP_LOOKBACK_DAYS
        && observations.first().is_some_and(|first| {
            (current.date - first.date).num_days() >= (LEADERSHIP_LOOKBACK_DAYS - 1) as i64
        });
    let previous_persistence_days = current_index
        .checked_sub(1)
        .map(|index| qualified_streak(observations.as_slice(), index))
        .unwrap_or(0);

    let current_score = leadership_score(current_persistence_days, current);
    let previous_score = previous
        .map(|observation| leadership_score(previous_persistence_days, observation))
        .unwrap_or(current_score);

    let previous_leader = previous.map(|observation| observation.leader.clone());
    let same_leader_as_previous = previous
        .map(|observation| observation.leader == current.leader)
        .unwrap_or(false);
    let confidence_floor_met = current.confidence.is_some();
    let recent_switches = recent_switch_count(observations.as_slice());
    let switch_history = build_switch_history(observations.as_slice());

    let leader_state = determine_state(
        current_persistence_days,
        same_leader_as_previous,
        recent_switches,
        current_score,
        previous_score,
        current,
        previous,
    );

    Some(LeaderPersistenceResult {
        current_leader: current.leader.clone(),
        previous_leader,
        persistence_days: current_persistence_days,
        observed_leadership_days,
        history_coverage_complete,
        leadership_score: current_score,
        previous_score,
        current_breadth: current.breadth,
        previous_breadth: previous.and_then(|observation| observation.breadth),
        current_relative_strength: current.relative_strength,
        previous_relative_strength: previous.and_then(|observation| observation.relative_strength),
        current_rotation_stability: current.rotation_stability,
        previous_rotation_stability: previous
            .and_then(|observation| observation.rotation_stability),
        current_confidence: current.confidence,
        previous_confidence: previous.and_then(|observation| observation.confidence),
        leader_state,
        switch_history,
        confidence_floor_met,
        same_leader_as_previous,
    })
}

fn qualified_streak(observations: &[&LeaderObservation], current_index: usize) -> usize {
    let current = observations[current_index];
    let mut streak = 0usize;
    for observation in observations[..=current_index].iter().rev() {
        if observation.leader != current.leader {
            break;
        }
        streak += 1;
    }
    streak
}

fn recent_switch_count(observations: &[&LeaderObservation]) -> usize {
    let window = observations
        .iter()
        .rev()
        .take(RECENT_SWITCH_WINDOW)
        .copied()
        .collect::<Vec<_>>();
    if window.len() < 2 {
        return 0;
    }

    window
        .windows(2)
        .filter(|pair| pair[0].leader != pair[1].leader)
        .count()
}

fn build_switch_history(observations: &[&LeaderObservation]) -> Vec<String> {
    observations
        .windows(2)
        .filter_map(|pair| {
            let previous = pair[0];
            let current = pair[1];
            if previous.leader == current.leader {
                return None;
            }
            Some(format!(
                "{}: {} -> {}",
                current.date, previous.leader, current.leader
            ))
        })
        .collect()
}

fn determine_state(
    current_persistence_days: usize,
    same_leader_as_previous: bool,
    recent_switches: usize,
    current_score: f64,
    previous_score: f64,
    current: &LeaderObservation,
    previous: Option<&LeaderObservation>,
) -> LeaderState {
    if recent_switches >= 2 {
        return LeaderState::Rotating;
    }

    if !same_leader_as_previous {
        return LeaderState::New;
    }

    if current_persistence_days == 0 {
        return LeaderState::Decaying;
    }

    let previous = match previous {
        Some(previous) => previous,
        None => return LeaderState::New,
    };

    if current_score + 1.0 < previous_score
        || metric_down(current.breadth, previous.breadth)
        || metric_down(current.relative_strength, previous.relative_strength)
        || metric_down(current.rotation_stability, previous.rotation_stability)
        || current
            .confidence
            .zip(previous.confidence)
            .is_some_and(|(current, previous)| current + 1.0 < previous)
    {
        return LeaderState::Decaying;
    }

    match current_persistence_days {
        0 | 1 => LeaderState::New,
        2 | 3 => LeaderState::Early,
        _ => LeaderState::Established,
    }
}

fn metric_down(current: Option<f64>, previous: Option<f64>) -> bool {
    match (current, previous) {
        (Some(current), Some(previous)) => current + 1.0 < previous,
        _ => false,
    }
}

fn leadership_score(days: usize, observation: &LeaderObservation) -> f64 {
    if days == 0 {
        return 0.0;
    }

    let breadth_factor = breadth_factor(observation.breadth);
    let relative_strength_factor = bounded_factor(observation.relative_strength, 0.75, 1.10, 0.87);
    let stability_factor = stability_factor(observation.rotation_stability);
    let confidence_factor = confidence_factor(observation.confidence);
    let raw = days as f64
        * breadth_factor
        * relative_strength_factor
        * stability_factor
        * confidence_factor
        * SCORE_SCALE;

    raw.clamp(0.0, 100.0)
}

fn breadth_factor(value: Option<f64>) -> f64 {
    let normalized = normalize(value, 0.65, 1.05, 0.80);
    if value.unwrap_or(0.0) < 30.0 {
        normalized * 0.8
    } else {
        normalized
    }
}

fn stability_factor(value: Option<f64>) -> f64 {
    let normalized = normalize(value, 0.70, 1.08, 0.88);
    if value.unwrap_or(0.0) < 40.0 {
        normalized * 0.85
    } else {
        normalized
    }
}

fn confidence_factor(value: Option<f64>) -> f64 {
    normalize(value, 0.70, 1.10, 0.85)
}

fn bounded_factor(value: Option<f64>, min: f64, max: f64, default: f64) -> f64 {
    normalize(value, min, max, default)
}

fn normalize(value: Option<f64>, min: f64, max: f64, default: f64) -> f64 {
    let raw = value.unwrap_or(default).clamp(0.0, 100.0);
    let factor = min + (raw / 100.0) * (max - min);
    factor.clamp(min, max)
}

#[cfg(test)]
mod tests {
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
}
