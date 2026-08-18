const RECENT_SWITCH_WINDOW: usize = 5;
const SCORE_SCALE: f64 = 16.0;
pub const LEADERSHIP_LOOKBACK_DAYS: usize = 20;
pub const LEADERLESS_STRUCTURE_THRESHOLD_DAYS: usize = 5;
#[cfg(test)]
use chrono::NaiveDate;

mod absence;
mod persistence;
mod snapshot;
mod transition;

#[cfg(test)]
pub(crate) use absence::boundary_marker as absence_boundary_marker;
#[cfg(test)]
pub(crate) use persistence::boundary_marker as persistence_boundary_marker;
pub use persistence::build_leader_persistence;
#[cfg(test)]
pub(crate) use snapshot::boundary_marker as snapshot_boundary_marker;
pub use snapshot::{LeaderObservation, LeaderPersistenceResult, LeaderState};

pub fn tactical_leadership_structure(
    current_leader: &str,
    absence_duration: usize,
) -> &'static str {
    if current_leader.trim().is_empty()
        || current_leader.trim().eq_ignore_ascii_case("none")
        || absence_duration >= LEADERLESS_STRUCTURE_THRESHOLD_DAYS
    {
        "LEADERLESS / FRAGMENTED"
    } else {
        "CORE_ASSET_LED"
    }
}
#[cfg(test)]
pub(crate) use transition::boundary_marker as transition_boundary_marker;

#[cfg(test)]
mod tests;
