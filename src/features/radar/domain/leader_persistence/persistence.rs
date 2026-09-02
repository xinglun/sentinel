use super::absence::{is_no_leader, normalized_leader, qualified_no_leader_streak};
use super::snapshot::leadership_score;
use super::tactical_leadership_structure;
use super::transition::{build_switch_history, determine_state, recent_switch_count};
use super::*;
use std::collections::BTreeMap;

pub fn build_leader_persistence(
    observations: &[LeaderObservation],
) -> Option<LeaderPersistenceResult> {
    let mut by_date = BTreeMap::new();
    for observation in observations {
        by_date.insert(observation.date, observation);
    }
    let observations = by_date.into_values().collect::<Vec<_>>();
    if observations.is_empty() {
        return Some(LeaderPersistenceResult {
            current_leader: String::new(),
            previous_leader: None,
            previous_snapshot_leader: None,
            last_confirmed_leader: None,
            leader_absence_since: None,
            persistence_days: 0,
            leader_absence_duration: 0,
            tactical_leadership_structure: "LEADERLESS / FRAGMENTED".to_string(),
            observed_leadership_days: 0,
            history_coverage_complete: false,
            history_coverage: "UNAVAILABLE",
            calculation_mode: "UNAVAILABLE",
            first_observed_at: None,
            leadership_score: 0.0,
            previous_score: 0.0,
            current_breadth: None,
            previous_breadth: None,
            current_relative_strength: None,
            previous_relative_strength: None,
            current_rotation_stability: None,
            previous_rotation_stability: None,
            current_confidence: None,
            previous_confidence: None,
            leader_state: LeaderState::Unavailable,
            switch_history: Vec::new(),
            confidence_floor_met: false,
            same_leader_as_previous: false,
        });
    }
    let current = observations.last().copied()?;
    let previous = observations.iter().rev().nth(1).copied();

    let current_index = observations.len() - 1;
    let current_persistence_days = if is_no_leader(&current.leader) {
        0
    } else {
        qualified_streak(observations.as_slice(), current_index)
    };
    let leader_absence_duration = if is_no_leader(&current.leader) {
        qualified_no_leader_streak(observations.as_slice(), current_index)
    } else {
        0
    };
    let leader_absence_since = if is_no_leader(&current.leader) {
        observations
            .iter()
            .rev()
            .take_while(|observation| is_no_leader(&observation.leader))
            .last()
            .map(|observation| observation.date)
    } else {
        None
    };
    let observed_leadership_days = observations
        .iter()
        .filter(|observation| {
            !is_no_leader(&current.leader) && observation.leader == current.leader
        })
        .count();
    let first_observed_at = (observations.len() > 1)
        .then(|| {
            (!is_no_leader(&current.leader))
                .then(|| {
                    observations
                        .iter()
                        .find(|observation| observation.leader == current.leader)
                        .map(|observation| observation.date)
                })
                .flatten()
        })
        .flatten();
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

    let previous_snapshot_leader =
        previous.map(|observation| normalized_leader(&observation.leader));
    let previous_leader = previous_snapshot_leader.clone();
    let last_confirmed_leader = observations
        .iter()
        .rev()
        .find(|observation| !is_no_leader(&observation.leader))
        .map(|observation| normalized_leader(&observation.leader));
    let same_leader_as_previous = previous
        .map(|observation| observation.leader == current.leader)
        .unwrap_or(false);
    let confidence_floor_met = current.confidence.is_some();
    let recent_switches = recent_switch_count(observations.as_slice());
    let switch_history = build_switch_history(observations.as_slice());

    let mut leader_state = determine_state(
        observations.as_slice(),
        current_persistence_days,
        same_leader_as_previous,
        recent_switches,
        current_score,
        previous_score,
        current,
        previous,
    );
    let history_coverage = if observations.len() == 1 {
        leader_state = LeaderState::Unavailable;
        "UNAVAILABLE"
    } else if history_coverage_complete {
        "COMPLETE"
    } else {
        "PARTIAL"
    };
    let calculation_mode = match history_coverage {
        "COMPLETE" => "PERSISTED_FACT",
        "PARTIAL" => "RECOMPUTED_FROM_PARTIAL_HISTORY",
        _ => "UNAVAILABLE",
    };

    Some(LeaderPersistenceResult {
        current_leader: normalized_leader(&current.leader),
        previous_leader,
        previous_snapshot_leader,
        last_confirmed_leader,
        leader_absence_since,
        persistence_days: current_persistence_days,
        leader_absence_duration,
        tactical_leadership_structure: tactical_leadership_structure(
            &current.leader,
            leader_absence_duration,
        )
        .to_string(),
        observed_leadership_days,
        history_coverage_complete,
        history_coverage,
        calculation_mode,
        first_observed_at,
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

#[cfg(test)]
pub(crate) fn boundary_marker() -> bool {
    true
}

pub(super) fn qualified_streak(observations: &[&LeaderObservation], current_index: usize) -> usize {
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
