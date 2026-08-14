use super::*;

pub(super) fn qualified_no_leader_streak(
    observations: &[&LeaderObservation],
    current_index: usize,
) -> usize {
    observations[..=current_index]
        .iter()
        .rev()
        .take_while(|observation| is_no_leader(&observation.leader))
        .count()
}

#[cfg(test)]
pub(crate) fn boundary_marker() -> bool {
    true
}

pub(super) fn normalized_leader(value: &str) -> String {
    if is_no_leader(value) {
        "none".to_string()
    } else {
        value.trim().to_string()
    }
}

pub(super) fn is_no_leader(value: &str) -> bool {
    value.trim().is_empty() || value.trim().eq_ignore_ascii_case("none")
}
