use super::*;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LeaderState {
    #[default]
    New,
    Early,
    Established,
    Dominant,
    Fading,
    Ended,
    Unavailable,
    Decaying,
    Rotating,
    Absent,
}

#[cfg(test)]
pub(crate) fn boundary_marker() -> bool {
    true
}

impl LeaderState {
    pub fn as_str(self) -> &'static str {
        match self {
            LeaderState::New => "NEW",
            LeaderState::Early => "EARLY",
            LeaderState::Established => "ESTABLISHED",
            LeaderState::Dominant => "DOMINANT",
            LeaderState::Fading => "FADING",
            LeaderState::Ended => "ENDED",
            LeaderState::Unavailable => "UNAVAILABLE",
            LeaderState::Decaying => "DECAYING",
            LeaderState::Rotating => "ROTATING",
            LeaderState::Absent => "ABSENT",
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
    pub leader_absence_duration: usize,
    pub observed_leadership_days: usize,
    pub history_coverage_complete: bool,
    pub history_coverage: &'static str,
    pub first_observed_at: Option<NaiveDate>,
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

pub(super) fn leadership_score(days: usize, observation: &LeaderObservation) -> f64 {
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

pub(super) fn breadth_factor(value: Option<f64>) -> f64 {
    let normalized = normalize(value, 0.65, 1.05, 0.80);
    if value.unwrap_or(0.0) < 30.0 {
        normalized * 0.8
    } else {
        normalized
    }
}

pub(super) fn stability_factor(value: Option<f64>) -> f64 {
    let normalized = normalize(value, 0.70, 1.08, 0.88);
    if value.unwrap_or(0.0) < 40.0 {
        normalized * 0.85
    } else {
        normalized
    }
}

pub(super) fn confidence_factor(value: Option<f64>) -> f64 {
    normalize(value, 0.70, 1.10, 0.85)
}

pub(super) fn bounded_factor(value: Option<f64>, min: f64, max: f64, default: f64) -> f64 {
    normalize(value, min, max, default)
}

pub(super) fn normalize(value: Option<f64>, min: f64, max: f64, default: f64) -> f64 {
    let raw = value.unwrap_or(default).clamp(0.0, 100.0);
    let factor = min + (raw / 100.0) * (max - min);
    factor.clamp(min, max)
}
