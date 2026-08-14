use super::absence::{is_no_leader, normalized_leader};
use super::snapshot::leadership_score;
use super::*;

pub(super) fn recent_switch_count(observations: &[&LeaderObservation]) -> usize {
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

#[cfg(test)]
pub(crate) fn boundary_marker() -> bool {
    true
}

pub(super) fn build_switch_history(observations: &[&LeaderObservation]) -> Vec<String> {
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
                current.date,
                normalized_leader(&previous.leader),
                normalized_leader(&current.leader)
            ))
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn determine_state(
    observations: &[&LeaderObservation],
    current_persistence_days: usize,
    same_leader_as_previous: bool,
    recent_switches: usize,
    current_score: f64,
    previous_score: f64,
    current: &LeaderObservation,
    previous: Option<&LeaderObservation>,
) -> LeaderState {
    if is_no_leader(&current.leader) {
        return LeaderState::Absent;
    }

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

    if meaningful_decline_streak(observations) >= 3 {
        return LeaderState::Fading;
    }

    if current_persistence_days >= 8
        && current.breadth.unwrap_or_default() >= 60.0
        && current.relative_strength.unwrap_or_default() >= 60.0
    {
        return LeaderState::Dominant;
    }

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

pub(super) fn meaningful_decline_streak(observations: &[&LeaderObservation]) -> usize {
    let mut streak = 0usize;
    for pair in observations.windows(2).rev() {
        let previous = pair[0];
        let current = pair[1];
        let score_declined = leadership_score(1, current) + 2.0 <= leadership_score(1, previous);
        let relative_strength_declined = current
            .relative_strength
            .zip(previous.relative_strength)
            .is_some_and(|(current, previous)| current + 2.0 <= previous);
        if score_declined || relative_strength_declined {
            streak += 1;
        } else {
            break;
        }
    }
    streak
}

pub(super) fn metric_down(current: Option<f64>, previous: Option<f64>) -> bool {
    match (current, previous) {
        (Some(current), Some(previous)) => current + 1.0 < previous,
        _ => false,
    }
}
