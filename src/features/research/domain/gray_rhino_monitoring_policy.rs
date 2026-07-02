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

/// 候補履歴から lifecycle state と direction を決定する domain policy。
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
                a.last_confirmed_at()
                    .cmp(&b.last_confirmed_at())
                    .then_with(|| lifecycle_rank(a.state).cmp(&lifecycle_rank(b.state)))
            });
            let latest = group.last().copied()?;
            let previous = group
                .iter()
                .rev()
                .skip(1)
                .find(|candidate| candidate.last_confirmed_at() < latest.last_confirmed_at())
                .copied();
            let observation_dates = group
                .iter()
                .map(|candidate| candidate.last_confirmed_at())
                .collect::<BTreeSet<_>>();
            let stale_days = (as_of_date - latest.last_confirmed_at()).num_days();
            let previous_state = previous.map(|candidate| candidate.state);
            let (current_state, direction) = classify_state(
                latest.kind,
                latest.state,
                previous_state,
                observation_dates.len(),
                stale_days,
                latest.resolved_at(),
            );

            Some(GrayRhinoMonitoringStatus {
                scope: latest.scope,
                kind: latest.kind,
                subject: latest.subject.clone(),
                current_state,
                previous_state,
                direction,
                observation_count: observation_dates.len(),
                latest_observed_at: latest.last_confirmed_at(),
                stale_days,
            })
        })
        .collect()
}

fn classify_state(
    kind: GrayRhinoCandidateKind,
    latest_state: GrayRhinoCandidateState,
    previous_state: Option<GrayRhinoCandidateState>,
    observation_count: usize,
    stale_days: i64,
    resolved_at: Option<NaiveDate>,
) -> (GrayRhinoCandidateState, GrayRhinoMonitoringDirection) {
    if latest_state == GrayRhinoCandidateState::Resolved || resolved_at.is_some() {
        return (
            GrayRhinoCandidateState::Resolved,
            GrayRhinoMonitoringDirection::Resolved,
        );
    }
    if stale_days >= 30 {
        return (
            stale_state_for_kind(kind, latest_state),
            GrayRhinoMonitoringDirection::Cooling,
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

fn is_persistent_structural_kind(kind: GrayRhinoCandidateKind) -> bool {
    matches!(
        kind,
        GrayRhinoCandidateKind::GovernanceConcentration
            | GrayRhinoCandidateKind::DependencyConcentration
            | GrayRhinoCandidateKind::InstitutionalMaturityGap
            | GrayRhinoCandidateKind::RedundancyGap
            | GrayRhinoCandidateKind::CapexPaybackFragility
    )
}

fn stale_state_for_kind(
    kind: GrayRhinoCandidateKind,
    latest_state: GrayRhinoCandidateState,
) -> GrayRhinoCandidateState {
    if is_persistent_structural_kind(kind) {
        match latest_state {
            GrayRhinoCandidateState::Critical
            | GrayRhinoCandidateState::Expanding
            | GrayRhinoCandidateState::Visible => GrayRhinoCandidateState::Cooling,
            other => other,
        }
    } else {
        GrayRhinoCandidateState::Cooling
    }
}

fn lifecycle_rank(state: GrayRhinoCandidateState) -> u8 {
    match state {
        GrayRhinoCandidateState::Background => 0,
        GrayRhinoCandidateState::Visible => 2,
        GrayRhinoCandidateState::Expanding => 3,
        GrayRhinoCandidateState::Critical => 4,
        GrayRhinoCandidateState::Cooling => 5,
        GrayRhinoCandidateState::Resolved => 6,
    }
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
