use crate::features::research::domain::gray_rhino_candidate::{
    GrayRhinoCandidate, GrayRhinoCandidateKind, GrayRhinoCandidateScope, GrayRhinoCandidateState,
};
use chrono::NaiveDate;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GrayRhinoMonitoringDirection {
    New,
    Stable,
    Intensifying,
    Cooling,
    Resolved,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GrayRhinoMonitoringStatus {
    pub scope: GrayRhinoCandidateScope,
    pub kind: GrayRhinoCandidateKind,
    pub subject: String,
    pub current_state: GrayRhinoCandidateState,
    pub previous_state: Option<GrayRhinoCandidateState>,
    pub direction: GrayRhinoMonitoringDirection,
    pub observation_count: usize,
    pub latest_observed_at: NaiveDate,
    pub stale_days: i64,
}

pub(crate) fn evaluate_gray_rhino_monitoring_states(
    candidates: &[GrayRhinoCandidate],
    as_of_date: NaiveDate,
) -> Vec<GrayRhinoMonitoringStatus> {
    let mut groups: BTreeMap<String, Vec<&GrayRhinoCandidate>> = BTreeMap::new();
    for candidate in candidates
        .iter()
        .filter(|candidate| candidate.observed_at <= as_of_date)
    {
        groups
            .entry(candidate_key(candidate))
            .or_default()
            .push(candidate);
    }

    groups
        .into_values()
        .filter_map(|mut group| {
            group.sort_by(|a, b| {
                a.observed_at
                    .cmp(&b.observed_at)
                    .then_with(|| state_rank(a.state).cmp(&state_rank(b.state)))
            });
            let latest = group.last().copied()?;
            let previous = group
                .iter()
                .rev()
                .skip(1)
                .find(|candidate| candidate.observed_at < latest.observed_at)
                .copied();
            let observation_dates = group
                .iter()
                .map(|candidate| candidate.observed_at)
                .collect::<BTreeSet<_>>();
            let stale_days = (as_of_date - latest.observed_at).num_days();
            let previous_state = previous.map(|candidate| candidate.state);
            let (current_state, direction) = classify_state(
                latest.state,
                previous_state,
                observation_dates.len(),
                stale_days,
            );

            Some(GrayRhinoMonitoringStatus {
                scope: latest.scope,
                kind: latest.kind,
                subject: latest.subject.clone(),
                current_state,
                previous_state,
                direction,
                observation_count: observation_dates.len(),
                latest_observed_at: latest.observed_at,
                stale_days,
            })
        })
        .collect()
}

pub(crate) fn render_gray_rhino_monitoring_states(
    statuses: &[GrayRhinoMonitoringStatus],
) -> String {
    if statuses.is_empty() {
        return "Gray Rhino Monitoring Status: none.\nBoundary: reference only; no trading, Gate, trend, or market-state mutation.".to_string();
    }
    let mut out = String::from("Gray Rhino Monitoring State (semantic isolation)\n");
    for status in statuses {
        out.push_str(&format!(
            "- {} / {:?} / {:?}: {:?} ({:?}, observations: {}, latest: {}, stale_days: {})\n",
            status.subject,
            status.scope,
            status.kind,
            status.current_state,
            status.direction,
            status.observation_count,
            status.latest_observed_at,
            status.stale_days
        ));
        if let Some(previous_state) = status.previous_state {
            out.push_str(&format!("  Previous state: {:?}\n", previous_state));
        }
    }
    out.push_str("Boundary: reference only; no trading, Gate, trend, or market-state mutation.");
    out
}

fn classify_state(
    latest_state: GrayRhinoCandidateState,
    previous_state: Option<GrayRhinoCandidateState>,
    observation_count: usize,
    stale_days: i64,
) -> (GrayRhinoCandidateState, GrayRhinoMonitoringDirection) {
    if stale_days >= 30 {
        return (
            GrayRhinoCandidateState::Resolved,
            GrayRhinoMonitoringDirection::Resolved,
        );
    }
    if stale_days >= 14 {
        return (
            GrayRhinoCandidateState::Cooling,
            GrayRhinoMonitoringDirection::Cooling,
        );
    }
    let Some(previous_state) = previous_state else {
        return (latest_state, GrayRhinoMonitoringDirection::New);
    };
    if state_rank(latest_state) > state_rank(previous_state) {
        return (latest_state, GrayRhinoMonitoringDirection::Intensifying);
    }
    if latest_state == GrayRhinoCandidateState::Visible && observation_count >= 2 {
        return (
            GrayRhinoCandidateState::Expanding,
            GrayRhinoMonitoringDirection::Intensifying,
        );
    }
    if state_rank(latest_state) < state_rank(previous_state) {
        return (latest_state, GrayRhinoMonitoringDirection::Cooling);
    }
    (latest_state, GrayRhinoMonitoringDirection::Stable)
}

fn state_rank(state: GrayRhinoCandidateState) -> u8 {
    match state {
        GrayRhinoCandidateState::Background => 0,
        GrayRhinoCandidateState::Resolved => 0,
        GrayRhinoCandidateState::Cooling => 1,
        GrayRhinoCandidateState::Visible => 2,
        GrayRhinoCandidateState::Expanding => 3,
        GrayRhinoCandidateState::Critical => 4,
    }
}

fn candidate_key(candidate: &GrayRhinoCandidate) -> String {
    format!(
        "{}::{:?}::{:?}",
        candidate.subject.to_uppercase(),
        candidate.scope,
        candidate.kind
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(
        subject: &str,
        state: GrayRhinoCandidateState,
        observed_at: NaiveDate,
    ) -> GrayRhinoCandidate {
        GrayRhinoCandidate {
            scope: GrayRhinoCandidateScope::Company,
            kind: GrayRhinoCandidateKind::GovernanceConcentration,
            subject: subject.to_string(),
            state,
            evidence: vec!["governance candidate".to_string()],
            watch_triggers: vec!["proxy update".to_string()],
            source_title: "Proxy".to_string(),
            observed_at,
        }
    }

    #[test]
    fn repeated_visible_candidate_intensifies_to_expanding() {
        let as_of_date = NaiveDate::from_ymd_opt(2026, 5, 25).unwrap();
        let statuses = evaluate_gray_rhino_monitoring_states(
            &[
                candidate(
                    "TSLA",
                    GrayRhinoCandidateState::Visible,
                    NaiveDate::from_ymd_opt(2026, 5, 24).unwrap(),
                ),
                candidate("TSLA", GrayRhinoCandidateState::Visible, as_of_date),
            ],
            as_of_date,
        );

        assert_eq!(statuses.len(), 1);
        assert_eq!(
            statuses[0].current_state,
            GrayRhinoCandidateState::Expanding
        );
        assert_eq!(
            statuses[0].direction,
            GrayRhinoMonitoringDirection::Intensifying
        );
    }

    #[test]
    fn stale_candidate_cools_then_resolves() {
        let candidate = candidate(
            "TSLA",
            GrayRhinoCandidateState::Expanding,
            NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
        );

        let cooling = evaluate_gray_rhino_monitoring_states(
            std::slice::from_ref(&candidate),
            NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
        );
        assert_eq!(cooling[0].current_state, GrayRhinoCandidateState::Cooling);

        let resolved = evaluate_gray_rhino_monitoring_states(
            std::slice::from_ref(&candidate),
            NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
        );
        assert_eq!(resolved[0].current_state, GrayRhinoCandidateState::Resolved);
    }
}
